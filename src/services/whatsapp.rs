#[allow(dead_code)]
use anyhow::Result;
#[allow(dead_code)]
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppMessage {
    pub from: String,
    pub body: String,
    pub has_media: bool,
    pub media_type: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct WhatsAppClient {
    // WhatsApp Web için birkaç seçenek var:
    // 1. whatsmeow (Go library) - en olgun çözüm ama Rust değil
    // 2. baileys (Node.js) wrapper
    // 3. REST API yaklaşımı (WhatsApp Business API)
    // 4. Python whatsapp-web.js bridge

    // Bu örnekte basit bir trait tanımlayacağız
    // Gerçek implementasyon için external service kullanılabilir
}

// WhatsApp istemcisi için trait
#[allow(dead_code)]
#[async_trait::async_trait]
pub trait WhatsAppService: Send + Sync {
    async fn send_message(&self, to: &str, message: &str) -> Result<()>;
    #[allow(dead_code)]
    async fn send_image(&self, to: &str, image_path: &str, caption: &str) -> Result<()>;
    #[allow(dead_code)]
    async fn download_media(&self, message_id: &str, output_path: &str) -> Result<String>;

    /// Send a message with quick reply buttons (WhatsApp interactive messages)
    /// buttons: Vec of (id, title) pairs (max 3 buttons)
    /// Default implementation sends plain text (for clients that don't support buttons)
    async fn send_message_with_buttons(
        &self,
        to: &str,
        message: &str,
        _buttons: Vec<(String, String)>,
    ) -> Result<()> {
        // Default implementation: just send the message without buttons
        self.send_message(to, message).await
    }
}

// Mock implementasyon - gerçek WhatsApp entegrasyonu için değiştirilmeli
#[allow(dead_code)]
pub struct MockWhatsAppClient;

#[async_trait::async_trait]
impl WhatsAppService for MockWhatsAppClient {
    async fn send_message(&self, to: &str, message: &str) -> Result<()> {
        log::info!("📱 Sending message to {}: {}", to, message);
        // Gerçek implementasyon: WhatsApp API çağrısı
        Ok(())
    }

    async fn send_image(&self, to: &str, image_path: &str, caption: &str) -> Result<()> {
        log::info!("📱 Sending image to {}: {} ({})", to, image_path, caption);
        // Gerçek implementasyon: WhatsApp Media API çağrısı
        Ok(())
    }

    async fn download_media(&self, message_id: &str, output_path: &str) -> Result<String> {
        log::info!("📥 Downloading media {} to {}", message_id, output_path);
        // Gerçek implementasyon: WhatsApp Media download
        Ok(output_path.to_string())
    }
}

#[allow(dead_code)]
impl MockWhatsAppClient {
    pub fn new() -> Self {
        Self
    }
}

// WhatsApp Business API Client (gerçek kullanım için)
#[allow(dead_code)]
pub struct WhatsAppBusinessClient {
    api_key: String,
    phone_number_id: String,
    client: reqwest::Client,
}

#[allow(dead_code)]
impl WhatsAppBusinessClient {
    pub fn new(api_key: String, phone_number_id: String) -> Self {
        Self {
            api_key,
            phone_number_id,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl WhatsAppService for WhatsAppBusinessClient {
    async fn send_message(&self, to: &str, message: &str) -> Result<()> {
        let url = format!(
            "https://graph.facebook.com/v18.0/{}/messages",
            self.phone_number_id
        );

        #[derive(Serialize)]
        struct MessageRequest {
            messaging_product: String,
            to: String,
            #[serde(rename = "type")]
            msg_type: String,
            text: TextContent,
        }

        #[derive(Serialize)]
        struct TextContent {
            body: String,
        }

        let request = MessageRequest {
            messaging_product: "whatsapp".to_string(),
            to: to.to_string(),
            msg_type: "text".to_string(),
            text: TextContent {
                body: message.to_string(),
            },
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("WhatsApp API error: {}", error_text);
        }

        Ok(())
    }

    async fn send_image(&self, to: &str, image_path: &str, _caption: &str) -> Result<()> {
        // WhatsApp Business API ile resim gönderme
        log::info!("Sending image via WhatsApp Business API: {} to {}", image_path, to);
        // Implementasyon: önce media upload, sonra message send
        Ok(())
    }

    async fn download_media(&self, message_id: &str, output_path: &str) -> Result<String> {
        // WhatsApp Business API ile media indirme
        log::info!("Downloading media from WhatsApp Business API: {}", message_id);
        Ok(output_path.to_string())
    }
}

pub fn format_daily_report(
    total_calories: f64,
    total_water: i64,
    meals_count: i64,
    water_logs: i64,
    calorie_goal: i32,
    water_goal: i32,
) -> String {
    // Progress bar oluştur
    let calorie_bar = create_progress_bar(total_calories, calorie_goal as f64);
    let water_bar = create_progress_bar(total_water as f64, water_goal as f64);

    format!(
        "📊 *Günlük Rapor*\n\n\
         🔥 Kalori\n\
         {}\n\
         {:.0}/{:.0} kcal ({}%)\n\n\
         💧 Su\n\
         {}\n\
         {}/{} ml ({}%)\n\n\
         🍽️ Öğün Sayısı: {}\n\
         📝 Su Kayıt: {}\n\n\
         {}",
        calorie_bar.bar,
        total_calories,
        calorie_goal,
        calorie_bar.percentage,
        water_bar.bar,
        total_water,
        water_goal,
        water_bar.percentage,
        meals_count,
        water_logs,
        get_motivational_message(total_calories, total_water)
    )
}

struct ProgressBar {
    bar: String,
    percentage: i32,
}

fn create_progress_bar(current: f64, goal: f64) -> ProgressBar {
    let percentage = ((current / goal) * 100.0).min(100.0) as i32;
    let filled = (percentage / 10) as usize; // 10 basamak
    let empty = 10 - filled;

    let bar = format!(
        "{}{}",
        "█".repeat(filled),
        "░".repeat(empty)
    );

    ProgressBar { bar, percentage }
}

fn get_motivational_message(calories: f64, water_ml: i64) -> String {
    let recommended_water = 2000; // 2 litre
    let water_percentage = (water_ml as f64 / recommended_water as f64 * 100.0) as i32;

    if water_percentage >= 100 && (1500.0..=2500.0).contains(&calories) {
        "🎉 Harika! Hem kalori hedefinde hem de su tüketiminde başarılı!"
    } else if water_percentage < 50 {
        "💧 Su tüketimine dikkat et! Daha fazla su içmeyi unutma."
    } else if calories < 1200.0 {
        "🍽️ Kalori alımın düşük. Yeterli beslenmeye dikkat et."
    } else if calories > 3000.0 {
        "⚠️ Kalori alımın yüksek. Porsiyonlara dikkat edebilirsin."
    } else {
        "👍 İyi gidiyorsun! Böyle devam et."
    }
    .to_string()
}
