use serde::{Deserialize, Serialize};
use std::sync::Arc;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::handlers::MessageHandler;
use crate::services::bird::BirdComClient;

/// Bird.com webhook payload structures (whatsapp.inbound format)
#[derive(Debug, Deserialize, Serialize)]
pub struct BirdWebhook {
    pub service: String,
    pub event: String,
    pub payload: WebhookPayload,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WebhookPayload {
    pub id: String,
    #[serde(rename = "channelId")]
    pub channel_id: String,
    pub sender: Sender,
    pub body: MessageBody,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Sender {
    pub contact: Contact,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Contact {
    #[serde(rename = "identifierValue")]
    pub identifier_value: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MessageBody {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub text: Option<TextContent>,
    pub image: Option<MediaContent>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TextContent {
    pub text: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MediaContent {
    pub images: Vec<ImageData>,
    pub caption: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ImageData {
    #[serde(rename = "mediaUrl")]
    pub media_url: String,
}

/// Handle incoming webhook from Bird.com
pub async fn handle_bird_webhook(
    handler: Arc<MessageHandler>,
    _bird_client: Arc<BirdComClient>,
    webhook: BirdWebhook,
) -> anyhow::Result<()> {
    log::info!("📨 Received webhook: event={}, id={}", webhook.event, webhook.payload.id);

    let from = &webhook.payload.sender.contact.identifier_value;

    match webhook.payload.body.msg_type.as_str() {
        "text" => {
            if let Some(text) = webhook.payload.body.text {
                log::info!("💬 Text message from {}: {}", from, text.text);
                handler.handle_message(from, &text.text, false, None).await?;
            }
        }
        "image" => {
            if let Some(image) = webhook.payload.body.image {
                if let Some(first_image) = image.images.first() {
                    log::info!("📸 Image message from {}: mediaUrl={}", from, first_image.media_url);

                    // Generate output path
                    let filename = format!(
                        "./data/images/img_{}.jpg",
                        chrono::Utc::now().timestamp()
                    );
                    
                    // Create directory if not exists
                    std::fs::create_dir_all("./data/images")?;

                    // Download directly from mediaUrl with AccessKey authentication (redirects enabled)
                    let client = reqwest::Client::builder()
                        .redirect(reqwest::redirect::Policy::limited(10))
                        .build()?;
                    let response = client
                        .get(&first_image.media_url)
                        .header("Authorization", format!("AccessKey {}", std::env::var("BIRD_API_KEY").unwrap_or_default()))
                        .send()
                        .await?;

                    if !response.status().is_success() {
                        anyhow::bail!("Failed to download image from mediaUrl: HTTP {}", response.status());
                    }

                    let bytes = response.bytes().await?;
                    std::fs::write(&filename, bytes)?;
                    log::info!("✅ Image downloaded from mediaUrl: {}", filename);

                    // Handle with caption if present
                    let caption = image.caption.as_deref().unwrap_or("");
                    handler
                        .handle_message(from, caption, true, Some(filename))
                        .await?;
                } else {
                    log::warn!("⚠️ Image message received but no images in array");
                }
            }
        }
        _ => {
            log::warn!("⚠️ Unknown message type: {}", webhook.payload.body.msg_type);
        }
    }

    Ok(())
}

/// Verify webhook signature using HMAC-SHA256
fn verify_webhook_signature(payload: &str, signature: &str, secret: &str) -> bool {
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(mac) => mac,
        Err(_) => return false,
    };

    mac.update(payload.as_bytes());
    let result = mac.finalize();
    let expected_signature = hex::encode(result.into_bytes());

    // Bird.com typically sends signature as "sha256=<signature>"
    let provided_signature = if signature.starts_with("sha256=") {
        &signature[7..] // Remove "sha256=" prefix
    } else {
        signature
    };

    expected_signature == provided_signature
}

// Axum integration (optional - requires axum dependency)
#[cfg(feature = "webhook-server")]
pub mod server {
    use super::*;
    use axum::{
        extract::State,
        http::StatusCode,
        routing::{get, post},
        Router,
    };

    pub struct AppState {
        pub message_handler: Arc<MessageHandler>,
        pub bird_client: Arc<BirdComClient>,
    }

    pub fn create_webhook_router(
        message_handler: Arc<MessageHandler>,
        bird_client: Arc<BirdComClient>,
    ) -> Router {
        let state = Arc::new(AppState {
            message_handler,
            bird_client,
        });

        Router::new()
            .route("/", get(root_handler))
            .route("/webhook/whatsapp", post(webhook_handler))
            .route("/health", get(health_check))
            .with_state(state)
    }

    async fn webhook_handler(
        headers: axum::http::HeaderMap,
        State(state): State<Arc<AppState>>,
        body: String,
    ) -> StatusCode {
        log::info!("🔔 Webhook received: {}", &body[..body.len().min(500)]);
        
        // Try to parse the payload
        let payload: BirdWebhook = match serde_json::from_str(&body) {
            Ok(p) => p,
            Err(e) => {
                log::error!("❌ Failed to parse webhook payload: {}", e);
                log::error!("📦 Raw payload: {}", body);
                return StatusCode::UNPROCESSABLE_ENTITY;
            }
        };
        
        log::info!("✅ Parsed webhook: {} (event: {})", payload.payload.id, payload.event);

        // Log all headers for debugging
        for (name, value) in headers.iter() {
            if name.as_str().to_lowercase().contains("signature") ||
               name.as_str().to_lowercase().contains("hub") {
                log::debug!("Header: {} = {:?}", name, value);
            }
        }

        // Verify webhook signature for security
        let webhook_secret = match std::env::var("BIRD_WEBHOOK_SECRET") {
            Ok(secret) => {
                log::debug!("Using webhook secret: {}...", &secret[..8]);
                secret
            }
            Err(_) => {
                log::warn!("⚠️ BIRD_WEBHOOK_SECRET not configured, skipping signature verification");
                String::new()
            }
        };

        // Get signature from headers (Bird.com typically sends X-Hub-Signature-256 or similar)
        let signature = if let Some(sig_header) = headers.get("x-hub-signature-256") {
            sig_header.to_str().unwrap_or("")
        } else if let Some(sig_header) = headers.get("x-signature") {
            sig_header.to_str().unwrap_or("")
        } else if let Some(sig_header) = headers.get("signature") {
            sig_header.to_str().unwrap_or("")
        } else {
            log::info!("ℹ️ No signature header found in webhook request (expected for testing)");
            ""
        };

        // Verify signature if both secret and signature are provided
        if !webhook_secret.is_empty() && !signature.is_empty() {
            // Serialize payload for signature verification
            let payload_json = match serde_json::to_string(&payload) {
                Ok(json) => json,
                Err(e) => {
                    log::error!("❌ Failed to serialize payload: {}", e);
                    return StatusCode::BAD_REQUEST;
                }
            };

            if !verify_webhook_signature(&payload_json, signature, &webhook_secret) {
                log::error!("❌ Webhook signature verification failed");
                return StatusCode::UNAUTHORIZED;
            }

            log::info!("✅ Webhook signature verified");
        } else if !signature.is_empty() {
            log::warn!("⚠️ Signature provided but no webhook secret configured");
        }

        // Process the webhook
        match handle_bird_webhook(state.message_handler.clone(), state.bird_client.clone(), payload).await {
            Ok(_) => {
                log::info!("✅ Webhook processed successfully");
                StatusCode::OK
            }
            Err(e) => {
                // Log the error but don't fail the webhook - Bird.com expects 200
                log::error!("❌ Webhook processing error: {}", e);
                // Return OK to prevent Bird.com from retrying
                StatusCode::OK
            }
        }
    }

    async fn root_handler() -> &'static str {
        "WhatsApp Nutrition Bot Webhook Server - Use /webhook/whatsapp for Bird.com webhooks"
    }

    async fn health_check() -> &'static str {
        "OK"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_deserialization() {
        let json = r#"{
            "service": "channels",
            "event": "whatsapp.inbound",
            "payload": {
                "id": "msg_123",
                "channelId": "channel_456",
                "sender": {
                    "contact": {
                        "identifierValue": "+905551234567"
                    }
                },
                "body": {
                    "type": "text",
                    "text": {
                        "text": "Merhaba"
                    }
                }
            }
        }"#;

        let webhook: BirdWebhook = serde_json::from_str(json).unwrap();

        assert_eq!(webhook.service, "channels");
        assert_eq!(webhook.event, "whatsapp.inbound");
        assert_eq!(webhook.payload.id, "msg_123");
        assert_eq!(webhook.payload.sender.contact.identifier_value, "+905551234567");
        assert_eq!(webhook.payload.body.msg_type, "text");
        assert_eq!(
            webhook.payload.body.text.unwrap().text,
            "Merhaba"
        );
    }
}
