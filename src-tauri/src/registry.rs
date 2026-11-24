use crate::{AppConfig, SharedState};
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIndex {
    pub version: u32,
    pub updated_at: String,
    pub packs: Vec<PackIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackIndexEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub tags: Vec<String>,
    pub download_url: String,
    pub signature: String,
    pub price_usd: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitPackRequest {
    pub pack_json: String,
    pub price_usd: Option<f64>,
    pub creator_wallet: Option<String>,
}

#[tauri::command]
pub async fn fetch_index(state: tauri::State<'_, SharedState<AppConfig>>) -> Result<RegistryIndex, String> {
    let config = state.read().await;
    let client = Client::new();
    
    let response = client
        .get(&config.registry_url)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch index: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Registry returned {}", response.status()));
    }
    
    let index = response.json::<RegistryIndex>().await
        .map_err(|e| format!("Failed to parse index: {}", e))?;
    
    Ok(index)
}

#[tauri::command]
pub async fn submit_pack(
    request: SubmitPackRequest,
    state: tauri::State<'_, SharedState<AppConfig>>,
) -> Result<String, String> {
    let config = state.read().await;
    let client = Client::new();
    
    // In real implementation, this would submit to a backend
    // For now, return a mock transaction ID
    let tx_id = format!("tx_{}", uuid::Uuid::new_v4().simple());
    
    // TODO: Implement actual submission to registry backend
    // This would involve:
    // 1. Verify pack signature
    // 2. Run eval against test models
    // 2. Submit to registry (GitHub Pages + backend)
    // 3. Return transaction ID for tracking
    
    Ok(format!("Submission queued: {}", tx_id))
}

#[tauri::command]
pub async fn download_pack(pack_id: String, state: tauri::State<'_, SharedState<AppConfig>>) -> Result<String, String> {
    let config = state.read().await;
    let client = Client::new();
    
    // Fetch index to find pack
    let index = fetch_index(state).await?;
    
    let entry = index.packs.iter()
        .find(|p| p.id == pack_id)
        .ok_or_else(|| format!("Pack not found: {}", pack_id))?;
    
    // Download pack
    let response = client
        .get(&entry.download_url)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| format!("Failed to download pack: {}", e))?;
    
    let pack_json = response.text().await
        .map_err(|e| format!("Failed to read pack: {}", e))?;
    
    Ok(pack_json)
}
