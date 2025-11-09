use anyhow::Result;
use serde::{Deserialize, Serialize};
use super::WhatsAppService;

/// Bird.com (MessageBird) WhatsApp Business API client
pub struct BirdComClient {
    api_key: String,
    workspace_id: String,
    channel_id: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct BirdMessage {
    receiver: Receiver,
    body: Body,
}

#[derive(Serialize)]
struct Receiver {
    contacts: Vec<Contact>,
}

#[derive(Serialize)]
struct Contact {
    #[serde(rename = "identifierValue")]
    identifier_value: String,
}

#[derive(Serialize)]
#[serde(untagged)]
#[allow(dead_code)]  // Interactive variant reserved for future WhatsApp Template Messages
enum Body {
    Text {
        #[serde(rename = "type")]
        msg_type: String,
        text: TextContent,
    },
    Interactive {
        #[serde(rename = "type")]
        msg_type: String,
        interactive: InteractiveContent,
    },
}

#[derive(Serialize)]
struct TextContent {
    text: String,
}

#[derive(Serialize)]
struct InteractiveContent {
    #[serde(rename = "type")]
    interactive_type: String,
    body: InteractiveBody,
    action: InteractiveAction,
}

#[derive(Serialize)]
struct InteractiveBody {
    text: String,
}

#[derive(Serialize)]
struct InteractiveAction {
    buttons: Vec<InteractiveButton>,
}

#[derive(Serialize)]
struct InteractiveButton {
    #[serde(rename = "type")]
    button_type: String,
    reply: ButtonReply,
}

#[derive(Serialize)]
struct ButtonReply {
    id: String,
    title: String,
}

#[derive(Deserialize)]
struct BirdResponse {
    id: String,
}

impl BirdComClient {
    pub fn new(api_key: String, workspace_id: String, channel_id: String) -> Self {
        Self {
            api_key,
            workspace_id,
            channel_id,
            client: reqwest::Client::new(),
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("https://api.bird.com/workspaces/{}{}", self.workspace_id, path)
    }

    /// Send a message with quick reply buttons (max 3 buttons)
    /// NOTE: Currently disabled - Bird.com requires WhatsApp Template Messages for buttons
    /// Keep this code for future template implementation
    #[allow(dead_code)]
    /// Send a simple text message without buttons
    pub async fn send_message(&self, to: &str, message: &str) -> Result<()> {
        let url = self.api_url(&format!("/channels/{}/messages", self.channel_id));

        let body = serde_json::json!({
            "receiver": {
                "contacts": [{ "identifierValue": to }]
            },
            "body": {
                "type": "text",
                "text": { "text": message }
            }
        });

        let response = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("AccessKey {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to send message: {}", error_text);
        }

        Ok(())
    }

    pub async fn send_message_with_buttons(
        &self,
        to: &str,
        message: &str,
        buttons: Vec<(String, String)>, // (id, title) pairs
    ) -> Result<()> {
        if buttons.is_empty() || buttons.len() > 10 {
            anyhow::bail!("Buttons must be between 1 and 10 items for list");
        }

        let url = self.api_url(&format!("/channels/{}/messages", self.channel_id));

        // Try Bird.com's "list" type for WhatsApp list messages
        let rows: Vec<serde_json::Value> = buttons.iter().map(|(id, title)| {
            serde_json::json!({
                "id": id,
                "title": title,
                "description": ""
            })
        }).collect();

        let payload = serde_json::json!({
            "receiver": {
                "contacts": [{ "identifierValue": to }]
            },
            "body": {
                "type": "list",
                "list": {
                    "header": "Su Kaydı 💧",
                    "body": message,
                    "buttonText": "Seç",
                    "sections": [{
                        "title": "Miktar Seçin",
                        "rows": rows
                    }]
                }
            }
        });

        log::info!("🔍 DEBUG - Sending list message to URL: {}", url);
        log::info!("🔍 DEBUG - Payload: {}", serde_json::to_string_pretty(&payload)?);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("AccessKey {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        log::info!("🔍 DEBUG - Response Status: {}", status);
        log::info!("🔍 DEBUG - Response Body: {}", response_text);

        if !status.is_success() {
            anyhow::bail!("Bird.com API error ({}): {}", status, response_text);
        }

        let result: BirdResponse = serde_json::from_str(&response_text)?;
        log::info!(
            "📤 OUTGOING INTERACTIVE MESSAGE - To: {} | Message ID: {} | Content: '{}' | Buttons: {}",
            to,
            result.id,
            message,
            "(interactive buttons)"
        );

        Ok(())
    }
}

#[async_trait::async_trait]
impl WhatsAppService for BirdComClient {
    async fn send_message_with_buttons(
        &self,
        to: &str,
        message: &str,
        buttons: Vec<(String, String)>,
    ) -> Result<()> {
        // Use the concrete implementation
        self.send_message_with_buttons(to, message, buttons).await
    }

    async fn send_message(&self, to: &str, message: &str) -> Result<()> {
        let url = self.api_url(&format!("/channels/{}/messages", self.channel_id));

        let payload = BirdMessage {
            receiver: Receiver {
                contacts: vec![Contact {
                    identifier_value: to.to_string(),
                }],
            },
            body: Body::Text {
                msg_type: "text".to_string(),
                text: TextContent {
                    text: message.to_string(),
                },
            },
        };

        log::info!("🔍 DEBUG - Sending to URL: {}", url);
        log::info!("🔍 DEBUG - Payload: {}", serde_json::to_string_pretty(&payload)?);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("AccessKey {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        
        log::info!("🔍 DEBUG - Response Status: {}", status);
        log::info!("🔍 DEBUG - Response Body: {}", response_text);

        if !status.is_success() {
            anyhow::bail!("Bird.com API error ({}): {}", status, response_text);
        }

        let result: BirdResponse = serde_json::from_str(&response_text)?;
        log::info!("📤 OUTGOING MESSAGE - To: {} | Message ID: {} | Content: '{}'",
                   to, result.id, message);

        Ok(())
    }

    async fn send_image(&self, to: &str, image_path: &str, caption: &str) -> Result<()> {
        // Bird.com media workflow:
        // 1. Upload image to Bird.com media endpoint
        // 2. Get media URL/ID
        // 3. Send message with media reference

        log::info!("📸 Sending image via Bird.com: {} to {} with caption: {}",
                   image_path, to, caption);

        // For now, send caption as text message
        // TODO: Implement proper media upload
        self.send_message(to, &format!("📷 [Image: {}]\n{}", image_path, caption)).await?;

        Ok(())
    }

    async fn download_media(&self, message_id: &str, output_path: &str) -> Result<String> {
        // Bird.com media download
        // GET /workspaces/{workspaceId}/messages/{messageId}/media

        log::info!("📥 Downloading media from Bird.com: message_id={}", message_id);

        let url = self.api_url(&format!("/messages/{}/media", message_id));

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("AccessKey {}", self.api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("Bird.com media download error ({}): {}", status, error_text);
        }

        // Save to file
        let bytes = response.bytes().await?;
        std::fs::write(output_path, bytes)?;

        log::info!("✅ Media downloaded to: {}", output_path);
        Ok(output_path.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bird_client_creation() {
        let client = BirdComClient::new(
            "test_key".to_string(),
            "workspace_123".to_string(),
            "channel_456".to_string(),
        );

        assert_eq!(client.workspace_id, "workspace_123");
        assert_eq!(client.channel_id, "channel_456");
    }

    #[test]
    fn test_api_url_generation() {
        let client = BirdComClient::new(
            "test_key".to_string(),
            "workspace_123".to_string(),
            "channel_456".to_string(),
        );

        let url = client.api_url("/channels/channel_456/messages");
        assert_eq!(url, "https://api.bird.com/workspaces/workspace_123/channels/channel_456/messages");
    }
}
