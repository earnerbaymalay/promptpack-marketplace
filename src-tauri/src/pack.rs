use crate::{AppConfig, SharedState};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use hex;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// .promptpack specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptPack {
    pub metadata: PackMetadata,
    pub prompt: PromptContent,
    pub eval: EvalConfig,
    pub signature: PackSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub author_pubkey: String,
    pub description: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub license: String,
    pub homepage: Option<String>,
    pub repository: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptContent {
    pub system: String,
    pub user_template: String,
    pub few_shot: Vec<FewShotExample>,
    pub variables: HashMap<String, VariableSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FewShotExample {
    pub input: HashMap<String, String>,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableSpec {
    pub r#type: String,
    pub description: String,
    pub required: bool,
    pub default: Option<String>,
    pub enum_values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalConfig {
    pub test_cases: Vec<EvalTestCase>,
    pub metrics: Vec<MetricSpec>,
    pub judge_model: String,
    pub threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalTestCase {
    pub name: String,
    pub input: HashMap<String, String>,
    pub expected_output: Option<String>,
    pub criteria: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSpec {
    pub name: String,
    pub weight: f32,
    pub threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackSignature {
    pub algorithm: String,
    pub public_key: String,
    pub signature: String,
    pub signed_at: DateTime<Utc>,
}

/// Verify a .promptpack file
#[tauri::command]
pub async fn verify_pack(pack_json: String) -> Result<PromptPack, String> {
    let pack: PromptPack = serde_json::from_str(&pack_json)
        .map_err(|e| format!("Invalid JSON: {}", e))?;

    // Verify signature
    let signing_data = compute_signing_data(&pack)?;
    let verifying_key = VerifyingKey::from_bytes(
        &hex::decode(&pack.signature.public_key)
            .map_err(|e| format!("Invalid public key hex: {}", e))?
            .try_into()
            .map_err(|_| "Invalid public key length")?
    ).map_err(|e| format!("Invalid public key: {}", e))?;

    let signature = Signature::from_bytes(
        &hex::decode(&pack.signature.signature)
            .map_err(|e| format!("Invalid signature hex: {}", e))?
            .try_into()
            .map_err(|_| "Invalid signature length")?
    );

    verifying_key.verify(&signing_data, &signature)
        .map_err(|_| "Signature verification failed")?;

    // Verify metadata consistency
    if pack.metadata.id != pack.signature.public_key[..16] {
        return Err("Pack ID mismatch with public key".into());
    }

    Ok(pack)
}

/// Create a new .promptpack (for creators)
#[tauri::command]
pub async fn create_pack(
    metadata: PackMetadata,
    prompt: PromptContent,
    eval: EvalConfig,
    private_key_hex: String,
) -> Result<String, String> {
    let mut pack = PromptPack {
        metadata,
        prompt,
        eval,
        signature: PackSignature {
            algorithm: "ed25519".into(),
            public_key: String::new(),
            signature: String::new(),
            signed_at: Utc::now(),
        },
    };

    // Derive public key from private key
    let private_key_bytes = hex::decode(&private_key_hex)
        .map_err(|e| format!("Invalid private key hex: {}", e))?;
    let signing_key = SigningKey::from_bytes(
        &private_key_bytes.try_into().map_err(|_| "Private key must be 32 bytes")?
    );
    let verifying_key = signing_key.verifying_key();
    
    pack.signature.public_key = hex::encode(verifying_key.as_bytes());
    pack.metadata.id = verifying_key.as_bytes()[..16].iter().map(|b| format!("{:02x}", b)).collect::<String>();
    pack.metadata.author_pubkey = pack.signature.public_key.clone();
    pack.metadata.created_at = Utc::now();
    pack.metadata.updated_at = Utc::now();

    // Sign the pack
    let signing_data = compute_signing_data(&pack)?;
    let signature = signing_key.sign(&signing_data);
    pack.signature.signature = hex::encode(signature.to_bytes());
    pack.signature.signed_at = Utc::now();

    serde_json::to_string_pretty(&pack)
        .map_err(|e| format!("Serialization failed: {}", e))
}

/// Extract pack contents without verification (for inspection)
#[tauri::command]
pub async fn extract_pack(pack_json: String) -> Result<PromptPack, String> {
    serde_json::from_str(&pack_json)
        .map_err(|e| format!("Invalid JSON: {}", e))
}

fn compute_signing_data(pack: &PromptPack) -> Result<Vec<u8>> {
    // Create deterministic signing data (exclude signature field)
    let mut hasher = Sha256::new();
    let metadata_json = serde_json::to_vec(&pack.metadata)?;
    let prompt_json = serde_json::to_vec(&pack.prompt)?;
    let eval_json = serde_json::to_vec(&pack.eval)?;
    hasher.update(&metadata_json);
    hasher.update(&prompt_json);
    hasher.update(&eval_json);
    Ok(hasher.finalize().to_vec())
}

#[tauri::command]
pub async fn watch_packs_dir(path: String, state: tauri::State<'_, SharedState<AppConfig>>) -> Result<(), String> {
    let config = state.read().await;
    let path = PathBuf::from(path);
    
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.blocking_send(event);
            }
        },
        Config::default()
    ).map_err(|e| format!("Failed to create watcher: {}", e))?;

    watcher.watch(&path, RecursiveMode::Recursive)
        .map_err(|e| format!("Failed to watch directory: {}", e))?;

    // Spawn background task to handle events
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event.kind {
                EventKind::Create(_) | EventKind::Modify(_) => {
                    for path in event.paths {
                        if path.extension().map_or(false, |ext| ext == "promptpack") {
                            // Trigger re-verification
                            println!("Pack changed: {:?}", path);
                        }
                    }
                }
                _ => {}
            }
        }
    });

    // Keep watcher alive (in real app, store in state)
    std::mem::forget(watcher);
    Ok(())
}
