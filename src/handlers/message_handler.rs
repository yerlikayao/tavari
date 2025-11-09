use anyhow::Result;
use chrono::{Utc, Timelike};
use std::sync::Arc;

use crate::models::{ConversationDirection, Meal, MealType, MessageType, User, WaterLog};
use crate::services::{Database, OpenRouterService, WhatsAppService};
use crate::handlers::OnboardingHandler;

pub struct MessageHandler {
    db: Arc<Database>,
    openai: Arc<OpenRouterService>,  // OpenRouter kullanıyoruz (OpenAI uyumlu)
    whatsapp: Arc<dyn WhatsAppService>,
}

impl MessageHandler {
    pub fn new(
        db: Arc<Database>,
        openai: Arc<OpenRouterService>,
        whatsapp: Arc<dyn WhatsAppService>,
    ) -> Self {
        Self {
            db,
            openai,
            whatsapp,
        }
    }

    /// Helper function to send message and log it
    async fn send_and_log(
        &self,
        to: &str,
        message: &str,
        message_type: MessageType,
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        // Send message
        self.whatsapp.send_message(to, message).await?;

        // Log to database
        let _ = self.db.log_conversation(
            to,
            ConversationDirection::Outgoing,
            message_type,
            message,
            metadata,
        ).await;

        Ok(())
    }

    pub async fn handle_message(
        &self,
        from: &str,
        message: &str,
        has_media: bool,
        media_path: Option<String>,
    ) -> Result<()> {
        // LOG: Gelen mesajı kaydet
        log::info!("📨 INCOMING MESSAGE - From: {} | Content: '{}' | Has Media: {} | Media Path: {:?}",
                   from, message, has_media, media_path);

        // Kullanıcıyı kontrol et veya oluştur
        self.ensure_user_exists(from).await?;

        // Log incoming message to database
        let message_type = if has_media { MessageType::Image } else { MessageType::Text };
        let metadata = if has_media {
            Some(serde_json::json!({
                "has_media": true,
                "media_path": media_path.clone()
            }))
        } else {
            None
        };
        let _ = self.db.log_conversation(
            from,
            ConversationDirection::Incoming,
            message_type,
            message,
            metadata,
        ).await;

        // Kullanıcı bilgilerini al
        let user = self.db.get_user(from).await?.ok_or_else(|| anyhow::anyhow!("User not found"))?;

        // Kullanıcı deaktif ise, mesajı işleme ama yanıt verme
        if !user.is_active {
            log::warn!("⚠️ User {} is inactive, ignoring message", from);
            return Ok(());
        }

        // Onboarding tamamlanmamışsa, onboarding handler'a yönlendir
        if !user.onboarding_completed {
            log::info!("👤 User {} in onboarding phase (step: {:?})", from, user.onboarding_step);

            // İlk mesajda otomatik olarak onboarding'i başlat
            // Kullanıcıdan "tekrar mesaj gönder" dememek için direkt başlatıyoruz
            let onboarding_handler = OnboardingHandler::new(self.db.clone(), self.whatsapp.clone());
            onboarding_handler.handle_step(&user, message).await?;
            return Ok(());
        }

        let message_lower = message.trim().to_lowercase();

        // Resim varsa öncelik ver (komutlardan önce)
        if has_media {
            if let Some(image_path) = media_path {
                self.handle_food_image(from, &image_path).await?;
                return Ok(());
            }
        }

        // Quick water button responses (1, 2, 3)
        let trimmed = message.trim();
        if trimmed == "1" {
            self.handle_water_log(from, "200 ml içtim").await?;
            return Ok(());
        } else if trimmed == "2" {
            self.handle_water_log(from, "250 ml içtim").await?;
            return Ok(());
        } else if trimmed == "3" {
            self.handle_water_log(from, "500 ml içtim").await?;
            return Ok(());
        }

        // "su" yazıldığında butonları göster
        if message_lower.trim() == "su" {
            self.handle_water_buttons(from).await?;
            return Ok(());
        }

        // Su tüketimi kaydı
        // "250 ml içtim", "su içtim", "500ml", "1 bardak su" gibi tüm varyasyonlar
        let has_water_keyword = message_lower.contains("su") || message_lower.contains("ml") || message_lower.contains("bardak");
        let has_consumed = message_lower.contains("içtim") || message_lower.contains("içim");

        if (has_water_keyword && has_consumed) || (message_lower.contains("ml") && message_lower.len() < 20) {
            self.handle_water_log(from, message).await?;
            return Ok(());
        }

        // Akıllı komut tespiti - slash olsun olmasın çalışır
        if self.try_handle_smart_command(from, &message_lower).await? {
            return Ok(());
        }

        // Varsayılan yardım mesajı
        self.send_help_message(from).await?;

        Ok(())
    }

    async fn ensure_user_exists(&self, phone: &str) -> Result<()> {
        if self.db.get_user(phone).await?.is_none() {
            let user = User {
                phone_number: phone.to_string(),
                created_at: Utc::now(),
                onboarding_completed: false,
                onboarding_step: None,  // Onboarding handler başlatacak
                breakfast_reminder: true,
                lunch_reminder: true,
                dinner_reminder: true,
                water_reminder: true,
                breakfast_time: None,
                lunch_time: None,
                dinner_time: None,
                opted_in: true,
                timezone: "Europe/Istanbul".to_string(),  // Varsayılan Türkiye
                water_reminder_interval: Some(120),  // Varsayılan: 2 saat (120 dakika)
                daily_water_goal: Some(2000),  // Varsayılan: 2 litre (2000 ml)
                daily_calorie_goal: Some(2000),  // Varsayılan: 2000 kcal
                silent_hours_start: Some("23:00".to_string()),  // Varsayılan: 23:00
                silent_hours_end: Some("07:00".to_string()),    // Varsayılan: 07:00
                is_active: true,  // Varsayılan: aktif
            };
            self.db.create_user(&user).await?;
            log::info!("✅ New user created: {}", phone);
        }
        Ok(())
    }

    /// Optimized: Detect meal type without fetching user (user already available)
    async fn detect_meal_type_with_user(&self, user: &User, current_time: chrono::NaiveTime, today: chrono::NaiveDate) -> Result<MealType> {
        log::debug!("🕐 Detecting meal type for user {} at {} (timezone: {})", user.phone_number, current_time, user.timezone);

        // Bugün kaydedilmiş öğünleri kontrol et
        let todays_meals = self.db.get_todays_meal_types(&user.phone_number, today).await?;

        let has_breakfast = todays_meals.iter().any(|m| matches!(m, MealType::Breakfast));
        let has_lunch = todays_meals.iter().any(|m| matches!(m, MealType::Lunch));
        let has_dinner = todays_meals.iter().any(|m| matches!(m, MealType::Dinner));

        log::debug!("📊 Today's meals - Breakfast: {}, Lunch: {}, Dinner: {}", has_breakfast, has_lunch, has_dinner);

        // Kullanıcının öğün saatlerini parse et
        let breakfast_time = user.breakfast_time.as_ref()
            .and_then(|t| chrono::NaiveTime::parse_from_str(t, "%H:%M").ok());
        let lunch_time = user.lunch_time.as_ref()
            .and_then(|t| chrono::NaiveTime::parse_from_str(t, "%H:%M").ok());
        let dinner_time = user.dinner_time.as_ref()
            .and_then(|t| chrono::NaiveTime::parse_from_str(t, "%H:%M").ok());

        // Eğer öğün saatleri ayarlanmamışsa varsayılan saatler kullan
        let breakfast = breakfast_time.unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap());
        let lunch = lunch_time.unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(13, 0, 0).unwrap());
        let dinner = dinner_time.unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(19, 0, 0).unwrap());

        // Tolerans: ±2 saat
        let tolerance = chrono::Duration::hours(2);

        // Sıralı öğün kontrolü: Kahvaltı -> Öğle -> Akşam
        // Kullanıcı önce kahvaltı yapmalı, sonra öğle, sonra akşam

        // Eğer kahvaltı kayıtlı değilse ve kahvaltı saatindeyse
        if !has_breakfast && Self::is_within_time_range(current_time, breakfast, tolerance) {
            log::info!("🍳 Detected meal type: Breakfast (current: {}, target: {})", current_time, breakfast);
            return Ok(MealType::Breakfast);
        }

        // Eğer kahvaltı kayıtlı ama öğle kayıtlı değilse ve öğle saatindeyse
        if has_breakfast && !has_lunch && Self::is_within_time_range(current_time, lunch, tolerance) {
            log::info!("🍱 Detected meal type: Lunch (current: {}, target: {})", current_time, lunch);
            return Ok(MealType::Lunch);
        }

        // Eğer kahvaltı ve öğle kayıtlı ama akşam kayıtlı değilse ve akşam saatindeyse
        if has_breakfast && has_lunch && !has_dinner && Self::is_within_time_range(current_time, dinner, tolerance) {
            log::info!("🍽️ Detected meal type: Dinner (current: {}, target: {})", current_time, dinner);
            return Ok(MealType::Dinner);
        }

        // Eğer sıralı öğün kuralına uymuyorsa ara öğün olarak kaydet
        log::info!("🍪 Detected meal type: Snack (sequential rule or time doesn't match main meals) at {}", current_time);
        Ok(MealType::Snack)
    }

    /// Bir zamanın hedef zaman ± tolerans aralığında olup olmadığını kontrol et
    fn is_within_time_range(current: chrono::NaiveTime, target: chrono::NaiveTime, tolerance: chrono::Duration) -> bool {
        // Zamanları dakika cinsine çevir (gece yarısından bu yana)
        let current_mins = current.num_seconds_from_midnight() as i64 / 60;
        let target_mins = target.num_seconds_from_midnight() as i64 / 60;
        let tolerance_mins = tolerance.num_minutes();

        // Fark hesapla (gün sınırını dikkate alarak)
        let diff = (current_mins - target_mins).abs();

        // Gün sınırı kontrolü (örn: 23:00 ile 01:00 arası)
        let diff_wrapped = std::cmp::min(diff, 1440 - diff); // 1440 = 24 * 60

        diff_wrapped <= tolerance_mins
    }

    async fn handle_text_meal(&self, from: &str, description: &str) -> Result<()> {
        // AI'dan yemek analizi al
        match self.openai.analyze_text_meal(description).await {
            Ok(calorie_info) => {
                // Kullanıcı bilgilerini tek seferde al (hem timezone hem de meal detection için)
                let user = self.db.get_user(from).await?.ok_or_else(|| anyhow::anyhow!("User not found"))?;
                let user_tz: chrono_tz::Tz = user.timezone.parse().unwrap_or(chrono_tz::Europe::Istanbul);
                let now = Utc::now().with_timezone(&user_tz);
                let today = now.date_naive();

                // Akıllı öğün tespiti (user'ı tekrar fetch etmeden)
                let meal_type = self.detect_meal_type_with_user(&user, now.time(), today).await?;

                let meal = Meal {
                    id: None,
                    user_phone: from.to_string(),
                    meal_type: meal_type.clone(),
                    calories: calorie_info.calories,
                    description: calorie_info.description.clone(),
                    image_path: None, // Text-based meal, no image
                    created_at: Utc::now(),
                };

                self.db.add_meal(&meal).await?;

                let today = now.date_naive();
                let stats = self.db.get_daily_stats(from, today).await?;

                let meal_type_name = match meal_type {
                    MealType::Breakfast => "Kahvaltı",
                    MealType::Lunch => "Öğle Yemeği",
                    MealType::Dinner => "Akşam Yemeği",
                    MealType::Snack => "Ara Öğün",
                };

                let summary = format!(
                    "✅ *{} Kaydedildi!*\n\n\
                     📝 {}\n\
                     🔥 {:.0} kcal\n\n\
                     📊 Bugün: {:.0} kcal ({} öğün)",
                    meal_type_name,
                    calorie_info.description,
                    calorie_info.calories,
                    stats.total_calories,
                    stats.meals_count
                );

                self.whatsapp.send_message(from, &summary).await?;
            }
            Err(e) => {
                log::error!("❌ Failed to analyze text meal: {}", e);
                self.whatsapp
                    .send_message(
                        from,
                        "❌ Analiz yapılamadı.\nLütfen daha detaylı açıkla veya fotoğraf gönder.",
                    )
                    .await?;
            }
        }

        Ok(())
    }

    async fn handle_food_image(&self, from: &str, image_path: &str) -> Result<()> {
        // Kullanıcı bilgilerini tek seferde al (hem timezone hem de meal detection için)
        let user = self.db.get_user(from).await?.ok_or_else(|| anyhow::anyhow!("User not found"))?;
        let user_tz: chrono_tz::Tz = user.timezone.parse().unwrap_or(chrono_tz::Europe::Istanbul);
        let now = Utc::now().with_timezone(&user_tz);
        let today = now.date_naive();

        // Günlük resim limiti kontrolü (max 20)
        let daily_image_count = self.db.get_daily_image_count(from, today).await?;

        if daily_image_count >= 20 {
            log::warn!("📸 User {} reached daily image limit: {}/20", from, daily_image_count);
            self.whatsapp
                .send_message(
                    from,
                    "⚠️ *Günlük resim limiti* (20/20)\n\n\
                     Yarın tekrar fotoğraf gönderebilirsin.\n\
                     Bugün için: ogun tavuk göğsü ve salata"
                )
                .await?;
            return Ok(());
        }

        match self.openai.analyze_food_image(image_path).await {
            Ok(calorie_info) => {
                // Akıllı öğün tespiti (user'ı tekrar fetch etmeden)
                let meal_type = self.detect_meal_type_with_user(&user, now.time(), today).await?;

                let meal = Meal {
                    id: None,
                    user_phone: from.to_string(),
                    meal_type: meal_type.clone(),
                    calories: calorie_info.calories,
                    description: calorie_info.description.clone(),
                    image_path: Some(image_path.to_string()),
                    created_at: Utc::now(),
                };

                self.db.add_meal(&meal).await?;

                let stats = self.db.get_daily_stats(from, today).await?;

                let meal_type_name = match meal_type {
                    MealType::Breakfast => "Kahvaltı",
                    MealType::Lunch => "Öğle Yemeği",
                    MealType::Dinner => "Akşam Yemeği",
                    MealType::Snack => "Ara Öğün",
                };

                // Günlük resim sayısını tekrar al (yeni eklenen dahil)
                let updated_image_count = self.db.get_daily_image_count(from, today).await?;

                let summary = format!(
                    "✅ *{} Kaydedildi!*\n\n\
                     📝 {}\n\
                     🔥 {:.0} kcal\n\n\
                     📊 Bugün: {:.0} kcal ({} öğün)\n\
                     📸 Resim: {}/20",
                    meal_type_name,
                    calorie_info.description,
                    calorie_info.calories,
                    stats.total_calories,
                    stats.meals_count,
                    updated_image_count
                );

                self.whatsapp.send_message(from, &summary).await?;
            }
            Err(e) => {
                log::error!("Image analysis error: {}", e);
                self.whatsapp
                    .send_message(from, "❌ Resim analiz edilemedi. Tekrar dene.")
                    .await?;
            }
        }

        Ok(())
    }

    async fn handle_water_log(&self, from: &str, message: &str) -> Result<()> {
        // Mesajdan ml miktarını çıkar
        let amount = self.parse_water_amount(message);

        let water_log = WaterLog {
            id: None,
            user_phone: from.to_string(),
            amount_ml: amount,
            created_at: Utc::now(),
        };

        self.db.add_water_log(&water_log).await?;

        // Kullanıcı bilgilerini tek seferde al (hem timezone hem de water_goal için)
        let user = self.db.get_user(from).await?.ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let user_tz: chrono_tz::Tz = user.timezone.parse().unwrap_or(chrono_tz::Europe::Istanbul);
        let today = Utc::now().with_timezone(&user_tz).date_naive();

        let stats = self.db.get_daily_stats(from, today).await?;
        let water_goal = user.daily_water_goal.unwrap_or(2000);

        let response = format!(
            "💧 *{} ml kaydedildi!*\n\n\
             Bugün: {} ml / {} ml\n\
             Kalan: {} ml\n\n\
             💡 Hızlıca kaydet: 250 ml su içtim",
            amount,
            stats.total_water_ml,
            water_goal,
            water_goal - stats.total_water_ml as i32
        );

        self.whatsapp.send_message(from, &response).await?;

        Ok(())
    }

    fn parse_water_amount(&self, message: &str) -> i32 {
        // Basit parsing - "250 ml", "1 bardak", "200ml", "1000 ml" vb.
        if message.contains("bardak") {
            return 250; // 1 bardak = ~250ml
        }

        // "ml" veya "ML" kelimesini kaldır
        let cleaned = message.replace("ml", " ").replace("ML", " ");

        // Sayıyı bul
        let words: Vec<&str> = cleaned.split_whitespace().collect();
        for word in words {
            if let Ok(amount) = word.parse::<i32>() {
                if amount > 0 && amount <= 5000 {  // Limit 5000ml'ye çıkarıldı
                    return amount;
                }
            }
        }

        200 // varsayılan (kullanıcı sadece "su" yazarsa)
    }

    /// Akıllı komut tespiti - slash olsun olmasın komutları tanır
    /// Örnek: "rapor", "/rapor", "yardım", "yardim" hepsi çalışır
    async fn try_handle_smart_command(&self, from: &str, message: &str) -> Result<bool> {
        // Slash varsa kaldır
        let clean_msg = message.trim_start_matches('/').trim_start_matches('!');
        let parts: Vec<&str> = clean_msg.split_whitespace().collect();
        let main_word = parts.first().unwrap_or(&"");

        // Komut eşleştirmeleri - Türkçe karakterleri normalize et
        let matched = match *main_word {
            // Rapor komutları
            "rapor" | "report" | "özet" | "ozet" | "summary" => {
                let user = self.db.get_user(from).await?.ok_or_else(|| anyhow::anyhow!("User not found"))?;
                let user_tz: chrono_tz::Tz = user.timezone.parse().unwrap_or(chrono_tz::Europe::Istanbul);
                let today = Utc::now().with_timezone(&user_tz).date_naive();
                let stats = self.db.get_daily_stats(from, today).await?;
                let report = crate::services::whatsapp::format_daily_report(
                    stats.total_calories,
                    stats.total_water_ml,
                    stats.meals_count,
                    stats.water_logs_count,
                    user.daily_calorie_goal.unwrap_or(2000),
                    user.daily_water_goal.unwrap_or(2000),
                );
                self.whatsapp.send_message(from, &report).await?;
                true
            }
            // Yardım komutları
            "yardim" | "yardım" | "help" | "?" | "komutlar" | "commands" => {
                self.send_help_message(from).await?;
                true
            }
            // Geçmiş komutları
            "gecmis" | "geçmiş" | "history" | "tarihçe" | "tarihce" => {
                let meals = self.db.get_recent_meals(from, 5).await?;

                if meals.is_empty() {
                    self.whatsapp.send_message(from, "📜 Henüz kayıtlı öğün yok.").await?;
                } else {
                    let mut response = "📜 *Son 5 Öğün*\n\n".to_string();
                    for (i, meal) in meals.iter().enumerate() {
                        response.push_str(&format!(
                            "{}. {} • {:.0} kcal\n{}\n{}\n\n",
                            i + 1,
                            meal.meal_type.to_string(),
                            meal.calories,
                            meal.description,
                            meal.created_at.format("%d.%m %H:%M")
                        ));
                    }
                    self.whatsapp.send_message(from, &response).await?;
                }
                true
            }
            // Tavsiye komutları
            "tavsiye" | "öneri" | "oneri" | "advice" | "tip" | "tips" => {
                // Kullanıcı bilgilerini tek seferde al (hem timezone hem de water_goal için)
                let user = self.db.get_user(from).await?.ok_or_else(|| anyhow::anyhow!("User not found"))?;
                let user_tz: chrono_tz::Tz = user.timezone.parse().unwrap_or(chrono_tz::Europe::Istanbul);
                let today = Utc::now().with_timezone(&user_tz).date_naive();
                let stats = self.db.get_daily_stats(from, today).await?;
                let water_goal = user.daily_water_goal.unwrap_or(2000);

                match self
                    .openai
                    .get_nutrition_advice(
                        stats.total_calories,
                        stats.total_water_ml,
                        water_goal,
                        stats.meals_count
                    )
                    .await
                {
                    Ok(advice) => {
                        self.whatsapp.send_message(from, &advice).await?;
                    }
                    Err(e) => {
                        log::error!("❌ Failed to get nutrition advice: {:?}", e);
                        log::error!("❌ Error details: {}", e);

                        // Provide more user-friendly error messages
                        let error_msg = if e.to_string().contains("moderation") {
                            "⚠️ AI hizmeti geçici olarak kullanılamıyor (içerik moderasyonu hatası). Lütfen daha sonra tekrar deneyin."
                        } else if e.to_string().contains("Rate limit") {
                            "⚠️ Çok fazla istek gönderildi. Lütfen birkaç dakika sonra tekrar deneyin."
                        } else {
                            "⚠️ Şu anda tavsiye alınamıyor. Lütfen daha sonra tekrar deneyin."
                        };

                        self.whatsapp
                            .send_message(from, error_msg)
                            .await?;
                    }
                }
                true
            }
            // Ayarlar komutları
            "ayarlar" | "settings" | "ayar" | "setting" => {
                self.handle_settings_command(from).await?;
                true
            }
            // Buton komutları - Su için hızlı butonlar
            "buton" | "butonlar" | "buttons" | "button" => {
                self.handle_water_buttons(from).await?;
                true
            }
            // Saat komutları
            "saat" | "time" => {
                self.handle_time_command(from, &parts).await?;
                true
            }
            // Timezone komutları
            "timezone" | "tz" | "zamandilimi" => {
                self.handle_timezone_command(from, &parts).await?;
                true
            }
            // Su hatırlatma aralığı komutları
            "suaraligi" | "suaraliği" | "waterinterval" => {
                self.handle_water_interval_command(from, &parts).await?;
                true
            }
            // Su hedefi komutları
            "suhedefi" | "watergoal" | "suhedfi" => {
                self.handle_water_goal_command(from, &parts).await?;
                true
            }
            // Kalori hedefi komutları
            "kalorihedefi" | "caloriegoal" | "kalorihedfi" => {
                self.handle_calorie_goal_command(from, &parts).await?;
                true
            }
            // Sessiz saatler komutları
            "sessiz" | "silent" | "silentsaatler" => {
                self.handle_silent_hours_command(from, &parts).await?;
                true
            }
            // Favori yemekler komutları
            "favori" | "favoriler" | "favorite" | "favorites" | "fav" => {
                self.handle_favorite_meals_command(from, &parts).await?;
                true
            }
            // Öğün kayıt komutları (text-based meal logging)
            "ogun" | "yemek" | "meal" | "food" => {
                if parts.len() < 2 {
                    self.whatsapp.send_message(
                        from,
                        "❌ Kullanım: ogun [yemek açıklaması]\n\nÖrnek: ogun tavuk göğsü ve salata"
                    ).await?;
                } else {
                    // Tüm kelime parçalarını birleştir (ilk kelime hariç)
                    let description = parts[1..].join(" ");
                    self.handle_text_meal(from, &description).await?;
                }
                true
            }
            // Check for quick favorite patterns (fav1, fav2, etc.)
            word if word.starts_with("fav") && word.len() > 3 => {
                let name = word.to_string();
                self.handle_quick_favorite(from, &name).await?;
                true
            }
            _ => false,
        };

        Ok(matched)
    }

    async fn handle_settings_command(&self, from: &str) -> Result<()> {
        let user = self.db.get_user(from).await?.ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let breakfast_time = user.breakfast_time.unwrap_or_else(|| "Ayarlanmamış".to_string());
        let lunch_time = user.lunch_time.unwrap_or_else(|| "Ayarlanmamış".to_string());
        let dinner_time = user.dinner_time.unwrap_or_else(|| "Ayarlanmamış".to_string());

        let breakfast_status = if user.breakfast_reminder { "✅" } else { "❌" };
        let lunch_status = if user.lunch_reminder { "✅" } else { "❌" };
        let dinner_status = if user.dinner_reminder { "✅" } else { "❌" };
        let water_status = if user.water_reminder { "✅" } else { "❌" };

        let water_interval = user.water_reminder_interval.unwrap_or(120);
        let water_goal = user.daily_water_goal.unwrap_or(2000);
        let calorie_goal = user.daily_calorie_goal.unwrap_or(2000);
        let silent_start = user.silent_hours_start.as_deref().unwrap_or("23:00");
        let silent_end = user.silent_hours_end.as_deref().unwrap_or("07:00");

        let message = format!(
            "⚙️ *Ayarlarınız*\n\n\
             🕐 *Öğün Saatleri*\n\
             Kahvaltı: {} {}\n\
             Öğle: {} {}\n\
             Akşam: {} {}\n\n\
             🎯 *Günlük Hedefler*\n\
             {} kcal kalori\n\
             {} ml su ({:.1}L)\n\n\
             💧 *Su Hatırlatma*\n\
             {} Her {} dakika\n\n\
             🌙 *Sessiz Saatler*\n\
             {} - {}\n\n\
             🌍 *Zaman Dilimi*\n\
             {}\n\n\
             *Değiştirmek için:*\n\
             kalorihedefi 2500\n\
             suhedefi 3000\n\
             sessiz 23:00 07:00\n\
             saat kahvalti 09:00\n\
             suaraligi 120\n\
             timezone Europe/Istanbul",
            breakfast_time, breakfast_status,
            lunch_time, lunch_status,
            dinner_time, dinner_status,
            calorie_goal,
            water_goal,
            water_goal as f64 / 1000.0,
            water_status,
            water_interval,
            silent_start,
            silent_end,
            user.timezone
        );

        self.whatsapp.send_message(from, &message).await?;
        Ok(())
    }

    async fn handle_time_command(&self, from: &str, cmd_parts: &[&str]) -> Result<()> {
        if cmd_parts.len() < 3 {
            self.whatsapp.send_message(
                from,
                "❌ Kullanım: saat [kahvalti|ogle|aksam] HH:MM\nÖrnek: saat kahvalti 09:00"
            ).await?;
            return Ok(());
        }

        let meal_type = cmd_parts[1].to_lowercase();
        let time = cmd_parts[2];

        // Validate time format (HH:MM) with proper hour/minute range checks
        if !self.validate_time_format(time) {
            self.whatsapp.send_message(
                from,
                "❌ Geçersiz saat formatı\nHH:MM olmalı (örn: 09:00, 13:30)"
            ).await?;
            return Ok(());
        }

        let meal_type_db = match meal_type.as_str() {
            "kahvalti" | "kahvaltı" | "breakfast" => "breakfast",
            "ogle" | "öğle" | "lunch" => "lunch",
            "aksam" | "akşam" | "dinner" => "dinner",
            _ => {
                self.whatsapp.send_message(
                    from,
                    "❌ Geçersiz öğün tipi. Kullan: kahvalti, ogle, aksam"
                ).await?;
                return Ok(());
            }
        };

        self.db.update_meal_time(from, meal_type_db, time).await?;

        let meal_display = match meal_type_db {
            "breakfast" => "Kahvaltı",
            "lunch" => "Öğle yemeği",
            "dinner" => "Akşam yemeği",
            _ => "Öğün"
        };

        self.whatsapp.send_message(
            from,
            &format!("✅ {} saati {} olarak güncellendi!", meal_display, time)
        ).await?;

        Ok(())
    }

    async fn handle_timezone_command(&self, from: &str, cmd_parts: &[&str]) -> Result<()> {
        if cmd_parts.len() < 2 {
            self.whatsapp.send_message(
                from,
                "❌ Kullanım: timezone [zaman dilimi]\n\n\
                 Örnekler:\n\
                 timezone Europe/Istanbul\n\
                 timezone America/New_York\n\
                 timezone Asia/Tokyo"
            ).await?;
            return Ok(());
        }

        let timezone = cmd_parts[1];

        // Validate timezone by trying to parse it
        match timezone.parse::<chrono_tz::Tz>() {
            Ok(_) => {
                // Valid timezone, update in database
                self.db.update_timezone(from, timezone).await?;

                self.whatsapp.send_message(
                    from,
                    &format!("✅ Zaman diliminiz {} olarak güncellendi!", timezone)
                ).await?;
            }
            Err(_) => {
                self.whatsapp.send_message(
                    from,
                    &format!("❌ Geçersiz zaman dilimi: {}\n\nÖrnek: Europe/Istanbul", timezone)
                ).await?;
            }
        }

        Ok(())
    }

    async fn handle_water_interval_command(&self, from: &str, cmd_parts: &[&str]) -> Result<()> {
        if cmd_parts.len() < 2 {
            self.whatsapp.send_message(
                from,
                "❌ Kullanım: suaraligi [dakika]\nÖrnek: suaraligi 120"
            ).await?;
            return Ok(());
        }

        let interval_str = cmd_parts[1];
        match interval_str.parse::<i32>() {
            Ok(interval) if interval > 0 && interval <= 480 => {
                self.db.update_water_reminder_interval(from, interval).await?;

                self.whatsapp.send_message(
                    from,
                    &format!("✅ Su hatırlatma aralığı {} dakika ({} saat) olarak güncellendi!",
                        interval,
                        interval as f64 / 60.0)
                ).await?;
            }
            Ok(interval) => {
                self.whatsapp.send_message(
                    from,
                    &format!("❌ Geçersiz aralık: {} dakika\nLütfen 1-480 dakika arası bir değer girin.", interval)
                ).await?;
            }
            Err(_) => {
                self.whatsapp.send_message(
                    from,
                    &format!("❌ Geçersiz sayı: {}\nLütfen sayı girin (örn: 120)", interval_str)
                ).await?;
            }
        }

        Ok(())
    }

    async fn handle_water_goal_command(&self, from: &str, cmd_parts: &[&str]) -> Result<()> {
        if cmd_parts.len() < 2 {
            self.whatsapp.send_message(
                from,
                "❌ Kullanım: suhedefi [ml]\nÖrnek: suhedefi 2500"
            ).await?;
            return Ok(());
        }

        let goal_str = cmd_parts[1];
        match goal_str.parse::<i32>() {
            Ok(goal) if (500..=10000).contains(&goal) => {
                self.db.update_water_goal(from, goal).await?;

                self.whatsapp.send_message(
                    from,
                    &format!("✅ Günlük su hedefiniz {} ml ({} litre) olarak güncellendi!",
                        goal,
                        goal as f64 / 1000.0)
                ).await?;
            }
            Ok(goal) => {
                self.whatsapp.send_message(
                    from,
                    &format!("❌ Geçersiz hedef: {} ml\nLütfen 500-10000 ml arası bir değer girin.", goal)
                ).await?;
            }
            Err(_) => {
                self.whatsapp.send_message(
                    from,
                    &format!("❌ Geçersiz sayı: {}\nLütfen sayı girin (örn: 2000)", goal_str)
                ).await?;
            }
        }

        Ok(())
    }

    async fn send_help_message(&self, to: &str) -> Result<()> {
        let help = "📱 *Beslenme Takip Botu*\n\n\
                   *🍽️ Nasıl Kullanılır?*\n\
                   • Yemek fotoğrafı gönder\n\
                   • ogun [açıklama] - Text ile kaydet\n\
                   • su - Hızlı su kaydı menüsü 💧\n\
                   • 250 ml içtim - Direkt su takibi\n\n\
                   *📊 Ana Komutlar*\n\
                   rapor - Günlük özet (progress bar)\n\
                   geçmiş - Son 5 öğün\n\
                   tavsiye - AI beslenme önerisi\n\
                   ayarlar - Tüm ayarlar\n\n\
                   *⭐ Favori Yemekler*\n\
                   favori - Liste görüntüle\n\
                   favori ekle fav1 Tavuklu pilav\n\
                   favori sil fav1\n\
                   fav1 - Hızlı kayıt\n\n\
                   *🎯 Hedefler*\n\
                   kalorihedefi 2500\n\
                   suhedefi 3000\n\
                   sessiz 23:00 07:00\n\n\
                   *⚙️ Ayarlar*\n\
                   saat kahvalti 09:00\n\
                   suaraligi 120\n\
                   timezone Europe/Istanbul\n\n\
                   *💡 İpucu:* Komutlarda '/' kullanmana gerek yok!";

        self.whatsapp.send_message(to, help).await?;
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

    // ============================================================
    // New Command Handlers
    // ============================================================

    async fn handle_calorie_goal_command(&self, from: &str, parts: &[&str]) -> Result<()> {
        if parts.len() < 2 {
            let user = self.db.get_user(from).await?.ok_or_else(|| anyhow::anyhow!("User not found"))?;
            let current_goal = user.daily_calorie_goal.unwrap_or(2000);
            self.whatsapp.send_message(
                from,
                &format!(
                    "🎯 *Günlük Kalori Hedefi*\n\n\
                     Mevcut hedefiniz: {} kcal\n\n\
                     Değiştirmek için:\n\
                     `kalorihedefi [miktar]`\n\n\
                     Örnek: kalorihedefi 2500",
                    current_goal
                )
            ).await?;
            return Ok(());
        }

        let goal: i32 = parts[1].parse().map_err(|_| anyhow::anyhow!("Geçersiz sayı"))?;

        if !(500..=5000).contains(&goal) {
            self.whatsapp.send_message(
                from,
                "❌ Kalori hedefi 500-5000 kcal arasında olmalıdır."
            ).await?;
            return Ok(());
        }

        self.db.update_calorie_goal(from, goal).await?;
        self.whatsapp.send_message(
            from,
            &format!("✅ Günlük kalori hedefiniz {} kcal olarak güncellendi!", goal)
        ).await?;

        Ok(())
    }

    async fn handle_silent_hours_command(&self, from: &str, parts: &[&str]) -> Result<()> {
        if parts.len() < 3 {
            let user = self.db.get_user(from).await?.ok_or_else(|| anyhow::anyhow!("User not found"))?;
            let start = user.silent_hours_start.as_deref().unwrap_or("23:00");
            let end = user.silent_hours_end.as_deref().unwrap_or("07:00");

            self.whatsapp.send_message(
                from,
                &format!(
                    "🌙 *Sessiz Saatler*\n\n\
                     Mevcut ayarınız: {} - {}\n\n\
                     Bu saatler arasında hatırlatma gönderilmez.\n\n\
                     Değiştirmek için:\n\
                     `sessiz [başlangıç] [bitiş]`\n\n\
                     Örnek: sessiz 23:00 07:00",
                    start, end
                )
            ).await?;
            return Ok(());
        }

        let start = parts[1];
        let end = parts[2];

        if !self.validate_time_format(start) || !self.validate_time_format(end) {
            self.whatsapp.send_message(
                from,
                "❌ Geçersiz saat formatı. HH:MM formatında girin.\nÖrnek: sessiz 23:00 07:00"
            ).await?;
            return Ok(());
        }

        self.db.update_silent_hours(from, start, end).await?;
        self.whatsapp.send_message(
            from,
            &format!("✅ Sessiz saatleriniz {} - {} olarak güncellendi!", start, end)
        ).await?;

        Ok(())
    }

    async fn handle_favorite_meals_command(&self, from: &str, parts: &[&str]) -> Result<()> {
        // Sub-command handling: favori [ekle|liste|sil]
        if parts.len() < 2 {
            // No sub-command: show list
            let favorites = self.db.get_favorite_meals(from).await?;

            if favorites.is_empty() {
                self.whatsapp.send_message(
                    from,
                    "⭐ *Favori Yemekler*\n\n\
                     Henüz favori yok.\n\n\
                     *Ekle:*\n\
                     favori ekle fav1 Tavuklu pilav\n\n\
                     *Kullan:*\n\
                     Sadece 'fav1' yaz!"
                ).await?;
                return Ok(());
            }

            let mut response = "⭐ *Favori Yemekleriniz*\n\n".to_string();
            for fav in favorites.iter() {
                response.push_str(&format!(
                    "• {} • {:.0} kcal\n   {}\n",
                    fav.name, fav.calories, fav.description
                ));
            }
            response.push_str("\n💡 Kaydet: Sadece favori adını yaz");

            self.whatsapp.send_message(from, &response).await?;
            return Ok(());
        }

        let subcommand = parts[1];
        match subcommand {
            "ekle" | "add" => {
                if parts.len() < 4 {
                    self.whatsapp.send_message(
                        from,
                        "❌ Kullanım: favori ekle [isim] [açıklama]\n\nÖrnek: favori ekle fav1 Tavuklu pilav ve salata"
                    ).await?;
                    return Ok(());
                }

                let name = parts[2].to_lowercase();
                let description = parts[3..].join(" ");

                // Validate name (only alphanumeric and Turkish characters)
                if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    self.whatsapp.send_message(
                        from,
                        "❌ Favori ismi sadece harf, rakam ve _ içerebilir."
                    ).await?;
                    return Ok(());
                }

                // Get calorie estimate from OpenAI
                let (calories, analyzed_description) = match self.openai.analyze_text_meal(&description).await {
                    Ok(info) => (info.calories, info.description),
                    Err(e) => {
                        log::warn!("Failed to analyze favorite meal calories: {:?}", e);
                        (0.0, description.clone()) // Default to 0 if analysis fails
                    }
                };

                let favorite = crate::models::FavoriteMeal {
                    id: None,
                    user_phone: from.to_string(),
                    name: name.clone(),
                    description: analyzed_description.clone(),
                    calories,
                    created_at: Utc::now(),
                };

                self.db.add_favorite_meal(&favorite).await?;
                self.whatsapp.send_message(
                    from,
                    &format!(
                        "✅ *Favori eklendi!*\n\n\
                         {} • {:.0} kcal\n\
                         {}\n\n\
                         💡 Kaydet: Sadece '{}' yaz",
                        name, calories, analyzed_description, name
                    )
                ).await?;
            }
            "sil" | "delete" | "remove" => {
                if parts.len() < 3 {
                    self.whatsapp.send_message(
                        from,
                        "❌ Kullanım: favori sil [isim]\n\nÖrnek: favori sil fav1"
                    ).await?;
                    return Ok(());
                }

                let name = parts[2].to_lowercase();
                self.db.delete_favorite_meal(from, &name).await?;
                self.whatsapp.send_message(
                    from,
                    &format!("✅ '{}' favorilerden silindi.", name)
                ).await?;
            }
            _ => {
                self.whatsapp.send_message(
                    from,
                    "❌ Geçersiz komut.\n\n\
                     Kullanılabilir komutlar:\n\
                     • `favori` - Liste göster\n\
                     • `favori ekle [isim] [açıklama]`\n\
                     • `favori sil [isim]`"
                ).await?;
            }
        }

        Ok(())
    }

    async fn handle_quick_favorite(&self, from: &str, name: &str) -> Result<()> {
        // Try to get the favorite meal
        let favorite = self.db.get_favorite_meal_by_name(from, name).await?;

        if let Some(fav) = favorite {
            // Detect meal type based on current time
            let user = self.db.get_user(from).await?.ok_or_else(|| anyhow::anyhow!("User not found"))?;
            let user_tz: chrono_tz::Tz = user.timezone.parse().unwrap_or(chrono_tz::Europe::Istanbul);
            let now_user = Utc::now().with_timezone(&user_tz);
            let current_time = now_user.time();
            let today = now_user.date_naive();
            let meal_type = self.detect_meal_type_with_user(&user, current_time, today).await?;

            // Log the meal
            let meal = crate::models::Meal {
                id: None,
                user_phone: from.to_string(),
                meal_type: meal_type.clone(),
                calories: fav.calories,
                description: fav.description.clone(),
                image_path: None,
                created_at: Utc::now(),
            };

            self.db.add_meal(&meal).await?;

            self.whatsapp.send_message(
                from,
                &format!(
                    "✅ *{} kaydedildi!*\n\n\
                     {}\n\
                     🔥 {:.0} kcal",
                    meal_type.to_string(),
                    fav.description,
                    fav.calories
                )
            ).await?;
        } else {
            self.whatsapp.send_message(
                from,
                &format!(
                    "❌ '{}' bulunamadı\n\nEklemek için:\nfavori ekle {} [açıklama]",
                    name, name
                )
            ).await?;
        }

        Ok(())
    }

    /// Handle water buttons command - send interactive buttons for quick water logging
    async fn handle_water_buttons(&self, from: &str) -> Result<()> {
        log::info!("💧 Sending water buttons to {}", from);

        let buttons = vec![
            ("water_200".to_string(), "💧 200 ml".to_string()),
            ("water_250".to_string(), "💧 250 ml".to_string()),
            ("water_500".to_string(), "💧 500 ml".to_string()),
        ];

        self.whatsapp
            .send_message_with_buttons(
                from,
                "💧 *Su Kaydı*\n\nNe kadar su içtin?",
                buttons,
            )
            .await?;

        Ok(())
    }
}
