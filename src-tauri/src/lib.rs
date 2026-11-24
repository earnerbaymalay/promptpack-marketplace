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
    pub stripe_publishable_key: Option<String>,
    pub registry_url: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ollama_url: "http://localhost:11434".into(),
            default_model: "qwen2.5-coder:1.5b".into(),
            stripe_publishable_key: None,
            registry_url: "https://earnerbaymalay.github.io/promptpack-index/index.json".into(),
        }
    }
}
