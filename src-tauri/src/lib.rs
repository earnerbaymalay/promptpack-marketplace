pub mod pack;
pub mod eval;
pub mod registry;
pub mod store;

use std::sync::Arc;
use tokio::sync::RwLock;

pub type SharedState<T> = Arc<RwLock<T>>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub ollama_url: String,
    pub default_model: String,
    pub temperature: f32,
    pub stripe_publishable_key: Option<String>,
    pub registry_url: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ollama_url: ollama_client::DEFAULT_URL.into(),
            default_model: "qwen2.5-coder:1.5b".into(),
            temperature: 0.1,
            stripe_publishable_key: None,
            registry_url: "https://earnerbaymalay.github.io/promptpack-index/index.json".into(),
        }
    }
}

impl AppConfig {
    /// Inference settings from the shared palexus-brain config (file + env), so every
    /// project uses the same Ollama host and default model. Falls back to defaults.
    pub fn from_brain() -> Self {
        match palexus_brain_schema::BrainConfig::load() {
            Ok(brain) => Self {
                ollama_url: brain.inference.ollama_url,
                default_model: brain.inference.default_model,
                temperature: brain.inference.default_temperature,
                ..Self::default()
            },
            Err(e) => {
                eprintln!("palexus-brain config not loaded ({e}); using defaults");
                Self::default()
            }
        }
    }
}
