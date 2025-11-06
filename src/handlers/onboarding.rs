use crate::models::User;
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
*Kahvaltı saatin?*\nÖrnek: 09:00";

        self.whatsapp.send_message(&user.phone_number, welcome_msg).await?;

        // İlk adım: kahvaltı saati
        self.db.update_onboarding_step(&user.phone_number, Some("breakfast_time".to_string())).await?;

        log::info!("🆕 Onboarding started for user: {}", user.phone_number);
        Ok(())
    }

    async fn save_breakfast_time(&self, user: &User, time: &str) -> Result<()> {
        if self.validate_time_format(time) {
            self.db.update_meal_time(&user.phone_number, "breakfast", time).await?;

            let msg = format!("✅ Kahvaltı: {}\n\n*Öğle yemeği saatin?*\nÖrnek: 13:00", time);

            self.whatsapp.send_message(&user.phone_number, &msg).await?;
            self.db.update_onboarding_step(&user.phone_number, Some("lunch_time".to_string())).await?;
        } else {
            let msg = "❌ Geçersiz format\n\nHH:MM olmalı\nÖrnek: 09:00";

            self.whatsapp.send_message(&user.phone_number, msg).await?;
        }
        Ok(())
    }

    async fn save_lunch_time(&self, user: &User, time: &str) -> Result<()> {
        if self.validate_time_format(time) {
            self.db.update_meal_time(&user.phone_number, "lunch", time).await?;

            let msg = format!("✅ Öğle: {}\n\n*Akşam yemeği saatin?*\nÖrnek: 19:00", time);

            self.whatsapp.send_message(&user.phone_number, &msg).await?;
            self.db.update_onboarding_step(&user.phone_number, Some("dinner_time".to_string())).await?;
        } else {
            let msg = "❌ Geçersiz format\n\nHH:MM olmalı\nÖrnek: 09:00";

            self.whatsapp.send_message(&user.phone_number, msg).await?;
        }
        Ok(())
    }

    async fn save_dinner_time(&self, user: &User, time: &str) -> Result<()> {
        if self.validate_time_format(time) {
            self.db.update_meal_time(&user.phone_number, "dinner", time).await?;
            self.db.update_onboarding_step(&user.phone_number, None).await?;
            self.db.complete_onboarding(&user.phone_number).await?;
        } else {
            let msg = "❌ Geçersiz format\n\nHH:MM olmalı\nÖrnek: 09:00";

            self.whatsapp.send_message(&user.phone_number, msg).await?;
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

        log::info!("✅ Onboarding completed for user: {}", user.phone_number);
        Ok(())
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
