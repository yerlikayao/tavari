use anyhow::Result;
use base64::{engine::general_purpose, Engine};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: Vec<ContentPart>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ContentPart {
    Text {
        #[serde(rename = "type")]
        content_type: String,
        text: String
    },
    ImageUrl {
        #[serde(rename = "type")]
        content_type: String,
        image_url: ImageData
    },
}

#[derive(Debug, Serialize)]
struct ImageData {
    url: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Debug, Deserialize)]
struct MessageContent {
    content: String,
}

#[derive(Debug, Clone)]
pub struct CalorieInfo {
    pub calories: f64,
    pub description: String,
}

pub struct OpenRouterService {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenRouterService {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }

    pub async fn analyze_food_image(&self, image_path: &str) -> Result<CalorieInfo> {
        log::debug!("📸 Starting image analysis for: {}", image_path);

        // Resmi base64'e çevir
        let image_data = fs::read(image_path)?;
        let base64_image = general_purpose::STANDARD.encode(&image_data);

        log::debug!("📊 Image file size: {} bytes", image_data.len());
        log::debug!("🔄 Base64 encoded size: {} bytes", base64_image.len());

        // Dosya uzantısından MIME type belirle
        let mime_type = if image_path.ends_with(".png") {
            "image/png"
        } else if image_path.ends_with(".jpg") || image_path.ends_with(".jpeg") {
            "image/jpeg"
        } else {
            "image/jpeg" // varsayılan
        };

        let data_url = format!("data:{};base64,{}", mime_type, base64_image);
        log::debug!("🖼️ Image data URL created: {}... (first 100 chars)", &data_url[..100.min(data_url.len())]);

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: vec![
                ContentPart::Text {
                    content_type: "text".to_string(),
                    text: "SEN BİR GIDA ANALİZİ UZMANISIN. Bu yemek resmini çok dikkatli incele ve aşağıdaki adımları takip et:\n\
                           \n\
                           1. YEMEK TANIMA:\n\
                           - Resimde ne yemekler var? (ana yemek, yan yemekler, içecekler)\n\
                           - Yemek türünü doğru belirle (Türk mutfağı, fast food, sağlıklı yemek vb.)\n\
                           - Farklı malzemeleri ayrı ayrı tanı\n\
                           \n\
                           2. KALORİ HESAPLAMA:\n\
                           - Porsiyon büyüklüğünü görsel olarak değerlendir\n\
                           - Her bileşen için gerçekçi kalori tahmini yap\n\
                           - Toplam kaloriyi hesapla\n\
                           \n\
                           3. BESLENME ANALİZİ:\n\
                           - Sağlıklı mı değil mi? (sebze, protein, yağ oranı)\n\
                           - Beslenme değeri (protein, karbonhidrat, yağ)\n\
                           - Beslenme tavsiyesi\n\
                           \n\
                           4. ÖZELLİKLER:\n\
                           - Pişirme yöntemi (ızgara, kızartma, haşlama vb.)\n\
                           - Malzeme kalitesi\n\
                           - Beslenme dengesi\n\
                           \n\
                           CEVAP FORMATI (KESİNLİKLE BU FORMATI KULLAN):\n\
                           Yemek: [ana yemek adı + yan yemekler]\n\
                           Kalori: [sadece sayı, kcal birimi olmadan]\n\
                           Açıklama: [detaylı analiz: ne olduğu, sağlıklı mı, beslenme değeri, porsiyon]".to_string(),
                },
                ContentPart::ImageUrl {
                    content_type: "image_url".to_string(),
                    image_url: ImageData {
                        url: data_url,
                    },
                },
            ],
        }];

        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            max_tokens: 500,
        };

        log::info!("🤖 Sending request to OpenRouter with model: {}", self.model);
        log::debug!("📤 Request payload size: {} bytes", serde_json::to_string(&request)?.len());

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://github.com/tavari-bot") // OpenRouter için gerekli
            .header("X-Title", "Tavari Nutrition Bot") // OpenRouter için opsiyonel
            .json(&request)
            .send()
            .await?;

        log::debug!("📥 OpenRouter response status: {}", response.status());

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            log::error!("❌ OpenRouter API error response: {}", error_text);
            anyhow::bail!("OpenRouter API error ({}): {}", status, error_text);
        }

        let response_text = response.text().await?;
        log::debug!("📄 Raw OpenRouter response size: {} bytes", response_text.len());

        let chat_response: ChatResponse = serde_json::from_str(&response_text)?;
        log::debug!("✅ Parsed OpenRouter response successfully");

        let content = &chat_response.choices[0].message.content;
        log::info!("💬 OpenRouter response content: {}", content);

        // Parse the response
        let calorie_info = self.parse_response(content)?;

        Ok(calorie_info)
    }

    fn parse_response(&self, response: &str) -> Result<CalorieInfo> {
        let mut calories = 0.0;
        let mut description = String::new();

        for line in response.lines() {
            if line.starts_with("Kalori:") {
                let calorie_str = line
                    .replace("Kalori:", "")
                    .trim()
                    .replace("kcal", "")
                    .replace("cal", "")
                    .trim()
                    .to_string();

                // Sayıyı çıkar (virgül veya nokta içerebilir)
                let cleaned = calorie_str
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
                    .collect::<String>()
                    .replace(',', ".");

                calories = cleaned.parse::<f64>().unwrap_or(0.0);
            } else if line.starts_with("Yemek:") || line.starts_with("Açıklama:") {
                description.push_str(line.trim());
                description.push('\n');
            }
        }

        if calories == 0.0 {
            // Eğer parse edilemezse, tüm metni açıklama olarak al ve ortalama bir değer ver
            description = response.to_string();
            log::warn!("Could not parse calories from response, using default");
            // Varsayılan orta büyüklük öğün kalorisi
            calories = 400.0;
        }

        Ok(CalorieInfo {
            calories,
            description: description.trim().to_string(),
        })
    }

    pub async fn get_nutrition_advice(&self, daily_calories: f64, daily_water: i64) -> Result<String> {
        log::info!("🤖 Requesting nutrition advice for {} kcal, {} ml water", daily_calories, daily_water);

        // Kategorize calorie and water intake to avoid triggering Meta moderation
        let calorie_level = if daily_calories < 1200.0 {
            "düşük enerji alımı"
        } else if daily_calories < 2000.0 {
            "orta düzeyde enerji alımı"
        } else if daily_calories < 2500.0 {
            "iyi düzeyde enerji alımı"
        } else {
            "yüksek enerji alımı"
        };

        let water_level = if daily_water < 1000 {
            "az su tüketimi"
        } else if daily_water < 2000 {
            "orta düzeyde su tüketimi"
        } else if daily_water < 3000 {
            "iyi düzeyde su tüketimi"
        } else {
            "yüksek su tüketimi"
        };

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: vec![ContentPart::Text {
                content_type: "text".to_string(),
                text: format!(
                    "Bir kullanıcı bugün beslenme takibini yapıyor: {} ve {}. \
                     Genel sağlıklı beslenme için kısa, pozitif ve motive edici bir öneri ver (2-3 cümle). \
                     Kullanıcıyı destekleyici ol.",
                    calorie_level, water_level
                ),
            }],
        }];

        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            max_tokens: 200,
        };

        log::info!("📤 Sending request to OpenRouter with model: {}", self.model);

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://github.com/tavari-bot")
            .header("X-Title", "Tavari Nutrition Bot")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        log::info!("📥 OpenRouter response status: {}", status);

        if !status.is_success() {
            let error_text = response.text().await?;
            log::error!("❌ OpenRouter API error ({}): {}", status, error_text);
            anyhow::bail!("OpenRouter API error ({}): {}", status, error_text);
        }

        let chat_response: ChatResponse = response.json().await?;
        log::info!("✅ Received nutrition advice successfully");
        Ok(chat_response.choices[0].message.content.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response() {
        let service = OpenRouterService::new(
            "test_key".to_string(),
            "test_model".to_string(),
        );

        let response = "Yemek: Pizza Margherita\nKalori: 650\nAçıklama: Orta boy pizza, 2 dilim";
        let info = service.parse_response(response).unwrap();

        assert_eq!(info.calories, 650.0);
        assert!(info.description.contains("Pizza"));
    }

    #[test]
    fn test_parse_response_with_comma() {
        let service = OpenRouterService::new(
            "test_key".to_string(),
            "test_model".to_string(),
        );

        let response = "Kalori: 1,250 kcal\nYemek: Menemen";
        let info = service.parse_response(response).unwrap();

        assert_eq!(info.calories, 1250.0);
    }
}
