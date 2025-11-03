use anyhow::Result;
use chrono::{Utc, Timelike};
use std::sync::Arc;

use crate::models::{Meal, MealType, User, WaterLog};
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

        // Kullanıcı bilgilerini al
        let user = self.db.get_user(from).await?.ok_or_else(|| anyhow::anyhow!("User not found"))?;

        // Onboarding tamamlanmamışsa, onboarding handler'a yönlendir
        if !user.onboarding_completed {
            log::info!("👤 User {} in onboarding phase (step: {:?})", from, user.onboarding_step);

            // İlk mesajda onboarding başlamasın, sadece bilgilendirme mesajı gönder
            if user.onboarding_step.is_none() {
                let info_msg = "👋 *Beslenme Takip Botuna Hoş Geldiniz!*\n\n\
                               Öncelikli olarak öğünlerinizin saatini girmelisiniz.\n\n\
                               *Herhangi bir mesaj yazarak onboarding'i başlatabilirsiniz.*\n\
                               (Örneğin: 'merhaba' veya 'başla')";

                self.whatsapp.send_message(from, info_msg).await?;
                self.db.update_onboarding_step(from, Some("ready_to_start".to_string())).await?;
                return Ok(());
            }

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

        // Su tüketimi kaydı
        if message_lower.contains("su") && (message_lower.contains("içtim") || message_lower.contains("ml")) {
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
            };
            self.db.create_user(&user).await?;
            log::info!("✅ New user created: {}", phone);
        }
        Ok(())
    }

    /// Kullanıcının saatine ve öğün saatlerine göre öğün tipini akıllıca belirle
    /// Kullanıcının zaman dilimine göre bugünün tarihini al
    async fn get_user_today(&self, from: &str) -> Result<chrono::NaiveDate> {
        let user = self.db.get_user(from).await?;
        let user_tz: chrono_tz::Tz = user
            .as_ref()
            .and_then(|u| u.timezone.parse().ok())
            .unwrap_or(chrono_tz::Europe::Istanbul);

        let now = Utc::now().with_timezone(&user_tz);
        Ok(now.date_naive())
    }

    async fn detect_meal_type(&self, from: &str) -> Result<MealType> {
        // Kullanıcı bilgilerini al
        let user = match self.db.get_user(from).await? {
            Some(u) => u,
            None => return Ok(MealType::Snack), // Kullanıcı yoksa ara öğün
        };

        // Kullanıcının zaman dilimine göre şu anki saati al
        let user_tz: chrono_tz::Tz = user.timezone.parse().unwrap_or(chrono_tz::Europe::Istanbul);
        let now = Utc::now().with_timezone(&user_tz);
        let current_time = now.time();

        log::debug!("🕐 Detecting meal type for user {} at {} (timezone: {})", from, current_time, user.timezone);

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

        // Kahvaltı zamanı mı? (Kahvaltı saati ± 2 saat)
        if Self::is_within_time_range(current_time, breakfast, tolerance) {
            log::info!("🍳 Detected meal type: Breakfast (current: {}, target: {})", current_time, breakfast);
            return Ok(MealType::Breakfast);
        }

        // Öğle yemeği zamanı mı?
        if Self::is_within_time_range(current_time, lunch, tolerance) {
            log::info!("🍱 Detected meal type: Lunch (current: {}, target: {})", current_time, lunch);
            return Ok(MealType::Lunch);
        }

        // Akşam yemeği zamanı mı?
        if Self::is_within_time_range(current_time, dinner, tolerance) {
            log::info!("🍽️ Detected meal type: Dinner (current: {}, target: {})", current_time, dinner);
            return Ok(MealType::Dinner);
        }

        // Hiçbir ana öğün zamanına denk gelmiyorsa ara öğün
        log::info!("🍪 Detected meal type: Snack (current: {}, not matching any main meal)", current_time);
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

    async fn handle_food_image(&self, from: &str, image_path: &str) -> Result<()> {
        match self.openai.analyze_food_image(image_path).await {
            Ok(calorie_info) => {
                // Akıllı öğün tespiti
                let meal_type = self.detect_meal_type(from).await?;

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

                let today = self.get_user_today(from).await?;
                let stats = self.db.get_daily_stats(from, today).await?;

                // Öğün tipine göre emoji seç
                let meal_emoji = match meal_type {
                    MealType::Breakfast => "🍳",
                    MealType::Lunch => "🍱",
                    MealType::Dinner => "🍽️",
                    MealType::Snack => "🍪",
                };

                let meal_type_name = match meal_type {
                    MealType::Breakfast => "Kahvaltı",
                    MealType::Lunch => "Öğle Yemeği",
                    MealType::Dinner => "Akşam Yemeği",
                    MealType::Snack => "Ara Öğün",
                };

                let summary = format!(
                    "✅ Kaydedildi!\n\n\
                     {} Öğün Tipi: {}\n\
                     🔥 Kalori: {:.0} kcal\n\
                     📝 {}\n\n\
                     📊 Günlük toplam: {:.0} kcal ({} öğün)",
                    meal_emoji,
                    meal_type_name,
                    calorie_info.calories,
                    calorie_info.description,
                    stats.total_calories,
                    stats.meals_count
                );

                self.whatsapp.send_message(from, &summary).await?;
            }
            Err(e) => {
                log::error!("Image analysis error: {}", e);
                self.whatsapp
                    .send_message(from, "❌ Resim analiz edilemedi. Lütfen tekrar dene.")
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

        let today = self.get_user_today(from).await?;
        let stats = self.db.get_daily_stats(from, today).await?;

        // Kullanıcının su hedefini al
        let user = self.db.get_user(from).await?;
        let water_goal = user.and_then(|u| u.daily_water_goal).unwrap_or(2000);

        let response = format!(
            "💧 {} ml su kaydedildi!\n\n\
             Bugünkü toplam: {} ml ({:.1} litre)\n\
             Hedef: {} ml ({:.1} litre)",
            amount,
            stats.total_water_ml,
            stats.total_water_ml as f64 / 1000.0,
            water_goal,
            water_goal as f64 / 1000.0
        );

        self.whatsapp.send_message(from, &response).await?;

        Ok(())
    }

    fn parse_water_amount(&self, message: &str) -> i32 {
        // Basit parsing - "250 ml", "1 bardak", "200ml" vb.
        if message.contains("bardak") {
            return 250; // 1 bardak = ~250ml
        }

        // "ml" veya "ML" kelimesini kaldır
        let cleaned = message.replace("ml", " ").replace("ML", " ");

        // Sayıyı bul
        let words: Vec<&str> = cleaned.split_whitespace().collect();
        for word in words {
            if let Ok(amount) = word.parse::<i32>() {
                if amount > 0 && amount <= 2000 {
                    return amount;
                }
            }
        }

        250 // varsayılan
    }

    /// Akıllı komut tespiti - slash olsun olmasın komutları tanır
    /// Örnek: "rapor", "/rapor", "yardım", "yardim" hepsi çalışır
    async fn try_handle_smart_command(&self, from: &str, message: &str) -> Result<bool> {
        // Slash varsa kaldır
        let clean_msg = message.trim_start_matches('/').trim_start_matches('!');
        let parts: Vec<&str> = clean_msg.split_whitespace().collect();
        let main_word = parts.get(0).unwrap_or(&"");

        // Komut eşleştirmeleri - Türkçe karakterleri normalize et
        let matched = match *main_word {
            // Rapor komutları
            "rapor" | "report" | "özet" | "ozet" | "summary" => {
                let today = self.get_user_today(from).await?;
                let stats = self.db.get_daily_stats(from, today).await?;
                let report = crate::services::whatsapp::format_daily_report(
                    stats.total_calories,
                    stats.total_water_ml,
                    stats.meals_count,
                    stats.water_logs_count,
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
                let mut response = "📜 Son 5 Öğün:\n\n".to_string();

                for (i, meal) in meals.iter().enumerate() {
                    response.push_str(&format!(
                        "{}. {} - {:.0} kcal\n   {}\n   {}\n\n",
                        i + 1,
                        meal.meal_type.to_string(),
                        meal.calories,
                        meal.description,
                        meal.created_at.format("%d.%m.%Y %H:%M")
                    ));
                }

                if meals.is_empty() {
                    response = "Henüz kayıtlı öğün yok.".to_string();
                }

                self.whatsapp.send_message(from, &response).await?;
                true
            }
            // Tavsiye komutları
            "tavsiye" | "öneri" | "oneri" | "advice" | "tip" | "tips" => {
                let today = self.get_user_today(from).await?;
                let stats = self.db.get_daily_stats(from, today).await?;

                // Kullanıcının su hedefini al
                let user = self.db.get_user(from).await?;
                let water_goal = user.and_then(|u| u.daily_water_goal).unwrap_or(2000);

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
                        self.whatsapp
                            .send_message(from, "Şu anda tavsiye alınamıyor.")
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

        let message = format!(
            "⚙️ *Ayarlarınız*\n\n\
             🕐 *Öğün Saatleri:*\n\
             Kahvaltı: {} {}\n\
             Öğle: {} {}\n\
             Akşam: {} {}\n\n\
             💧 *Su Ayarları:*\n\
             Hatırlatma: {}\n\
             Hatırlatma Aralığı: {} dakika ({} saat)\n\
             Günlük Hedef: {} ml ({:.1} litre)\n\n\
             🌍 Zaman Dilimi: {}\n\n\
             *Komutlar:* (slash opsiyonel)\n\
             saat kahvalti HH:MM - Kahvaltı saatini değiştir\n\
             saat ogle HH:MM - Öğle yemeği saatini değiştir\n\
             saat aksam HH:MM - Akşam yemeği saatini değiştir\n\
             timezone [IANA timezone] - Zaman dilimini değiştir\n\
             suaraligi [dakika] - Su hatırlatma aralığını değiştir\n\
             suhedefi [ml] - Günlük su hedefini değiştir\n\n\
             Örnekler:\n\
             saat kahvalti 09:00\n\
             timezone America/New_York\n\
             suaraligi 90 (90 dakikada bir hatırlat)\n\
             suhedefi 2500 (2.5 litre hedef)",
            breakfast_time, breakfast_status,
            lunch_time, lunch_status,
            dinner_time, dinner_status,
            water_status,
            water_interval,
            water_interval / 60,
            water_goal,
            water_goal as f64 / 1000.0,
            user.timezone
        );

        self.whatsapp.send_message(from, &message).await?;
        Ok(())
    }

    async fn handle_time_command(&self, from: &str, cmd_parts: &[&str]) -> Result<()> {
        if cmd_parts.len() < 3 {
            self.whatsapp.send_message(
                from,
                "❌ Kullanım: saat [kahvalti|ogle|aksam] HH:MM\n\nÖrnek: saat kahvalti 09:00"
            ).await?;
            return Ok(());
        }

        let meal_type = cmd_parts[1].to_lowercase();
        let time = cmd_parts[2];

        // Validate time format (HH:MM)
        if !time.contains(':') || time.len() != 5 {
            self.whatsapp.send_message(
                from,
                "❌ Geçersiz saat formatı. HH:MM formatında olmalı (örn: 09:00)"
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
                "❌ Kullanım: timezone [IANA timezone]\n\n\
                 Örnekler:\n\
                 timezone Europe/Istanbul\n\
                 timezone America/New_York\n\
                 timezone Asia/Tokyo\n\n\
                 Zaman dilimlerinin listesi: https://en.wikipedia.org/wiki/List_of_tz_database_time_zones"
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
                    &format!("❌ Geçersiz zaman dilimi: {}\n\n\
                             IANA timezone formatında olmalı (örn: Europe/Istanbul)\n\
                             Liste: https://en.wikipedia.org/wiki/List_of_tz_database_time_zones", timezone)
                ).await?;
            }
        }

        Ok(())
    }

    async fn handle_water_interval_command(&self, from: &str, cmd_parts: &[&str]) -> Result<()> {
        if cmd_parts.len() < 2 {
            self.whatsapp.send_message(
                from,
                "❌ Kullanım: suaraligi [dakika]\n\n\
                 Örnekler:\n\
                 suaraligi 60 (1 saatte bir)\n\
                 suaraligi 90 (1.5 saatte bir)\n\
                 suaraligi 120 (2 saatte bir)"
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
                "❌ Kullanım: suhedefi [ml]\n\n\
                 Örnekler:\n\
                 suhedefi 2000 (2 litre)\n\
                 suhedefi 2500 (2.5 litre)\n\
                 suhedefi 3000 (3 litre)"
            ).await?;
            return Ok(());
        }

        let goal_str = cmd_parts[1];
        match goal_str.parse::<i32>() {
            Ok(goal) if goal >= 500 && goal <= 10000 => {
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
                   *Kullanım:*\n\
                   🍽️ Yemek resmi gönder → Kalori analizi\n\
                   💧 'X ml su içtim' yaz → Su kaydı\n\n\
                   *Komutlar:* (slash '/' opsiyonel)\n\
                   📊 rapor, özet → Günlük özet\n\
                   📜 geçmiş, tarihçe → Son öğünler\n\
                   💡 tavsiye, öneri → AI beslenme tavsiyesi (bugünkü verilere göre)\n\
                   ⚙️ ayarlar → Ayarlarını görüntüle\n\
                   🕐 saat [öğün] [HH:MM] → Öğün saatini değiştir\n\
                   🌍 timezone [tz] → Zaman dilimini değiştir\n\
                   💧 suhedefi [ml] → Günlük su hedefinizi değiştir\n\
                   ⏱️ suaraligi [dakika] → Su hatırlatma aralığını değiştir\n\
                   ❓ yardım, ? → Bu mesaj\n\n\
                   *İpucu:* Slash kullanmadan da yazabilirsiniz!\n\
                   Örnek: 'rapor' veya '/rapor' ikisi de çalışır\n\n\
                   *Otomatik hatırlatmalar:*\n\
                   • Kahvaltı, öğle, akşam (zaman dilimine göre)\n\
                   • Su içme (ayarlanabilir, varsayılan 2 saat)";

        self.whatsapp.send_message(to, help).await?;
        Ok(())
    }
}
