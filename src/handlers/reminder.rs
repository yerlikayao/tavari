use anyhow::Result;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::models::{ConversationDirection, MessageType};
use crate::services::{Database, WhatsAppService};

pub struct ReminderService {
    db: Arc<Database>,
    whatsapp: Arc<dyn WhatsAppService>,
    scheduler: JobScheduler,
}

impl ReminderService {
    pub async fn new(db: Arc<Database>, whatsapp: Arc<dyn WhatsAppService>) -> Result<Self> {
        let scheduler = JobScheduler::new().await?;

        Ok(Self {
            db,
            whatsapp,
            scheduler,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        // Personalized meal reminders - Her 30 dakikada bir kontrol et
        self.add_personalized_meal_reminders().await?;

        // Su içme hatırlatması (Her 2 saatte bir, 08:00-22:00 arası)
        self.add_water_reminder("0 0 8,10,12,14,16,18,20,22 * * *").await?;

        // Günlük özet (22:00)
        self.add_daily_summary("0 0 22 * * *").await?;

        self.scheduler.start().await?;

        log::info!("✅ Reminder service started (personalized)");
        Ok(())
    }

    async fn add_personalized_meal_reminders(&mut self) -> Result<()> {
        let db = self.db.clone();
        let whatsapp = self.whatsapp.clone();

        // Her 30 dakikada bir çalış ve kullanıcıların öğün saatlerini kontrol et
        let job = Job::new_async("0 0,30 * * * *", move |_uuid, _l| {
            let db = db.clone();
            let whatsapp = whatsapp.clone();

            Box::pin(async move {
                use chrono::Utc;
                use chrono::Timelike;
                use chrono_tz::Tz;

                if let Ok(users) = db.get_active_users().await {
                    log::debug!("🔄 Meal reminder check running for {} users", users.len());
                    for user in users {
                        if !user.onboarding_completed {
                            log::debug!("⏭️ Skipping {} - onboarding not completed", user.phone_number);
                            continue;
                        }

                        // Kullanıcının timezone'unda mevcut saati hesapla
                        let user_tz: Tz = user.timezone.parse().unwrap_or(chrono_tz::Europe::Istanbul);
                        let now_utc = Utc::now();
                        let now_user = now_utc.with_timezone(&user_tz);
                        let current_time = now_user.format("%H:%M").to_string();

                        log::debug!("⏰ User {} - Current time: {} (TZ: {})", user.phone_number, current_time, user.timezone);

                        // Check silent hours
                        let silent_start = user.silent_hours_start.as_deref().unwrap_or("23:00");
                        let silent_end = user.silent_hours_end.as_deref().unwrap_or("07:00");
                        let is_silent = Self::is_silent_hours(
                            now_user.hour(),
                            now_user.minute(),
                            silent_start,
                            silent_end,
                        );

                        if is_silent {
                            log::debug!("🌙 User {} - In silent hours ({} - {}), skipping meal reminders", user.phone_number, silent_start, silent_end);
                            continue;
                        }

                        // Kahvaltı kontrolü
                        if user.breakfast_reminder {
                            if let Some(ref breakfast_time) = user.breakfast_time {
                                log::debug!("🍳 Checking breakfast for {}: current={}, target={}", user.phone_number, current_time, breakfast_time);
                                if &current_time == breakfast_time {
                                    // Bugün kahvaltı kaydedilmiş mi kontrol et
                                    let today = now_user.date_naive();
                                    if let Ok(todays_meals) = db.get_todays_meal_types(&user.phone_number, today).await {
                                        let has_breakfast = todays_meals.iter().any(|m| matches!(m, crate::models::MealType::Breakfast));

                                        if has_breakfast {
                                            log::debug!("⏭️ Skipping breakfast reminder for {} - already logged today", user.phone_number);
                                        } else {
                                            let msg = "☀️ *Kahvaltı zamanı!*\n\nYedikten sonra fotoğrafını gönder 📸";
                                            let _ = whatsapp.send_message(&user.phone_number, msg).await;

                                            // Log reminder
                                            let _ = db.log_conversation(
                                                &user.phone_number,
                                                ConversationDirection::Outgoing,
                                                MessageType::Reminder,
                                                msg,
                                                Some(serde_json::json!({"reminder_type": "breakfast", "time": breakfast_time})),
                                            ).await;

                                            log::info!("📤 Sent breakfast reminder to {} ({})", user.phone_number, user.timezone);
                                        }
                                    }
                                }
                            }
                        }

                        // Öğle yemeği kontrolü
                        if user.lunch_reminder {
                            if let Some(ref lunch_time) = user.lunch_time {
                                log::debug!("🍱 Checking lunch for {}: current={}, target={}", user.phone_number, current_time, lunch_time);
                                if &current_time == lunch_time {
                                    // Bugün öğle yemeği kaydedilmiş mi kontrol et
                                    let today = now_user.date_naive();
                                    if let Ok(todays_meals) = db.get_todays_meal_types(&user.phone_number, today).await {
                                        let has_lunch = todays_meals.iter().any(|m| matches!(m, crate::models::MealType::Lunch));

                                        if has_lunch {
                                            log::debug!("⏭️ Skipping lunch reminder for {} - already logged today", user.phone_number);
                                        } else {
                                            let msg = "🌞 *Öğle yemeği zamanı!*\n\nYedikten sonra fotoğrafını gönder 📸";
                                            let _ = whatsapp.send_message(&user.phone_number, msg).await;

                                            // Log reminder
                                            let _ = db.log_conversation(
                                                &user.phone_number,
                                                ConversationDirection::Outgoing,
                                                MessageType::Reminder,
                                                msg,
                                                Some(serde_json::json!({"reminder_type": "lunch", "time": lunch_time})),
                                            ).await;

                                            log::info!("📤 Sent lunch reminder to {} ({})", user.phone_number, user.timezone);
                                        }
                                    }
                                }
                            }
                        }

                        // Akşam yemeği kontrolü
                        if user.dinner_reminder {
                            if let Some(ref dinner_time) = user.dinner_time {
                                log::debug!("🍽️ Checking dinner for {}: current={}, target={}", user.phone_number, current_time, dinner_time);
                                if &current_time == dinner_time {
                                    // Bugün akşam yemeği kaydedilmiş mi kontrol et
                                    let today = now_user.date_naive();
                                    if let Ok(todays_meals) = db.get_todays_meal_types(&user.phone_number, today).await {
                                        let has_dinner = todays_meals.iter().any(|m| matches!(m, crate::models::MealType::Dinner));

                                        if has_dinner {
                                            log::debug!("⏭️ Skipping dinner reminder for {} - already logged today", user.phone_number);
                                        } else {
                                            let msg = "🌙 *Akşam yemeği zamanı!*\n\nYedikten sonra fotoğrafını gönder 📸";
                                            let _ = whatsapp.send_message(&user.phone_number, msg).await;

                                            // Log reminder
                                            let _ = db.log_conversation(
                                                &user.phone_number,
                                                ConversationDirection::Outgoing,
                                                MessageType::Reminder,
                                                msg,
                                                Some(serde_json::json!({"reminder_type": "dinner", "time": dinner_time})),
                                            ).await;

                                            log::info!("📤 Sent dinner reminder to {} ({})", user.phone_number, user.timezone);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    log::debug!("✅ Meal reminder check completed");
                }
            })
        })?;

        self.scheduler.add(job).await?;
        log::info!("✅ Added personalized meal reminders (checks every 30 min)");
        Ok(())
    }

    async fn add_water_reminder(&mut self, _schedule: &str) -> Result<()> {
        let db = self.db.clone();
        let whatsapp = self.whatsapp.clone();

        // Her saat başı kontrol et, kullanıcı timezone'unda su içme saatleri (8,10,12,14,16,18,20,22)
        let job = Job::new_async("0 0 * * * *", move |_uuid, _l| {
            let db = db.clone();
            let whatsapp = whatsapp.clone();

            Box::pin(async move {
                use chrono::Utc;
                use chrono::Timelike;
                use chrono_tz::Tz;

                let message = "💧 *Su içme zamanı!*\n\nEn az 1 bardak su iç.\nKaydet: 250 ml su içtim";

                if let Ok(users) = db.get_active_users().await {
                    log::debug!("💧 Water reminder check running for {} users", users.len());
                    for user in users {
                        if user.water_reminder && user.onboarding_completed {
                            // Kullanıcının timezone'unda mevcut saati hesapla
                            let user_tz: Tz = user.timezone.parse().unwrap_or(chrono_tz::Europe::Istanbul);
                            let now_utc = Utc::now();
                            let now_user = now_utc.with_timezone(&user_tz);
                            let current_hour = now_user.hour();

                            log::debug!("💧 User {} - Current hour: {} (TZ: {}), checking if in [8,10,12,14,16,18,20,22]", user.phone_number, current_hour, user.timezone);

                            // Check silent hours
                            let silent_start = user.silent_hours_start.as_deref().unwrap_or("23:00");
                            let silent_end = user.silent_hours_end.as_deref().unwrap_or("07:00");
                            let is_silent = Self::is_silent_hours(
                                now_user.hour(),
                                now_user.minute(),
                                silent_start,
                                silent_end,
                            );

                            if is_silent {
                                log::debug!("🌙 User {} - In silent hours ({} - {}), skipping water reminder", user.phone_number, silent_start, silent_end);
                                continue;
                            }

                            // Su içme saatleri: 8,10,12,14,16,18,20,22
                            if [8, 10, 12, 14, 16, 18, 20, 22].contains(&current_hour) {
                                let _ = whatsapp.send_message(&user.phone_number, message).await;

                                // Log water reminder
                                let _ = db.log_conversation(
                                    &user.phone_number,
                                    ConversationDirection::Outgoing,
                                    MessageType::Reminder,
                                    message,
                                    Some(serde_json::json!({"reminder_type": "water", "hour": current_hour})),
                                ).await;

                                log::info!("📤 Sent water reminder to {} at {}:00 ({})", user.phone_number, current_hour, user.timezone);
                            }
                        } else {
                            log::debug!("⏭️ Skipping water reminder for {} (reminder={}, onboarded={})", user.phone_number, user.water_reminder, user.onboarding_completed);
                        }
                    }
                    log::debug!("✅ Water reminder check completed");
                }
            })
        })?;

        self.scheduler.add(job).await?;
        log::info!("Added water reminder (timezone-aware)");
        Ok(())
    }

    async fn add_daily_summary(&mut self, _schedule: &str) -> Result<()> {
        let db = self.db.clone();
        let whatsapp = self.whatsapp.clone();

        // Her saat başı kontrol et, kullanıcı timezone'unda 22:00'da gönder
        let job = Job::new_async("0 0 * * * *", move |_uuid, _l| {
            let db = db.clone();
            let whatsapp = whatsapp.clone();

            Box::pin(async move {
                use chrono::Utc;
                use chrono::Timelike;
                use chrono_tz::Tz;

                if let Ok(users) = db.get_active_users().await {
                    log::debug!("📊 Daily summary check running for {} users", users.len());
                    for user in users {
                        if !user.onboarding_completed {
                            log::debug!("⏭️ Skipping {} - onboarding not completed", user.phone_number);
                            continue;
                        }

                        // Kullanıcının timezone'unda mevcut saati hesapla
                        let user_tz: Tz = user.timezone.parse().unwrap_or(chrono_tz::Europe::Istanbul);
                        let now_utc = Utc::now();
                        let now_user = now_utc.with_timezone(&user_tz);
                        let current_hour = now_user.hour();

                        log::debug!("📊 User {} - Current hour: {} (TZ: {}), checking if == 22", user.phone_number, current_hour, user.timezone);

                        // 22:00'da günlük özet gönder
                        if current_hour == 22 {
                            let today = now_user.date_naive();
                            if let Ok(stats) = db.get_daily_stats(&user.phone_number, today).await {
                                let report = crate::services::whatsapp::format_daily_report(
                                    stats.total_calories,
                                    stats.total_water_ml,
                                    stats.meals_count,
                                    stats.water_logs_count,
                                    user.daily_calorie_goal.unwrap_or(2000),
                                    user.daily_water_goal.unwrap_or(2000),
                                );

                                let message = format!("🌙 *Günlük Özet*\n\n{}", report);
                                let _ = whatsapp.send_message(&user.phone_number, &message).await;

                                // Log daily summary
                                let _ = db.log_conversation(
                                    &user.phone_number,
                                    ConversationDirection::Outgoing,
                                    MessageType::Reminder,
                                    &message,
                                    Some(serde_json::json!({
                                        "reminder_type": "daily_summary",
                                        "calories": stats.total_calories,
                                        "water_ml": stats.total_water_ml,
                                        "meals_count": stats.meals_count
                                    })),
                                ).await;

                                log::info!("📤 Sent daily summary to {} at 22:00 ({})", user.phone_number, user.timezone);
                            }
                        }
                    }
                    log::debug!("✅ Daily summary check completed");
                }
            })
        })?;

        self.scheduler.add(job).await?;
        log::info!("Added daily summary reminder (timezone-aware)");
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        self.scheduler.shutdown().await?;
        log::info!("Reminder service stopped");
        Ok(())
    }

    /// Check if current time is within user's silent hours
    /// Silent hours can cross midnight (e.g., 23:00 - 07:00)
    fn is_silent_hours(
        current_hour: u32,
        current_minute: u32,
        start: &str,
        end: &str,
    ) -> bool {
        // Parse start and end times
        let parse_time = |time_str: &str| -> Option<(u32, u32)> {
            let parts: Vec<&str> = time_str.split(':').collect();
            if parts.len() != 2 {
                return None;
            }
            let h = parts[0].parse::<u32>().ok()?;
            let m = parts[1].parse::<u32>().ok()?;
            Some((h, m))
        };

        let (start_h, start_m) = match parse_time(start) {
            Some(t) => t,
            None => return false, // Invalid format, don't block
        };

        let (end_h, end_m) = match parse_time(end) {
            Some(t) => t,
            None => return false,
        };

        // Convert to minutes since midnight for easier comparison
        let current_minutes = current_hour * 60 + current_minute;
        let start_minutes = start_h * 60 + start_m;
        let end_minutes = end_h * 60 + end_m;

        if start_minutes < end_minutes {
            // Normal case: e.g., 08:00 - 22:00
            current_minutes >= start_minutes && current_minutes < end_minutes
        } else {
            // Crosses midnight: e.g., 23:00 - 07:00
            current_minutes >= start_minutes || current_minutes < end_minutes
        }
    }
}
