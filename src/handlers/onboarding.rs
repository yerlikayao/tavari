use crate::models::{ConversationDirection, MessageType, User};
use crate::services::{Database, WhatsAppService};
use anyhow::Result;
use std::sync::Arc;

pub struct OnboardingHandler {
    db: Arc<Database>,
    whatsapp: Arc<dyn WhatsAppService>,
}

impl OnboardingHandler {
    pub fn new(db: Arc<Database>, whatsapp: Arc<dyn WhatsAppService>) -> Self {
        Self { db, whatsapp }
    }

    pub async fn handle_step(&self, user: &User, message: &str) -> Result<()> {
        match user.onboarding_step.as_deref() {
            None => {
                // İlk mesaj - onboarding başlat
                self.start_onboarding(user).await?;
            }
            Some("ready_to_start") => {
                // Kullanıcı onboarding'i başlatmak istiyor
                self.start_onboarding(user).await?;
            }
            Some("breakfast_time") => {
                // Kahvaltı saatini kaydet
                self.save_breakfast_time(user, message).await?;
            }
            Some("lunch_time") => {
                // Öğle saatini kaydet
                self.save_lunch_time(user, message).await?;
            }
            Some("dinner_time") => {
                // Akşam saatini kaydet (içinde onboarding tamamlama da var)
                self.save_dinner_time(user, message).await?;
            }
            _ => {
                log::warn!("Unknown onboarding step: {:?}", user.onboarding_step);
            }
        }
        Ok(())
    }

    async fn start_onboarding(&self, user: &User) -> Result<()> {
        let welcome_msg = "🍽️ *Hoş geldin!*\n\n\
Beslenme takibini kişiselleştirmek için öğün saatlerini öğrenmeliyim.\n\n\
*Genelde kahvaltını ne zaman yaparsın?*\n\
Normal konuşarak yaz:\n\
• \"sabah 9'da\"\n\
• \"09:00\"\n\
• \"saat 9 gibi\"";

        self.whatsapp.send_message(&user.phone_number, welcome_msg).await?;

        // Log outgoing message
        let _ = self.db.log_conversation(
            &user.phone_number,
            ConversationDirection::Outgoing,
            MessageType::Response,
            welcome_msg,
            Some(serde_json::json!({"onboarding_step": "welcome"})),
        ).await;

        // İlk adım: kahvaltı saati
        self.db.update_onboarding_step(&user.phone_number, Some("breakfast_time".to_string())).await?;

        log::info!("🆕 Onboarding started for user: {}", user.phone_number);
        Ok(())
    }

    async fn save_breakfast_time(&self, user: &User, time: &str) -> Result<()> {
        let parsed_time = self.parse_natural_time(time);

        if let Some(formatted_time) = parsed_time {
            self.db.update_meal_time(&user.phone_number, "breakfast", &formatted_time).await?;

            let msg = format!("✅ Kahvaltı: {}\n\n*Öğle yemeğini ne zaman yersin?*\n\
Normal konuşarak yaz:\n\
• \"öğlen 1'de\"\n\
• \"13:00\"\n\
• \"saat 13 gibi\"", formatted_time);

            self.whatsapp.send_message(&user.phone_number, &msg).await?;

            // Log outgoing message
            let _ = self.db.log_conversation(
                &user.phone_number,
                ConversationDirection::Outgoing,
                MessageType::Response,
                &msg,
                Some(serde_json::json!({"onboarding_step": "breakfast_time_saved", "time": time})),
            ).await;

            self.db.update_onboarding_step(&user.phone_number, Some("lunch_time".to_string())).await?;
        } else {
            let msg = "❌ Saati anlayamadım\n\nÖrnekler:\n• \"sabah 9'da\"\n• \"09:00\"\n• \"saat 9 gibi\"";

            self.whatsapp.send_message(&user.phone_number, msg).await?;

            // Log error message
            let _ = self.db.log_conversation(
                &user.phone_number,
                ConversationDirection::Outgoing,
                MessageType::Error,
                msg,
                Some(serde_json::json!({"onboarding_step": "breakfast_time_invalid", "input": time})),
            ).await;
        }
        Ok(())
    }

    async fn save_lunch_time(&self, user: &User, time: &str) -> Result<()> {
        let parsed_time = self.parse_natural_time(time);

        if let Some(formatted_time) = parsed_time {
            self.db.update_meal_time(&user.phone_number, "lunch", &formatted_time).await?;

            let msg = format!("✅ Öğle: {}\n\n*Akşam yemeğini ne zaman yersin?*\n\
Normal konuşarak yaz:\n\
• \"akşam 7'de\"\n\
• \"19:00\"\n\
• \"saat 19 gibi\"", formatted_time);

            self.whatsapp.send_message(&user.phone_number, &msg).await?;

            // Log outgoing message
            let _ = self.db.log_conversation(
                &user.phone_number,
                ConversationDirection::Outgoing,
                MessageType::Response,
                &msg,
                Some(serde_json::json!({"onboarding_step": "lunch_time_saved", "time": time})),
            ).await;

            self.db.update_onboarding_step(&user.phone_number, Some("dinner_time".to_string())).await?;
        } else {
            let msg = "❌ Saati anlayamadım\n\nÖrnekler:\n• \"öğlen 1'de\"\n• \"13:00\"\n• \"saat 13 gibi\"";

            self.whatsapp.send_message(&user.phone_number, msg).await?;

            // Log error message
            let _ = self.db.log_conversation(
                &user.phone_number,
                ConversationDirection::Outgoing,
                MessageType::Error,
                msg,
                Some(serde_json::json!({"onboarding_step": "lunch_time_invalid", "input": time})),
            ).await;
        }
        Ok(())
    }

    async fn save_dinner_time(&self, user: &User, time: &str) -> Result<()> {
        let parsed_time = self.parse_natural_time(time);

        if let Some(formatted_time) = parsed_time {
            self.db.update_meal_time(&user.phone_number, "dinner", &formatted_time).await?;
            self.db.update_onboarding_step(&user.phone_number, None).await?;
            self.db.complete_onboarding(&user.phone_number).await?;
        } else {
            let msg = "❌ Saati anlayamadım\n\nÖrnekler:\n• \"akşam 7'de\"\n• \"19:00\"\n• \"saat 19 gibi\"";

            self.whatsapp.send_message(&user.phone_number, msg).await?;

            // Log error message
            let _ = self.db.log_conversation(
                &user.phone_number,
                ConversationDirection::Outgoing,
                MessageType::Error,
                msg,
                Some(serde_json::json!({"onboarding_step": "dinner_time_invalid", "input": time})),
            ).await;

            return Ok(());
        }

        // Fetch updated user with all meal times from database
        let updated_user = self.db.get_user(&user.phone_number).await?
            .ok_or_else(|| anyhow::anyhow!("User not found after onboarding completion"))?;

        let completion_msg = format!("🎉 *Hazırsın!*\n\n\
✅ Kahvaltı: {}\n\
✅ Öğle: {}\n\
✅ Akşam: {}\n\n\
*Nasıl kullanılır?*\n\
📸 Yemek fotoğrafı gönder\n\
💧 250 ml su içtim\n\
📊 rapor\n\n\
İyi beslenmeler! 🥗",
            updated_user.breakfast_time.as_deref().unwrap_or(""),
            updated_user.lunch_time.as_deref().unwrap_or(""),
            updated_user.dinner_time.as_deref().unwrap_or(""));

        self.whatsapp.send_message(&user.phone_number, &completion_msg).await?;

        // Log completion message
        let _ = self.db.log_conversation(
            &user.phone_number,
            ConversationDirection::Outgoing,
            MessageType::Response,
            &completion_msg,
            Some(serde_json::json!({
                "onboarding_step": "completed",
                "breakfast_time": updated_user.breakfast_time,
                "lunch_time": updated_user.lunch_time,
                "dinner_time": updated_user.dinner_time
            })),
        ).await;

        log::info!("✅ Onboarding completed for user: {}", user.phone_number);
        Ok(())
    }

    /// Parse natural language time input to HH:MM format
    /// Accepts formats like: "9", "09:00", "sabah 9", "saat 9 gibi", "9'da"
    fn parse_natural_time(&self, input: &str) -> Option<String> {
        let input = input.trim().to_lowercase();

        // First try exact HH:MM format
        if self.validate_time_format(&input) {
            return Some(input);
        }

        // Extract numbers from the input
        let numbers: Vec<u32> = input
            .split(|c: char| !c.is_numeric())
            .filter_map(|s| s.parse::<u32>().ok())
            .collect();

        if numbers.is_empty() {
            return None;
        }

        // Take the first number as the hour
        let hour = numbers[0];

        // If there's a second number, use it as minutes, otherwise default to 00
        let minute = if numbers.len() > 1 {
            numbers[1]
        } else {
            0
        };

        // Validate the parsed time
        if hour < 24 && minute < 60 {
            Some(format!("{:02}:{:02}", hour, minute))
        } else {
            None
        }
    }

    fn validate_time_format(&self, time: &str) -> bool {
        // HH:MM formatını kontrol et
        let parts: Vec<&str> = time.split(':').collect();
        if parts.len() != 2 {
            return false;
        }

        let hour = parts[0].parse::<u32>();
        let minute = parts[1].parse::<u32>();

        match (hour, minute) {
            (Ok(h), Ok(m)) => h < 24 && m < 60,
            _ => false,
        }
    }
}
