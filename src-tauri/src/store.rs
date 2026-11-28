use crate::eval::EvalSummary;
use crate::{AppConfig, SharedState};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub async fn get_config(state: State<'_, SharedState<AppConfig>>) -> Result<AppConfig, String> {
    Ok(state.read().await.clone())
}

#[tauri::command]
pub async fn set_config(config: AppConfig, state: State<'_, SharedState<AppConfig>>) -> Result<(), String> {
    *state.write().await = config;
    Ok(())
}

/// `<palexus-brain data dir>/promptpack/evals.jsonl`, one EvalSummary per line.
/// Override with PROMPTPACK_DATA_DIR.
fn evals_path() -> PathBuf {
    let dir = std::env::var_os("PROMPTPACK_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| {
        palexus_brain_schema::BrainConfig::data_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("promptpack")
    });
    dir.join("evals.jsonl")
}

pub fn append_eval(summary: &EvalSummary) -> std::io::Result<()> {
    let path = evals_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", serde_json::to_string(summary)?)
}

/// Summaries for `pack_id` (all packs if empty), newest first. Unreadable lines are skipped.
pub fn load_evals(pack_id: &str) -> std::io::Result<Vec<EvalSummary>> {
    let file = match std::fs::File::open(evals_path()) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    let mut out: Vec<EvalSummary> = std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str::<EvalSummary>(&l).ok())
        .filter(|s| pack_id.is_empty() || s.pack_id == pack_id)
        .collect();
    out.reverse();
    Ok(out)
}

#[tauri::command]
pub async fn get_eval_history(pack_id: Option<String>) -> Result<Vec<EvalSummary>, String> {
    load_evals(pack_id.as_deref().unwrap_or("")).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_installed_packs() -> Result<Vec<String>, String> {
    // Installed-pack tracking is not implemented yet.
    Ok(vec![])
}
