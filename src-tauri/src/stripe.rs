// Stripe integration for PromptPack Marketplace
// Handles checkout sessions for paid packs and webhook verification using direct HTTP calls

use crate::{AppConfig, SharedState};
use anyhow::{Context, Result as AnyhowResult};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSessionRequest {
    pub pack_id: String,
    pub success_url: String,
    pub cancel_url: String,
    pub customer_email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSessionResponse {
    pub session_id: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub id: String,
    pub object: String,
    pub api_version: String,
    pub created: i64,
    pub data: WebhookEventData,
    pub livemode: bool,
    pub pending_webhooks: i32,
    pub request: Option<WebhookRequest>,
    pub type_: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEventData {
    pub object: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookRequest {
    pub id: String,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseRecord {
    pub session_id: String,
    pub pack_id: String,
    pub customer_email: String,
    pub amount_usd: f64,
    pub purchased_at: String,
    pub download_token: String,
}

/// Stripe Checkout Session response
#[derive(Debug, Deserialize)]
struct StripeCheckoutSession {
    id: String,
    url: Option<String>,
}

/// Create a Stripe Checkout session for a paid pack using direct HTTP API
#[tauri::command]
pub async fn create_checkout_session(
    request: CheckoutSessionRequest,
    state: State<'_, SharedState<AppConfig>>,
) -> Result<CheckoutSessionResponse, String> {
    let config = state.read().await;
    
    let stripe_secret = config.stripe_secret_key.as_ref()
        .ok_or("Stripe not configured. Set stripe_secret_key in config.")?;
    
    let http_client = HttpClient::new();
    
    // Fetch pack details from registry
    let registry_url = config.registry_url.clone();
    let pack_info = fetch_pack_info(&registry_url, &request.pack_id).await
        .map_err(|e| format!("Failed to fetch pack info: {}", e))?;
    
    let price_cents = (pack_info.price_usd.unwrap_or(0.0) * 100.0) as i64;
    if price_cents <= 0 {
        return Err("Pack is free, no checkout needed".into());
    }
    
    // Build form data for Stripe Checkout Session creation
    let pack_name = pack_info.name.clone();
    let pack_description = pack_info.description.clone();
    let pack_version = pack_info.version.clone();
    let mut form = HashMap::new();
    form.insert("success_url", request.success_url);
    form.insert("cancel_url", request.cancel_url);
    form.insert("mode", "payment".to_string());
    form.insert("line_items[0][price_data][currency]", "usd".to_string());
    form.insert("line_items[0][price_data][product_data][name]", pack_name.clone());
    form.insert("line_items[0][price_data][product_data][description]", pack_description);
    form.insert("line_items[0][price_data][unit_amount]", price_cents.to_string());
    form.insert("line_items[0][quantity]", "1".to_string());
    
    if let Some(email) = request.customer_email {
        form.insert("customer_email", email);
    }
    
    // Add metadata for webhook processing
    form.insert("metadata[pack_id]", request.pack_id.clone());
    form.insert("metadata[pack_name]", pack_name);
    form.insert("metadata[pack_version]", pack_version);
    
    // Make request to Stripe API
    let response = http_client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(stripe_secret, Some(""))
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("Stripe API request failed: {}", e))?;
    
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Stripe API error: {}", error_text));
    }
    
    let session: StripeCheckoutSession = response.json().await
        .map_err(|e| format!("Failed to parse Stripe response: {}", e))?;
    
    Ok(CheckoutSessionResponse {
        session_id: session.id,
        url: session.url.unwrap_or_default(),
    })
}

/// Handle Stripe webhook events (checkout.session.completed)
#[tauri::command]
pub async fn handle_stripe_webhook(
    payload: String,
    signature: String,
    state: State<'_, SharedState<AppConfig>>,
) -> Result<String, String> {
    let config = state.read().await;
    
    let webhook_secret = config.stripe_webhook_secret.as_ref()
        .ok_or("Stripe webhook secret not configured")?;
    
    // Verify webhook signature using stripe's algorithm
    if !verify_webhook_signature(&payload, &signature, webhook_secret) {
        return Err("Webhook signature verification failed".into());
    }
    
    // Parse event
    let event: WebhookEvent = serde_json::from_str(&payload)
        .map_err(|e| format!("Failed to parse webhook event: {}", e))?;
    
    if event.type_ == "checkout.session.completed" {
        if let Some(session) = event.data.object.as_object() {
            let session_id = session.get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            
            let pack_id = session.get("metadata")
                .and_then(|m| m.as_object())
                .and_then(|m| m.get("pack_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            
            let customer_email = session.get("customer_email")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            
            let amount_total = session.get("amount_total")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            
            if !pack_id.is_empty() && !session_id.is_empty() {
                record_purchase(PurchaseRecord {
                    session_id: session_id.to_string(),
                    pack_id: pack_id.to_string(),
                    customer_email: customer_email.to_string(),
                    amount_usd: amount_total as f64 / 100.0,
                    purchased_at: chrono::Utc::now().to_rfc3339(),
                    download_token: generate_download_token(session_id, pack_id),
                }).await.map_err(|e| format!("Failed to record purchase: {}", e))?;
            }
        }
    }
    
    Ok("Webhook processed".to_string())
}

/// Verify a download token for paid pack access
#[tauri::command]
pub async fn verify_download_token(
    token: String,
    pack_id: String,
) -> Result<bool, String> {
    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() != 2 {
        return Ok(false);
    }
    let session_id = parts[0];
    let expected = generate_download_token(session_id, &pack_id);
    Ok(token == expected)
}

/// Download a pack (checks payment for paid packs)
#[tauri::command]
pub async fn download_pack_secure(
    pack_id: String,
    download_token: Option<String>,
    state: State<'_, SharedState<AppConfig>>,
) -> Result<String, String> {
    let config = state.read().await;
    let http_client = HttpClient::new();
    
    // Fetch index to find pack
    let index = fetch_index_impl(&config.registry_url).await
        .map_err(|e| format!("Failed to fetch index: {}", e))?;
    
    let entry = index.packs.iter()
        .find(|p| p.id == pack_id)
        .ok_or_else(|| format!("Pack not found: {}", pack_id))?;
    
    // Check if pack is paid
    if let Some(price) = entry.price_usd {
        if price > 0.0 {
            // Paid pack - require valid download token
            let token = download_token.ok_or("Paid pack requires download token")?;
            let valid = verify_download_token(token, pack_id.clone()).await?;
            if !valid {
                return Err("Invalid or expired download token".into());
            }
        }
    }
    
    // Download from URL
    let response = http_client
        .get(&entry.download_url)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await;
    
    match response {
        Ok(resp) if resp.status().is_success() => {
            let pack_json = resp.text().await
                .map_err(|e| format!("Failed to read pack: {}", e))?;
            Ok(pack_json)
        }
        _ => {
            // Fallback: try local
            let local_path = std::path::PathBuf::from("packs").join(format!("{}.promptpack", pack_id));
            if local_path.exists() {
                let pack_json = tokio::fs::read_to_string(&local_path).await
                    .map_err(|e| format!("Failed to read local pack: {}", e))?;
                Ok(pack_json)
            } else {
                Err("Pack download failed".into())
            }
        }
    }
}

// Helper functions

async fn fetch_pack_info(registry_url: &str, pack_id: &str) -> AnyhowResult<PackInfo> {
    let http_client = HttpClient::new();
    let response = http_client
        .get(registry_url)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;
    
    let index: RegistryIndex = response.json().await?;
    
    index.packs.iter()
        .find(|p| p.id == pack_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Pack not found: {}", pack_id))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryIndex {
    pub version: u32,
    pub updated_at: String,
    pub packs: Vec<PackInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PackInfo {
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

async fn fetch_index_impl(registry_url: &str) -> AnyhowResult<RegistryIndex> {
    let http_client = HttpClient::new();
    let response = http_client
        .get(registry_url)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;
    
    let index = response.json::<RegistryIndex>().await?;
    Ok(index)
}

async fn record_purchase(record: PurchaseRecord) -> AnyhowResult<()> {
    let path = std::path::PathBuf::from("purchases.jsonl");
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", serde_json::to_string(&record)?)?;
    Ok(())
}

fn verify_webhook_signature(payload: &str, signature: &str, secret: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    
    // Stripe signature format: t=<timestamp>,v1=<signature>
    let parts: Vec<&str> = signature.split(',').collect();
    let mut timestamp = "";
    let mut v1_sig = "";
    
    for part in parts {
        if part.starts_with("t=") {
            timestamp = &part[2..];
        } else if part.starts_with("v1=") {
            v1_sig = &part[3..];
        }
    }
    
    if timestamp.is_empty() || v1_sig.is_empty() {
        return false;
    }
    
    // Check timestamp is recent (within 5 minutes)
    if let Ok(ts) = timestamp.parse::<i64>() {
        let now = chrono::Utc::now().timestamp();
        if (now - ts).abs() > 300 {
            return false;
        }
    }
    
    // Compute expected signature
    let signed_payload = format!("{}.{}", timestamp, payload);
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(signed_payload.as_bytes());
    let result = mac.finalize();
    let expected_sig = hex::encode(result.into_bytes());
    
    // Constant-time comparison
    subtle_equals(v1_sig, &expected_sig)
}

fn subtle_equals(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (ca, cb) in a.bytes().zip(b.bytes()) {
        result |= ca ^ cb;
    }
    result == 0
}

fn generate_download_token(session_id: &str, pack_id: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    
    let secret = std::env::var("STRIPE_DOWNLOAD_SECRET")
        .unwrap_or_else(|_| "dev-secret-change-in-production".to_string());
    
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(format!("{}:{}", session_id, pack_id).as_bytes());
    let result = mac.finalize();
    let code = hex::encode(result.into_bytes());
    format!("{}:{}", session_id, code)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_download_token_generation() {
        let token1 = generate_download_token("sess_123", "pack_abc");
        let token2 = generate_download_token("sess_123", "pack_abc");
        assert_eq!(token1, token2);
        
        let token3 = generate_download_token("sess_456", "pack_abc");
        assert_ne!(token1, token3);
    }
    
    #[test]
    fn test_subtle_equals() {
        assert!(subtle_equals("abc", "abc"));
        assert!(!subtle_equals("abc", "abd"));
        assert!(!subtle_equals("abc", "abcd"));
    }
}