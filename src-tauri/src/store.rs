use crate::eval::EvalSummary;
use crate::pack::PromptPack;
use crate::{AppConfig, SharedState};
use anyhow::{Context, Result as AnyhowResult};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub async fn get_config(state: State<'_, SharedState<AppConfig>>) -> Result<AppConfig, String> {
    Ok(state.read().await.clone())
}

#[tauri::command]
pub async fn set_config(
    state: State<'_, SharedState<AppConfig>>,
    key: String,
    value: String,
) -> Result<(), String> {
    let mut config = state.write().await;
    match key.as_str() {
        "ollama_url" => config.ollama_url = value,
        "ollama_timeout" => config.default_model = value, // repurposing for simplicity
        "registry_url" => config.registry_url = value,
        "stripe_publishable_key" => config.stripe_publishable_key = Some(value),
        "stripe_secret_key" => config.stripe_secret_key = Some(value),
        "stripe_webhook_secret" => config.stripe_webhook_secret = Some(value),
        _ => return Err(format!("Unknown config key: {}", key)),
    }
    Ok(())
}

/// Pack info for the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub cases: usize,
    pub tags: Vec<String>,
    pub installed_at: String,
}

/// Packs directory: `<palexus-brain data dir>/promptpack/packs/`
fn packs_dir() -> PathBuf {
    let dir = std::env::var_os("PROMPTPACK_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| {
        palexus_brain_schema::BrainConfig::data_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("promptpack")
    });
    dir.join("packs")
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

/// List installed packs with full info
#[tauri::command]
pub async fn get_installed_packs() -> Result<Vec<PackInfo>, String> {
    let dir = packs_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }
    
    let mut packs = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "promptpack") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(pack) = serde_json::from_str::<PromptPack>(&content) {
                    packs.push(PackInfo {
                        id: pack.metadata.id.clone(),
                        name: pack.metadata.name.clone(),
                        version: pack.metadata.version.clone(),
                        author: pack.metadata.author.clone(),
                        description: pack.metadata.description.clone(),
                        cases: pack.eval.test_cases.len(),
                        tags: pack.metadata.tags.clone(),
                        installed_at: entry.metadata()
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
                            .unwrap_or_default(),
                    });
                }
            }
        }
    }
    
    packs.sort_by(|a, b| b.installed_at.cmp(&a.installed_at));
    Ok(packs)
}

/// Install a pack from base64-encoded .promptpack file
#[tauri::command]
pub async fn install_pack(pack_data: String, file_name: String) -> Result<(), String> {
    let dir = packs_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    
    // Decode base64
    let bytes = base64::engine::general_purpose::STANDARD.decode(&pack_data)
        .map_err(|e| format!("Invalid base64: {}", e))?;
    let pack_json = String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8: {}", e))?;
    
    // Verify the pack first
    let pack: PromptPack = serde_json::from_str(&pack_json).map_err(|e| format!("Invalid JSON: {}", e))?;
    crate::pack::verify(&pack).map_err(|e| format!("Pack verification failed: {}", e))?;
    
    // Save to packs directory
    let dest = dir.join(&file_name);
    std::fs::write(&dest, pack_json).map_err(|e| e.to_string())?;
    
    Ok(())
}

/// Uninstall a pack by ID
#[tauri::command]
pub async fn uninstall_pack(pack_id: String) -> Result<(), String> {
    let dir = packs_dir();
    if !dir.exists() {
        return Err("Packs directory not found".into());
    }
    
    let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "promptpack") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(pack) = serde_json::from_str::<PromptPack>(&content) {
                    if pack.metadata.id == pack_id {
                        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
                        return Ok(());
                    }
                }
            }
        }
    }
    
    Err("Pack not found".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_id_generation() {
        // Test that pack_id is deterministic
        let id1 = crate::pack::pack_id("abc", "test");
        let id2 = crate::pack::pack_id("abc", "test");
        assert_eq!(id1, id2);
    }
}