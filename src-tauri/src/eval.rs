use crate::{AppConfig, SharedState};
use anyhow::{Context, Result as AnyhowResult};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub id: String,
    pub pack_id: String,
    pub model: String,
    pub test_case: String,
    pub input: HashMap<String, String>,
    pub output: String,
    pub latency_ms: u64,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost_usd: f64,
    pub quality_score: f32,
    pub passed: bool,
    pub metrics: HashMap<String, f32>,
    pub evaluated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSummary {
    pub pack_id: String,
    pub model: String,
    pub total_cases: usize,
    pub passed: usize,
    pub failed: usize,
    pub avg_latency_ms: f64,
    pub total_tokens_in: u32,
    pub total_tokens_out: u32,
    pub total_cost_usd: f64,
    pub avg_quality: f32,
    pub overall_score: f32,
    pub details: Vec<EvalResult>,
    pub evaluated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    system: Option<String>,
    template: Option<String>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaOptions {
    temperature: f32,
    top_p: f32,
    top_k: u32,
    num_predict: i32,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
    eval_count: Option<u32>,
    eval_duration: Option<u64>,
    prompt_eval_count: Option<u32>,
    prompt_eval_duration: Option<u64>,
    done: bool,
}

#[tauri::command]
pub async fn run_eval(
    pack_id: String,
    model: Option<String>,
    test_cases: Vec<serde_json::Value>,
    state: tauri::State<'_, SharedState<AppConfig>>,
) -> Result<EvalSummary, String> {
    let config = state.read().await;
    let client = Client::new();
    let model = model.unwrap_or_else(|| config.default_model.clone());
    let ollama_url = config.ollama_url.clone();

    let mut results = Vec::new();
    let mut total_latency = 0u64;
    let mut total_tokens_in = 0u32;
    let mut total_tokens_out = 0u32;
    let mut total_cost = 0.0f64;
    let mut total_quality = 0.0f32;
    let mut passed = 0;

    for (idx, test_case) in test_cases.iter().enumerate() {
        let start = Instant::now();
        
        let prompt = build_prompt(&test_case)
            .map_err(|e| format!("Failed to build prompt: {}", e))?;
        
        let (output, tokens_in, tokens_out, latency_ms) = call_ollama(&client, &ollama_url, &model, &prompt).await
            .map_err(|e| format!("Ollama call failed: {}", e))?;
        
        let latency_ms = start.elapsed().as_millis() as u64;
        total_latency += latency_ms;
        
        let tokens_in = tokens_in.unwrap_or(0);
        let tokens_out = tokens_out.unwrap_or(0);
        total_tokens_in += tokens_in;
        total_tokens_out += tokens_out;
        
        // Calculate cost (rough estimate: $0.001 per 1k tokens for local compute)
        let cost = (tokens_in + tokens_out) as f64 * 0.000001;
        total_cost += cost;
        
        // Quality scoring via local judge (simplified)
        let quality = score_quality(&test_case, &output)
            .map_err(|e| format!("Failed to score quality: {}", e))?;
        total_quality += quality;
        
        let passed_threshold = quality >= 0.7;
        if passed_threshold { passed += 1; }
        
        let mut metrics = HashMap::new();
        metrics.insert("latency_ms".into(), latency_ms as f32);
        metrics.insert("quality".into(), quality);
        metrics.insert("tokens_per_sec".into(), if latency_ms > 0 { tokens_out as f32 / (latency_ms as f32 / 1000.0) } else { 0.0 });
        
        results.push(EvalResult {
            id: Uuid::new_v4().to_string(),
            pack_id: pack_id.clone(),
            model: model.clone(),
            test_case: format!("case_{}", idx),
            input: serde_json::from_value(test_case["input"].clone()).unwrap_or_default(),
            output,
            latency_ms,
            tokens_in,
            tokens_out,
            cost_usd: cost,
            quality_score: quality,
            passed: passed_threshold,
            metrics,
            evaluated_at: Utc::now(),
        });
    }

    let avg_latency = if results.is_empty() { 0.0 } else { total_latency as f64 / results.len() as f64 };
    let avg_quality = if results.is_empty() { 0.0 } else { total_quality / results.len() as f32 };
    let overall_score = (passed as f32 / results.len().max(1) as f32) * 0.5 + avg_quality * 0.5;

    Ok(EvalSummary {
        pack_id,
        model,
        total_cases: results.len(),
        passed,
        failed: results.len() - passed,
        avg_latency_ms: avg_latency,
        total_tokens_in,
        total_tokens_out,
        total_cost_usd: total_cost,
        avg_quality,
        overall_score,
        details: results,
        evaluated_at: Utc::now(),
    })
}

#[tauri::command]
pub async fn get_eval_results(pack_id: String) -> Result<Vec<EvalSummary>, String> {
    // In real implementation, load from local store
    Ok(vec![])
}

fn build_prompt(test_case: &Value) -> AnyhowResult<String> {
    let system = test_case["system"].as_str().unwrap_or("");
    let template = test_case["template"].as_str().unwrap_or("{input}");
    let input = test_case["input"].as_str().unwrap_or("");
    
    let prompt = template.replace("{input}", input);
    Ok(format!("System: {}\n\nUser: {}", system, prompt))
}

async fn call_ollama(
    client: &Client,
    url: &str,
    model: &str,
    prompt: &str,
) -> AnyhowResult<(String, Option<u32>, Option<u32>, u64)> {
    let request = OllamaRequest {
        model: model.to_string(),
        prompt: prompt.to_string(),
        system: None,
        template: None,
        stream: false,
        options: OllamaOptions {
            temperature: 0.1,
            top_p: 0.9,
            top_k: 40,
            num_predict: 2048,
        },
    };

    let start = Instant::now();
    let response = client
        .post(&format!("{}/api/generate", url))
        .json(&request)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .context("Failed to call Ollama")?;

    let latency_ms = start.elapsed().as_millis() as u64;
    
    let resp: OllamaResponse = response.json().await
        .context("Failed to parse Ollama response")?;

    Ok((
        resp.response,
        resp.prompt_eval_count,
        resp.eval_count,
        latency_ms,
    ))
}

fn score_quality(test_case: &Value, output: &str) -> AnyhowResult<f32> {
    // Simple heuristic scoring - in production use a local judge model
    let criteria = test_case["criteria"].as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    
    if criteria.is_empty() {
        // Length-based heuristic
        let len = output.len() as f32;
        return Ok((len / 100.0).min(1.0).max(0.1));
    }
    
    let matches = criteria.iter().filter(|c| output.to_lowercase().contains(&c.to_lowercase())).count();
    Ok(matches as f32 / criteria.len() as f32)
}