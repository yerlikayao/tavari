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
                    text: "Sen bir gıda analizi uzmanısın. Bu yemek resmini analiz et ve kullanıcıya detaylı bilgi ver.\n\
                           \n\
                           ANALİZ ADIMLARI:\n\
                           1. Yemekleri tanı (ana yemek, yan yemekler, içecekler)\n\
                           2. Porsiyon büyüklüğünü değerlendir\n\
                           3. Toplam kaloriyi hesapla\n\
                           4. Beslenme değerini analiz et (protein, karbonhidrat, yağ)\n\
                           5. Sağlık açısından değerlendir\n\
                           \n\
                           CEVAP FORMATI (KESİNLİKLE BU FORMATI KULLAN):\n\
                           Yemek: [yemek adı ve bileşenler]\n\
                           Kalori: [sadece sayı - kcal birimi YAZMA]\n\
                           Porsiyon: [büyüklük açıklaması]\n\
                           Besin Değeri: [protein/karbonhidrat/yağ dengesi]\n\
                           Sağlık Notu: [sağlıklı mı, iyileştirme önerileri]\n\
                           \n\
                           ÖNEMLİ:\n\
                           - Markdown kullanma (**, ###, __, vb. YASAK)\n\
                           - Sadece düz metin kullan\n\
                           - Her satır net ve kısa olsun\n\
                           - Kalori satırında SADECE SAYI yaz (örn: Kalori: 650)\n\
                           - Emoji kullanabilirsin ama az kullan\n\
                           \n\
                           ÖRNEK CEVAP:\n\
                           Yemek: Izgara tavuk göğsü, pilav, salata\n\
                           Kalori: 520\n\
                           Porsiyon: Orta büyüklük, yaklaşık 350g\n\
                           Besin Değeri: Yüksek protein, orta karbonhidrat, düşük yağ\n\
                           Sağlık Notu: Dengeli ve sağlıklı bir öğün. Salata miktarını arttırabilirsiniz.".to_string(),
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

        let status = response.status();
        log::info!("📥 OpenRouter response status: {}", status);

        if !status.is_success() {
            let error_text = response.text().await?;
            log::error!("❌ OpenRouter API error response: {}", error_text);

            // Provide more specific error messages
            if status == 429 {
                anyhow::bail!("Rate limit exceeded for OpenRouter API. Free model '{}' may have usage limits.", self.model);
            } else if status == 401 {
                anyhow::bail!("OpenRouter API authentication failed. Check API key.");
            } else if status == 403 {
                // Check if it's a moderation error
                if error_text.contains("moderation") || error_text.contains("flagged") {
                    log::error!("❌ Content moderation false positive: {}", error_text);
                    anyhow::bail!("Content moderation error - AI provider blocked the request. This is likely a false positive.");
                } else {
                    anyhow::bail!("OpenRouter API access forbidden (403): {}", error_text);
                }
            } else if status == 503 {
                anyhow::bail!("OpenRouter service unavailable. Model '{}' may be temporarily down.", self.model);
            } else {
                anyhow::bail!("OpenRouter API error ({}): {}", status, error_text);
            }
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

    /// Markdown ve özel karakterleri temizle
    fn clean_markdown(&self, text: &str) -> String {
        text
            // Markdown başlıkları: ###, ##, # -> kaldır
            .replace("###", "")
            .replace("##", "")
            .replace("# ", "")
            // Markdown bold/italic: **, *, __ -> kaldır
            .replace("**", "")
            .replace("__", "")
            // Markdown liste işaretleri: -, * -> koru (beslenme listelerinde yararlı)
            // Markdown kod blokları: ``` -> kaldır
            .replace("```", "")
            // Markdown bağlantılar: [text](url) -> text
            .lines()
            .map(|line| {
                // [text](url) formatını temizle
                if line.contains('[') && line.contains("](") {
                    let mut cleaned = line.to_string();
                    while let Some(start) = cleaned.find('[') {
                        if let Some(middle) = cleaned[start..].find("](") {
                            if let Some(end) = cleaned[start + middle..].find(')') {
                                let text_start = start + 1;
                                let text_end = start + middle;
                                let link_end = start + middle + end + 1;
                                let text = &cleaned[text_start..text_end];
                                cleaned = format!("{}{}{}",
                                    &cleaned[..start],
                                    text,
                                    &cleaned[link_end..]);
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    cleaned
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            // Fazla boşlukları temizle
            .trim()
            .to_string()
    }

    fn parse_response(&self, response: &str) -> Result<CalorieInfo> {
        let mut calories = 0.0;
        let mut description = String::new();

        for line in response.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with("Kalori:") {
                let calorie_str = trimmed
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
                    .collect::<String>();

                // Virgül ve nokta yönetimi:
                // - Türkçe format: "1.250" (nokta binlik ayracı) -> "1250"
                // - İngilizce format: "1,250" (virgül binlik ayracı) -> "1250"
                // - Ondalık: "1.5" veya "1,5" -> "1.5"
                let final_str = if cleaned.contains(',') && cleaned.contains('.') {
                    // İkisi de varsa, son olanı ondalık ayracı olarak al
                    let comma_pos = cleaned.rfind(',').unwrap_or(0);
                    let dot_pos = cleaned.rfind('.').unwrap_or(0);
                    if comma_pos > dot_pos {
                        // Virgül sonra geliyorsa, virgül ondalık ayracı
                        cleaned.replace('.', "").replace(',', ".")
                    } else {
                        // Nokta sonra geliyorsa, nokta ondalık ayracı
                        cleaned.replace(',', "")
                    }
                } else if cleaned.contains(',') {
                    // Sadece virgül varsa, binlik ayracı olarak kabul et ve kaldır
                    // Ama eğer virgülden sonra 1-2 rakam varsa ondalık ayracıdır
                    if let Some(comma_pos) = cleaned.find(',') {
                        let after_comma = &cleaned[comma_pos + 1..];
                        if after_comma.len() <= 2 && !after_comma.is_empty() {
                            // Ondalık ayracı: "650,5" -> "650.5"
                            cleaned.replace(',', ".")
                        } else {
                            // Binlik ayracı: "1,250" -> "1250"
                            cleaned.replace(',', "")
                        }
                    } else {
                        cleaned
                    }
                } else if cleaned.contains('.') {
                    // Sadece nokta varsa, binlik ayracı olarak kabul et ve kaldır
                    // Ama eğer noktadan sonra 1-2 rakam varsa ondalık ayracıdır
                    if let Some(dot_pos) = cleaned.find('.') {
                        let after_dot = &cleaned[dot_pos + 1..];
                        if after_dot.len() <= 2 && !after_dot.is_empty() {
                            // Ondalık ayracı: "650.5" -> "650.5"
                            cleaned
                        } else {
                            // Binlik ayracı: "1.250" -> "1250"
                            cleaned.replace('.', "")
                        }
                    } else {
                        cleaned
                    }
                } else {
                    cleaned
                };

                calories = final_str.parse::<f64>().unwrap_or(0.0);
            } else {
                // TÜM satırları description'a ekle (sadece Kalori: satırı hariç)
                // Bu sayede Yemek:, Açıklama:, Porsiyon:, Sağlıklı mı: vb. tüm bilgiler korunur
                description.push_str(trimmed);
                description.push('\n');
            }
        }

        if calories == 0.0 {
            // Eğer parse edilemezse, tüm metni açıklama olarak al ve ortalama bir değer ver
            description = response.to_string();
            log::warn!("⚠️ Could not parse calories from response, using default 400 kcal");
            log::debug!("📄 Original AI response: {}", response);
            // Varsayılan orta büyüklük öğün kalorisi
            calories = 400.0;
        }

        // Markdown ve özel karakterleri temizle
        let clean_description = self.clean_markdown(&description);

        Ok(CalorieInfo {
            calories,
            description: clean_description,
        })
    }

    pub async fn analyze_text_meal(&self, meal_description: &str) -> Result<CalorieInfo> {
        log::info!("📝 Analyzing text meal description: {}", meal_description);

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: vec![ContentPart::Text {
                content_type: "text".to_string(),
                text: format!(
                    "Sen bir gıda analizi uzmanısın. Kullanıcının yazdığı yemek açıklamasını analiz et.\n\
                     \n\
                     KULLANICININ YAZDIĞI: \"{}\"\n\
                     \n\
                     GÖREVİN:\n\
                     1. Yemeği/yemekleri tanımla\n\
                     2. Porsiyon büyüklüğünü tahmin et\n\
                     3. Toplam kaloriyi hesapla\n\
                     4. Beslenme değerini değerlendir\n\
                     \n\
                     CEVAP FORMATI (KESİNLİKLE BU FORMATI KULLAN):\n\
                     Yemek: [yemek adı ve bileşenler]\n\
                     Kalori: [sadece sayı - kcal birimi YAZMA]\n\
                     Porsiyon: [büyüklük tahmini]\n\
                     Besin Değeri: [protein/karbonhidrat/yağ dengesi]\n\
                     Sağlık Notu: [kısa değerlendirme]\n\
                     \n\
                     ÖNEMLİ:\n\
                     - Markdown kullanma (**, ###, __, vb. YASAK)\n\
                     - Sadece düz metin kullan\n\
                     - Kalori satırında SADECE SAYI yaz\n\
                     - Porsiyon bilgisi verilmediyse ortalama bir porsiyon varsay\n\
                     \n\
                     ÖRNEK:\n\
                     Yemek: Izgara tavuk göğsü, salata\n\
                     Kalori: 350\n\
                     Porsiyon: Orta büyüklük (tahmini 250g)\n\
                     Besin Değeri: Yüksek protein, düşük karbonhidrat\n\
                     Sağlık Notu: Hafif ve sağlıklı bir öğün",
                    meal_description
                ),
            }],
        }];

        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            max_tokens: 300,
        };

        log::info!("🤖 Sending text meal analysis request to OpenRouter with model: {}", self.model);

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
            log::error!("❌ OpenRouter API error response: {}", error_text);

            // Provide more specific error messages
            if status == 429 {
                anyhow::bail!("Rate limit exceeded for OpenRouter API. Free model '{}' may have usage limits.", self.model);
            } else if status == 401 {
                anyhow::bail!("OpenRouter API authentication failed. Check API key.");
            } else if status == 403 {
                // Check if it's a moderation error
                if error_text.contains("moderation") || error_text.contains("flagged") {
                    log::error!("❌ Content moderation false positive: {}", error_text);
                    anyhow::bail!("Content moderation error - AI provider blocked the request. This is likely a false positive.");
                } else {
                    anyhow::bail!("OpenRouter API access forbidden (403): {}", error_text);
                }
            } else if status == 503 {
                anyhow::bail!("OpenRouter service unavailable. Model '{}' may be temporarily down.", self.model);
            } else {
                anyhow::bail!("OpenRouter API error ({}): {}", status, error_text);
            }
        }

        let response_text = response.text().await?;
        log::debug!("📄 Raw OpenRouter response size: {} bytes", response_text.len());

        let chat_response: ChatResponse = serde_json::from_str(&response_text)?;
        log::debug!("✅ Parsed OpenRouter response successfully");

        let content = &chat_response.choices[0].message.content;
        log::info!("💬 OpenRouter text meal analysis: {}", content);

        // Parse the response
        let calorie_info = self.parse_response(content)?;

        Ok(calorie_info)
    }

    pub async fn get_nutrition_advice(&self, daily_calories: f64, daily_water: i64, water_goal: i32, meals_count: i64) -> Result<String> {
        log::info!("🤖 Requesting nutrition advice for {} kcal, {} ml water, {} meals", daily_calories, daily_water, meals_count);

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: vec![ContentPart::Text {
                content_type: "text".to_string(),
                text: format!(
                    "You are a wellness coach. Provide brief encouraging feedback in Turkish about daily progress.\n\
                     \n\
                     Data: {} kcal, {} meals, {} ml water (goal: {} ml)\n\
                     \n\
                     Write 3-4 short sentences in Turkish. Use actual numbers. Be positive. No markdown. Start sentences with emoji.\n\
                     \n\
                     Example:\n\
                     🎯 Bugun 1500 kcal aldiniz, gayet iyi.\n\
                     💧 Su hedefinize 700 ml kaldi.\n\
                     ✨ Devam edin!",
                    daily_calories,
                    meals_count,
                    daily_water,
                    water_goal
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

            // Provide more specific error messages
            if status == 429 {
                anyhow::bail!("Rate limit exceeded for OpenRouter API. Free model '{}' may have usage limits.", self.model);
            } else if status == 401 {
                anyhow::bail!("OpenRouter API authentication failed. Check API key.");
            } else if status == 403 {
                // Check if it's a moderation error
                if error_text.contains("moderation") || error_text.contains("flagged") {
                    log::error!("❌ Content moderation false positive: {}", error_text);
                    anyhow::bail!("Content moderation error - AI provider blocked the request. This is likely a false positive.");
                } else {
                    anyhow::bail!("OpenRouter API access forbidden (403): {}", error_text);
                }
            } else if status == 503 {
                anyhow::bail!("OpenRouter service unavailable. Model '{}' may be temporarily down.", self.model);
            } else {
                anyhow::bail!("OpenRouter API error ({}): {}", status, error_text);
            }
        }

        let chat_response: ChatResponse = response.json().await?;
        log::info!("✅ Received nutrition advice response");
        log::debug!("📋 Response: {:?}", chat_response);

        // Validate response has choices
        if chat_response.choices.is_empty() {
            log::error!("❌ OpenRouter returned empty choices array");
            anyhow::bail!("OpenRouter returned empty response");
        }

        // Markdown ve özel karakterleri temizle
        let advice = &chat_response.choices[0].message.content;
        log::info!("✅ Nutrition advice content length: {} chars", advice.len());
        let clean_advice = self.clean_markdown(advice);

        Ok(clean_advice)
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
        assert!(info.description.contains("Açıklama"));
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
        assert!(info.description.contains("Menemen"));
    }

    #[test]
    fn test_parse_response_with_decimal_comma() {
        let service = OpenRouterService::new(
            "test_key".to_string(),
            "test_model".to_string(),
        );

        let response = "Kalori: 650,5\nYemek: Salata";
        let info = service.parse_response(response).unwrap();

        assert_eq!(info.calories, 650.5);
    }

    #[test]
    fn test_parse_response_with_decimal_dot() {
        let service = OpenRouterService::new(
            "test_key".to_string(),
            "test_model".to_string(),
        );

        let response = "Kalori: 650.5\nYemek: Salata";
        let info = service.parse_response(response).unwrap();

        assert_eq!(info.calories, 650.5);
    }

    #[test]
    fn test_parse_response_turkish_format() {
        let service = OpenRouterService::new(
            "test_key".to_string(),
            "test_model".to_string(),
        );

        // Türkçe format: 1.250 (binlik ayracı nokta)
        let response = "Kalori: 1.250\nYemek: Köfte";
        let info = service.parse_response(response).unwrap();

        assert_eq!(info.calories, 1250.0);
    }

    #[test]
    fn test_parse_response_with_all_fields() {
        let service = OpenRouterService::new(
            "test_key".to_string(),
            "test_model".to_string(),
        );

        let response = "Yemek: Pizza Margherita\n\
                        Kalori: 650\n\
                        Porsiyon: Orta boy, 2 dilim\n\
                        Sağlıklı mı: Orta düzeyde\n\
                        Açıklama: İyi bir öğün";
        let info = service.parse_response(response).unwrap();

        assert_eq!(info.calories, 650.0);
        // Tüm alanlar description'da olmalı
        assert!(info.description.contains("Pizza"));
        assert!(info.description.contains("Porsiyon"));
        assert!(info.description.contains("Sağlıklı"));
        assert!(info.description.contains("Açıklama"));
        // Kalori satırı description'da olmamalı
        assert!(!info.description.contains("Kalori: 650"));
    }

    #[test]
    fn test_clean_markdown() {
        let service = OpenRouterService::new(
            "test_key".to_string(),
            "test_model".to_string(),
        );

        // Markdown başlıkları
        let text = "### Ana Başlık\n## Alt Başlık\n# Başlık";
        let cleaned = service.clean_markdown(text);
        assert!(!cleaned.contains("###"));
        assert!(!cleaned.contains("##"));
        assert!(!cleaned.contains("# "));

        // Bold ve italic
        let text = "**Kalın metin** ve __diğer kalın__";
        let cleaned = service.clean_markdown(text);
        assert!(!cleaned.contains("**"));
        assert!(!cleaned.contains("__"));
        assert!(cleaned.contains("Kalın metin"));

        // Kod blokları
        let text = "```rust\nlet x = 5;\n```";
        let cleaned = service.clean_markdown(text);
        assert!(!cleaned.contains("```"));

        // Bağlantılar
        let text = "Bu bir [link](https://example.com) örneği";
        let cleaned = service.clean_markdown(text);
        assert!(!cleaned.contains("]("));
        assert!(cleaned.contains("link"));
        assert!(!cleaned.contains("https://example.com"));
    }

    #[test]
    fn test_parse_response_with_markdown() {
        let service = OpenRouterService::new(
            "test_key".to_string(),
            "test_model".to_string(),
        );

        let response = "Yemek: **Pizza Margherita**\n\
                        Kalori: 650\n\
                        ### Beslenme Bilgisi\n\
                        Açıklama: __Orta boy__ pizza, [Detay](https://example.com)";
        let info = service.parse_response(response).unwrap();

        assert_eq!(info.calories, 650.0);
        // Markdown karakterleri temizlenmiş olmalı
        assert!(!info.description.contains("**"));
        assert!(!info.description.contains("###"));
        assert!(!info.description.contains("__"));
        assert!(!info.description.contains("]("));
        // İçerik korunmuş olmalı
        assert!(info.description.contains("Pizza"));
        assert!(info.description.contains("Orta boy"));
        assert!(info.description.contains("Detay"));
    }

    #[test]
    fn test_parse_response_new_format() {
        let service = OpenRouterService::new(
            "test_key".to_string(),
            "test_model".to_string(),
        );

        // Yeni geliştirilmiş format
        let response = "Yemek: Izgara tavuk göğsü, pilav, salata\n\
                        Kalori: 520\n\
                        Porsiyon: Orta büyüklük, yaklaşık 350g\n\
                        Besin Değeri: Yüksek protein, orta karbonhidrat, düşük yağ\n\
                        Sağlık Notu: Dengeli ve sağlıklı bir öğün";
        let info = service.parse_response(response).unwrap();

        assert_eq!(info.calories, 520.0);
        // Tüm alanlar korunmuş olmalı
        assert!(info.description.contains("Izgara tavuk"));
        assert!(info.description.contains("Porsiyon"));
        assert!(info.description.contains("Besin Değeri"));
        assert!(info.description.contains("Sağlık Notu"));
        assert!(info.description.contains("Dengeli"));
    }
}
