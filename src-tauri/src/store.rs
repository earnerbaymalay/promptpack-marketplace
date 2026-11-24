use crate::{AppConfig, SharedState};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

#[tauri::command]
pub async fn get_config(state: State<'_, SharedState<AppConfig>>) -> Result<AppConfig, String> {
    Ok(state.read().await.clone())
}

#[tauri::command]
pub async fn set_config(
    config: AppConfig,
    state: State<'_, SharedState<AppConfig>>,
) -> Result<(), String> {
    *state.write().await = config;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalHistoryEntry {
    pub id: String,
    pub pack_id: String,
    pub model: String,
    pub summary: serde_json::Value,
    pub created_at: String,
}

#[tauri::command]
pub async fn save_eval_history(entry: EvalHistoryEntry) -> Result<(), String> {
    // In real implementation, save to local store (Tauri store plugin)
    Ok(())
}

#[tauri::command]
pub async fn get_eval_history(pack_id: Option<String>) -> Result<Vec<serde_json::Value>, String> {
    // In real implementation, load from local store
    Ok(vec![])
}

#[tauri::command]
pub async fn get_installed_packs() -> Result<Vec<String>, String> {
    // In real implementation, load from local store
    Ok(vec![])
}
