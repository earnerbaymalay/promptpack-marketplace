//! eval-pack <file.promptpack> [--model NAME]
//! Verify a pack, run its eval cases on a local Ollama model, print a report,
//! and append the summary to the eval history. Settings come from palexus-brain.

use promptpack_marketplace::{eval, pack, store, AppConfig};
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(file) = args.first() else {
        eprintln!("usage: eval-pack <file.promptpack> [--model NAME]");
        return ExitCode::FAILURE;
    };
    let cfg = AppConfig::from_brain();
    let model = args.iter().position(|a| a == "--model").and_then(|i| args.get(i + 1)).cloned().unwrap_or(cfg.default_model.clone());

    let result = async {
        let json = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
        let p = pack::verify_pack(json).await?;
        println!("{} ({}) on {model}, judge {}, via {}", p.metadata.name, p.metadata.id, p.eval.judge_model, cfg.ollama_url);
        let s = eval::evaluate(&ollama_client::Client::new(&cfg.ollama_url), &p, &model, cfg.temperature).await?;
        store::append_eval(&s).map_err(|e| e.to_string())?;
        Ok::<_, String>(s)
    }
    .await;

    match result {
        Ok(s) => {
            for d in &s.details {
                let q = d.quality_score.map(|q| format!("{q:.2}")).unwrap_or("n/a".into());
                println!(
                    "  {:<4} {:<28} quality {q}  {:>5.1} tok/s  TTFT {:>6.0} ms  {} tok  {}",
                    if d.passed { "PASS" } else { "FAIL" }, d.test_case, d.tokens_per_sec, d.ttft_ms, d.tokens_out, d.judge_reason
                );
            }
            println!("{}/{} passed (threshold {:.2}), avg quality {:.2}, {:.1} tok/s", s.passed, s.total_cases, s.threshold, s.avg_quality, s.avg_tokens_per_sec);
            if s.passed == s.total_cases { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
