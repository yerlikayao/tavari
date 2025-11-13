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
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MessageBody {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub text: Option<TextContent>,
    pub image: Option<MediaContent>,
    pub interactive: Option<InteractiveResponse>,
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

#[derive(Debug, Deserialize, Serialize)]
pub struct InteractiveResponse {
    #[serde(rename = "type")]
    pub interactive_type: String,
    #[serde(rename = "buttonReply")]
    pub button_reply: Option<ButtonReplyData>,
    #[serde(rename = "listReply")]
    pub list_reply: Option<ListReplyData>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ButtonReplyData {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ListReplyData {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
}

/// Handle incoming webhook from Bird.com
pub async fn handle_bird_webhook(
    handler: Arc<MessageHandler>,
    _bird_client: Arc<BirdComClient>,
    webhook: BirdWebhook,
) -> anyhow::Result<()> {
    log::info!("📨 Received webhook: event={}, id={}", webhook.event, webhook.payload.id);

    let from = &webhook.payload.sender.contact.identifier_value;
    let sender_name = webhook.payload.sender.contact.name.as_deref();

    // Update user's name if provided by WhatsApp
    if let Some(name) = sender_name {
        log::debug!("📝 Updating name for {}: {}", from, name);
        let _ = handler.update_user_name(from, Some(name)).await;
    }

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

                    // Generate output path - use absolute path from /app
                    let data_dir = "/app/data/images";
                    let filename = format!(
                        "{}/img_{}.jpg",
                        data_dir,
                        chrono::Utc::now().timestamp()
                    );

                    // Create directory if not exists
                    log::info!("📁 Ensuring directory exists: {}", data_dir);
                    if let Err(e) = std::fs::create_dir_all(data_dir) {
                        log::error!("❌ Failed to create directory {}: {}", data_dir, e);
                        log::error!("   Current directory: {:?}", std::env::current_dir());
                        log::error!("   Error kind: {:?}", e.kind());

                        // Check parent directory permissions
                        if let Ok(metadata) = std::fs::metadata("/app/data") {
                            log::error!("   /app/data permissions: readonly={}, is_dir={}",
                                metadata.permissions().readonly(), metadata.is_dir());
                        } else {
                            log::error!("   /app/data directory does not exist!");
                        }

                        return Err(e.into());
                    }

                    // Verify directory permissions
                    match std::fs::metadata(data_dir) {
                        Ok(metadata) => {
                            log::info!("✅ Directory {} exists - readonly={}, is_dir={}",
                                data_dir, metadata.permissions().readonly(), metadata.is_dir());
                        }
                        Err(e) => {
                            log::error!("❌ Cannot access {}: {}", data_dir, e);
                            return Err(e.into());
                        }
                    }

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
                    log::info!("💾 Writing {} bytes to: {}", bytes.len(), filename);

                    // Try to write the file
                    if let Err(e) = std::fs::write(&filename, &bytes) {
                        log::error!("❌ Failed to write file {}: {}", filename, e);
                        log::error!("   Error kind: {:?}", e.kind());
                        log::error!("   Bytes to write: {}", bytes.len());

                        // Check directory permissions again
                        if let Ok(metadata) = std::fs::metadata("./data/images") {
                            log::error!("   Directory metadata: readonly={}, is_dir={}",
                                metadata.permissions().readonly(), metadata.is_dir());
                        }

                        // Try to list files in directory
                        match std::fs::read_dir(data_dir) {
                            Ok(entries) => {
                                log::error!("   Files in {}:", data_dir);
                                for entry in entries.flatten() {
                                    log::error!("     - {:?}", entry.path());
                                }
                            }
                            Err(e) => {
                                log::error!("   Cannot read directory: {}", e);
                            }
                        }

                        return Err(e.into());
                    }

                    // Verify file was written
                    match std::fs::metadata(&filename) {
                        Ok(metadata) => {
                            log::info!("✅ Image saved successfully: {} ({} bytes)", filename, metadata.len());
                        }
                        Err(e) => {
                            log::error!("❌ File written but cannot verify: {}", e);
                        }
                    }

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
        "interactive" => {
            if let Some(interactive) = webhook.payload.body.interactive {
                // Check for list reply first (WhatsApp list messages)
                if let Some(list_reply) = interactive.list_reply {
                    log::info!("📋 List selection from {}: id={}, title={}", from, list_reply.id, list_reply.title);

                    // Handle water list selections
                    if list_reply.id.starts_with("water_") {
                        // Extract amount from list ID (e.g., "water_200" -> "200")
                        let amount = list_reply.id.strip_prefix("water_").unwrap_or("0");
                        let water_message = format!("{} ml içtim", amount);
                        log::info!("💧 Processing water list selection: {}", water_message);
                        handler.handle_message(from, &water_message, false, None).await?;
                    } else {
                        // Unknown selection, just handle as text
                        handler.handle_message(from, &list_reply.title, false, None).await?;
                    }
                } else if let Some(button_reply) = interactive.button_reply {
                    log::info!("🔘 Button click from {}: id={}, title={}", from, button_reply.id, button_reply.title);

                    // Handle water button clicks
                    if button_reply.id.starts_with("water_") {
                        // Extract amount from button ID (e.g., "water_200" -> "200")
                        let amount = button_reply.id.strip_prefix("water_").unwrap_or("0");
                        let water_message = format!("{} ml içtim", amount);
                        log::info!("💧 Processing water button: {}", water_message);
                        handler.handle_message(from, &water_message, false, None).await?;
                    } else {
                        // Unknown button, just handle as text
                        handler.handle_message(from, &button_reply.title, false, None).await?;
                    }
                } else {
                    log::warn!("⚠️ Interactive message received but no button/list reply");
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
    let provided_signature = signature.strip_prefix("sha256=").unwrap_or(signature);

    expected_signature == provided_signature
}

// Admin dashboard module
#[cfg(feature = "webhook-server")]
pub mod admin;

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
            // Use raw body for signature verification (not re-serialized JSON)
            // Re-serialization can change whitespace/key ordering and break HMAC verification
            if !verify_webhook_signature(&body, signature, &webhook_secret) {
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
