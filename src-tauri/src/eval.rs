//! Evaluate a prompt pack against a local Ollama model.
//!
//! Each eval test case is run through the pack's real prompt (system prompt,
//! few-shot examples, user template filled with the case's inputs). A judge
//! model (the pack's `eval.judge_model`) then scores the answer 0..1 against
//! the case's criteria. Timing comes from Ollama's own counters.

use crate::pack::{EvalTestCase, PromptPack};
use crate::{AppConfig, SharedState};
use chrono::{DateTime, Utc};
use ollama_client::{ChatRequest, Client, Message, Options};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub test_case: String,
    pub input: BTreeMap<String, String>,
    pub output: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub total_ms: f64,
    pub ttft_ms: f64,
    pub tpot_ms: f64,
    pub tokens_per_sec: f64,
    /// 0..1 from the judge model; `None` if judging failed.
    pub quality_score: Option<f32>,
    pub judge_reason: String,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSummary {
    pub pack_id: String,
    pub pack_name: String,
    pub model: String,
    pub judge_model: String,
    pub threshold: f32,
    pub total_cases: usize,
    pub passed: usize,
    pub failed: usize,
    pub avg_quality: f32,
    pub avg_tokens_per_sec: f64,
    pub avg_ttft_ms: f64,
    pub avg_tpot_ms: f64,
    pub total_tokens_in: u64,
    pub total_tokens_out: u64,
    pub details: Vec<EvalResult>,
    pub evaluated_at: DateTime<Utc>,
}

/// Fill `{name}` placeholders from `inputs`, falling back to variable defaults.
/// Unknown placeholders are left as-is so literal braces in prompts survive.
pub fn render_template(pack: &PromptPack, inputs: &BTreeMap<String, String>) -> String {
    let mut out = pack.prompt.user_template.clone();
    for (name, spec) in &pack.prompt.variables {
        let value = inputs.get(name).cloned().or_else(|| spec.default.clone()).unwrap_or_default();
        out = out.replace(&format!("{{{name}}}"), &value);
    }
    for (name, value) in inputs {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// Messages for one case: system prompt, few-shot pairs, then the filled template.
pub fn build_messages(pack: &PromptPack, inputs: &BTreeMap<String, String>) -> Vec<Message> {
    let mut messages = vec![Message::system(&pack.prompt.system)];
    for shot in &pack.prompt.few_shot {
        messages.push(Message::user(render_template(pack, &shot.input)));
        messages.push(Message::assistant(&shot.output));
    }
    messages.push(Message::user(render_template(pack, inputs)));
    messages
}

#[derive(Deserialize)]
struct Verdict {
    score: f32,
    #[serde(default)]
    reason: String,
}

/// Ask the judge model for {"score": 0..1, "reason": "..."}.
async fn judge(client: &Client, judge_model: &str, case: &EvalTestCase, answer: &str) -> Result<Verdict, String> {
    let criteria = case.criteria.iter().map(|c| format!("- {c}")).collect::<Vec<_>>().join("\n");
    let expected = case.expected_output.as_deref().map(|e| format!("\nReference answer (for guidance):\n{e}\n")).unwrap_or_default();
    let prompt = format!(
        "You grade an AI answer against criteria.\n\nCriteria:\n{criteria}\n{expected}\nAnswer to grade:\n<<<\n{answer}\n>>>\n\n\
         Reply with ONLY a JSON object: {{\"score\": <number 0 to 1, fraction of criteria met>, \"reason\": \"<one sentence>\"}}"
    );
    let req = ChatRequest::new(judge_model, vec![Message::user(prompt)])
        .options(Options { temperature: Some(0.0), max_tokens: Some(200), ..Default::default() });
    let text = client.chat(&req).await.map_err(|e| e.to_string())?.message.content;
    parse_verdict(&text)
}

/// Pull the first {...} block out of the judge's reply (models often add prose or fences).
fn parse_verdict(text: &str) -> Result<Verdict, String> {
    let start = text.find('{').ok_or("judge reply has no JSON")?;
    let end = text.rfind('}').ok_or("judge reply has no JSON")?;
    let mut v: Verdict = serde_json::from_str(&text[start..=end]).map_err(|e| format!("bad judge JSON: {e}"))?;
    v.score = v.score.clamp(0.0, 1.0);
    Ok(v)
}

/// Run every eval test case in `pack` on `model`, judged by the pack's judge model.
pub async fn evaluate(client: &Client, pack: &PromptPack, model: &str, temperature: f32) -> Result<EvalSummary, String> {
    let mut details = Vec::new();
    for (i, case) in pack.eval.test_cases.iter().enumerate() {
        let req = ChatRequest::new(model, build_messages(pack, &case.input))
            .options(Options { temperature: Some(temperature), max_tokens: Some(2048), ..Default::default() });
        let resp = client.chat(&req).await.map_err(|e| format!("case {}: {e}", case.name))?;
        let m = resp.metrics;

        let (quality_score, judge_reason) = match judge(client, &pack.eval.judge_model, case, &resp.message.content).await {
            Ok(v) => (Some(v.score), v.reason),
            Err(e) => (None, format!("judge failed: {e}")),
        };
        details.push(EvalResult {
            test_case: if case.name.is_empty() { format!("case_{i}") } else { case.name.clone() },
            input: case.input.clone(),
            output: resp.message.content,
            tokens_in: m.prompt_eval_count,
            tokens_out: m.eval_count,
            total_ms: m.total_ms(),
            ttft_ms: m.ttft_ms(),
            tpot_ms: m.tpot_ms(),
            tokens_per_sec: m.tokens_per_sec(),
            quality_score,
            judge_reason,
            passed: quality_score.is_some_and(|q| q >= pack.eval.threshold),
        });
    }
    Ok(summarize(pack, model, details))
}

fn summarize(pack: &PromptPack, model: &str, details: Vec<EvalResult>) -> EvalSummary {
    let n = details.len().max(1) as f64;
    let avg = |f: fn(&EvalResult) -> f64| details.iter().map(f).sum::<f64>() / n;
    let judged: Vec<f32> = details.iter().filter_map(|d| d.quality_score).collect();
    let passed = details.iter().filter(|d| d.passed).count();
    EvalSummary {
        pack_id: pack.metadata.id.clone(),
        pack_name: pack.metadata.name.clone(),
        model: model.to_string(),
        judge_model: pack.eval.judge_model.clone(),
        threshold: pack.eval.threshold,
        total_cases: details.len(),
        passed,
        failed: details.len() - passed,
        avg_quality: if judged.is_empty() { 0.0 } else { judged.iter().sum::<f32>() / judged.len() as f32 },
        avg_tokens_per_sec: avg(|d| d.tokens_per_sec),
        avg_ttft_ms: avg(|d| d.ttft_ms),
        avg_tpot_ms: avg(|d| d.tpot_ms),
        total_tokens_in: details.iter().map(|d| d.tokens_in).sum(),
        total_tokens_out: details.iter().map(|d| d.tokens_out).sum(),
        details,
        evaluated_at: Utc::now(),
    }
}

/// Verify `pack_json`, then evaluate it on `model` (default: configured model).
#[tauri::command]
pub async fn run_eval(
    pack_json: String,
    model: Option<String>,
    state: tauri::State<'_, SharedState<AppConfig>>,
) -> Result<EvalSummary, String> {
    let pack = crate::pack::verify_pack(pack_json).await?;
    let (url, default_model, temperature) = {
        let c = state.read().await;
        (c.ollama_url.clone(), c.default_model.clone(), c.temperature)
    };
    let summary = evaluate(&Client::new(&url), &pack, model.as_deref().unwrap_or(&default_model), temperature).await?;
    crate::store::append_eval(&summary).map_err(|e| format!("eval ran but saving history failed: {e}"))?;
    Ok(summary)
}

/// Past eval summaries for a pack, newest first.
#[tauri::command]
pub async fn get_eval_results(pack_id: String) -> Result<Vec<EvalSummary>, String> {
    crate::store::load_evals(&pack_id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_parsing_tolerates_prose_and_clamps() {
        let v = parse_verdict("Sure!\n```json\n{\"score\": 1.4, \"reason\": \"all met\"}\n```").unwrap();
        assert_eq!(v.score, 1.0);
        assert_eq!(v.reason, "all met");
        assert!(parse_verdict("no json here").is_err());
    }
}
