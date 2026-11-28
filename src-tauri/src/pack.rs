use crate::{AppConfig, SharedState};
use anyhow::{Context, Result as AnyhowResult};
use chrono::{DateTime, Utc};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use hex;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// Maps inside signed content are BTreeMaps: serialization must be byte-identical
// every time or signatures break (HashMap iteration order is randomised per process).

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
    pub variables: BTreeMap<String, VariableSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FewShotExample {
    pub input: BTreeMap<String, String>,
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
    pub input: BTreeMap<String, String>,
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

/// Pack id: first 16 hex chars of sha256("<author pubkey hex>:<pack name>").
/// Bound to the signing key, but unique per pack (one author can publish many).
pub fn pack_id(public_key_hex: &str, name: &str) -> String {
    let digest = Sha256::digest(format!("{public_key_hex}:{name}").as_bytes());
    hex::encode(&digest[..8])
}

/// Set key-derived fields (public key, author_pubkey, id) and sign. Timestamps
/// in metadata are left as given so re-signing an unchanged pack is reproducible.
pub fn sign_pack(mut pack: PromptPack, signing_key: &SigningKey) -> AnyhowResult<PromptPack> {
    let public_key = hex::encode(signing_key.verifying_key().as_bytes());
    pack.metadata.author_pubkey = public_key.clone();
    pack.metadata.id = pack_id(&public_key, &pack.metadata.name);
    pack.signature = PackSignature {
        algorithm: "ed25519".into(),
        public_key,
        signature: String::new(),
        signed_at: Utc::now(),
    };
    let signing_data = compute_signing_data(&pack)?;
    pack.signature.signature = hex::encode(signing_key.sign(&signing_data).to_bytes());
    Ok(pack)
}

/// Check the signature and that the key-derived fields match the signing key.
pub fn verify(pack: &PromptPack) -> Result<(), String> {
    if pack.signature.algorithm != "ed25519" {
        return Err(format!("Unsupported signature algorithm '{}'", pack.signature.algorithm));
    }
    let key_bytes: [u8; 32] = hex::decode(&pack.signature.public_key)
        .map_err(|e| format!("Invalid public key hex: {}", e))?
        .try_into()
        .map_err(|_| "Invalid public key length")?;
    let verifying_key = VerifyingKey::from_bytes(&key_bytes).map_err(|e| format!("Invalid public key: {}", e))?;
    let sig_bytes: [u8; 64] = hex::decode(&pack.signature.signature)
        .map_err(|e| format!("Invalid signature hex: {}", e))?
        .try_into()
        .map_err(|_| "Invalid signature length")?;

    let signing_data = compute_signing_data(pack).map_err(|e| format!("Failed to compute signing data: {}", e))?;
    verifying_key
        .verify(&signing_data, &Signature::from_bytes(&sig_bytes))
        .map_err(|_| "Signature verification failed")?;

    if pack.metadata.author_pubkey != pack.signature.public_key {
        return Err("metadata.author_pubkey does not match the signing key".into());
    }
    if pack.metadata.id != pack_id(&pack.signature.public_key, &pack.metadata.name) {
        return Err("Pack id does not match its key and name".into());
    }
    Ok(())
}

/// Verify a .promptpack file
#[tauri::command]
pub async fn verify_pack(pack_json: String) -> Result<PromptPack, String> {
    let pack: PromptPack = serde_json::from_str(&pack_json).map_err(|e| format!("Invalid JSON: {}", e))?;
    verify(&pack)?;
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
    let key_bytes: [u8; 32] = hex::decode(&private_key_hex)
        .map_err(|e| format!("Invalid private key hex: {}", e))?
        .try_into()
        .map_err(|_| "Private key must be 32 bytes")?;
    let now = Utc::now();
    let pack = PromptPack {
        metadata: PackMetadata { created_at: now, updated_at: now, ..metadata },
        prompt,
        eval,
        signature: PackSignature { algorithm: "ed25519".into(), public_key: String::new(), signature: String::new(), signed_at: now },
    };
    let pack = sign_pack(pack, &SigningKey::from_bytes(&key_bytes)).map_err(|e| format!("Signing failed: {}", e))?;
    serde_json::to_string_pretty(&pack).map_err(|e| format!("Serialization failed: {}", e))
}

/// Extract pack contents without verification (for inspection)
#[tauri::command]
pub async fn extract_pack(pack_json: String) -> Result<PromptPack, String> {
    serde_json::from_str(&pack_json)
        .map_err(|e| format!("Invalid JSON: {}", e))
}

fn compute_signing_data(pack: &PromptPack) -> AnyhowResult<Vec<u8>> {
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
    let _config = state.read().await;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(vars: usize) -> PromptPack {
        let now = Utc::now();
        let variables = (0..vars)
            .map(|i| (format!("var{i}"), VariableSpec { r#type: "string".into(), description: "d".into(), required: true, default: None, enum_values: None }))
            .collect();
        PromptPack {
            metadata: PackMetadata {
                id: String::new(), name: "Sample".into(), version: "1.0.0".into(), author: "me".into(),
                author_pubkey: String::new(), description: "d".into(), tags: vec![], created_at: now,
                updated_at: now, license: "MIT".into(), homepage: None, repository: None,
            },
            prompt: PromptContent { system: "s".into(), user_template: "u".into(), few_shot: vec![], variables },
            eval: EvalConfig { test_cases: vec![], metrics: vec![], judge_model: "m".into(), threshold: 0.5 },
            signature: PackSignature { algorithm: "ed25519".into(), public_key: String::new(), signature: String::new(), signed_at: now },
        }
    }

    #[test]
    fn sign_verify_round_trip_through_json() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        // Many variables: would fail intermittently with HashMap ordering.
        let signed = sign_pack(sample(12), &key).unwrap();
        let json = serde_json::to_string_pretty(&signed).unwrap();
        let reparsed: PromptPack = serde_json::from_str(&json).unwrap();
        verify(&reparsed).unwrap();
    }

    #[test]
    fn tampering_and_wrong_id_are_rejected() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let signed = sign_pack(sample(1), &key).unwrap();

        let mut tampered = signed.clone();
        tampered.prompt.system = "evil".into();
        assert!(verify(&tampered).is_err());

        let mut renamed = signed.clone();
        renamed.metadata.id = "0000000000000000".into();
        assert!(verify(&renamed).is_err());

        let mut placeholder = signed;
        placeholder.signature.signature = "placeholder_signature".into();
        assert!(verify(&placeholder).is_err());
    }

    #[test]
    fn ids_differ_per_pack_for_one_author() {
        assert_ne!(pack_id("ab", "Pack A"), pack_id("ab", "Pack B"));
        assert_eq!(pack_id("ab", "Pack A").len(), 16);
    }
}
