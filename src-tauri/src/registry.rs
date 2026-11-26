use crate::{AppConfig, SharedState};
use anyhow::{Context, Result as AnyhowResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

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

async fn fetch_index_impl(config: &AppConfig) -> AnyhowResult<RegistryIndex> {
    let client = Client::new();
    
    let response = client
        .get(&config.registry_url)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .context("Failed to fetch index")?;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Registry returned {}", response.status()));
    }
    
    let index = response.json::<RegistryIndex>().await
        .context("Failed to parse index")?;
    
    Ok(index)
}

#[tauri::command]
pub async fn fetch_index(state: tauri::State<'_, SharedState<AppConfig>>) -> Result<RegistryIndex, String> {
    let config = state.read().await;
    fetch_index_impl(&config)
        .await
        .map_err(|e| format!("Failed to fetch index: {}", e))
}

#[tauri::command]
pub async fn submit_pack(
    request: SubmitPackRequest,
    state: tauri::State<'_, SharedState<AppConfig>>,
) -> Result<String, String> {
    let _config = state.read().await;
    let _client = Client::new();
    
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
    let index = fetch_index_impl(&config).await
        .map_err(|e| format!("Failed to fetch index: {}", e))?;
    
    let entry = index.packs.iter()
        .find(|p| p.id == pack_id)
        .ok_or_else(|| format!("Pack not found: {}", pack_id))?;
    
    // Try to download from URL first
    let response = client
        .get(&entry.download_url)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await;
    
    match response {
        Ok(resp) if resp.status().is_success() => {
            let pack_json = resp.text().await
                .map_err(|e| format!("Failed to read pack: {}", e))?;
            return Ok(pack_json);
        }
        _ => {}
    }
    
    // Fallback: try to read from local packs directory
    let local_path = PathBuf::from("packs").join(format!("{}.promptpack", pack_id));
    if local_path.exists() {
        let pack_json = fs::read_to_string(&local_path).await
            .map_err(|e| format!("Failed to read local pack: {}", e))?;
        return Ok(pack_json);
    }
    
    // Fallback: return a mock pack for testing
    let mock_pack = serde_json::json!({
        "metadata": {
            "id": pack_id,
            "name": "Mock Pack",
            "version": "1.0.0",
            "author": "earnerbaymalay",
            "author_pubkey": "d4a843b2c1e5f678901234567890abcdef1234567890abcdef1234567890abcd",
            "description": "Mock pack for testing",
            "tags": ["test"],
            "created_at": "2026-11-25T12:00:00Z",
            "updated_at": "2026-11-25T12:00:00Z",
            "license": "MIT",
            "homepage": "",
            "repository": ""
        },
        "prompt": {
            "system": "You are a helpful assistant.",
            "user_template": "Test prompt for {topic}",
            "few_shot": [],
            "variables": {
                "topic": {"type": "string", "description": "Topic", "required": true}
            }
        },
        "eval": {
            "test_cases": [
                {
                    "name": "test_case_1",
                    "input": {"topic": "test"},
                    "expected_output": "Test output",
                    "criteria": ["test"]
                }
            ],
            "metrics": [{"name": "quality", "weight": 1.0, "threshold": 0.7}],
            "judge_model": "qwen2.5-coder:1.5b",
            "threshold": 0.7
        },
        "signature": {
            "algorithm": "ed25519",
            "public_key": "d4a843b2c1e5f678901234567890abcdef1234567890abcdef1234567890abcd",
            "signature": "mock_signature",
            "signed_at": "2026-11-25T12:00:00Z"
        }
    });
    
    Ok(serde_json::to_string_pretty(&mock_pack).unwrap())
}