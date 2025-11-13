use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use std::sync::Arc;

use crate::models::{Conversation, Meal, User};
use crate::services::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStats {
    pub user: User,
    pub total_meals: i64,
    pub total_conversations: i64,
    pub total_calories_today: f64,
    pub total_water_today: i64,
    pub last_activity: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyTrend {
    pub day: String,
    pub active_users: i64,
    pub total_meals: i64,
    pub avg_calories: f64,
    pub total_water_ml: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminDashboardData {
    pub total_users: i64,
    pub active_users_today: i64,
    pub total_meals_today: i64,
    pub total_conversations_today: i64,
    pub avg_calories_per_user_today: f64,
    pub avg_water_per_user_today: i64,
    pub weekly_trends: Vec<WeeklyTrend>,
    pub users: Vec<UserStats>,
}

pub struct AdminService {
    pub db: Arc<Database>,
}

impl AdminService {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Get all users with their stats
    pub async fn get_all_user_stats(&self) -> Result<Vec<UserStats>> {
        let users = self.db.get_all_users().await?;
        let mut stats = Vec::new();

        for user in users {
            // Use user's timezone for accurate "today" calculation
            let user_tz: chrono_tz::Tz = user.timezone.parse()
                .unwrap_or(chrono_tz::Europe::Istanbul);
            let today = chrono::Utc::now().with_timezone(&user_tz).date_naive();

            let total_meals = self.get_user_total_meals(&user.phone_number).await?;
            let total_conversations = self.db.get_conversation_count(&user.phone_number).await?;
            let daily_stats = self.db.get_daily_stats(&user.phone_number, today).await?;
            let last_activity = self.get_user_last_activity(&user.phone_number).await?;

            log::info!(
                "📊 Admin Stats for {}: calories={}, water={}, meals_count={}",
                user.phone_number,
                daily_stats.total_calories,
                daily_stats.total_water_ml,
                daily_stats.meals_count
            );

            stats.push(UserStats {
                user,
                total_meals,
                total_conversations,
                total_calories_today: daily_stats.total_calories,
                total_water_today: daily_stats.total_water_ml,
                last_activity,
            });
        }

        Ok(stats)
    }

    /// Get dashboard overview data with enhanced insights
    pub async fn get_dashboard_data(&self) -> Result<AdminDashboardData> {
        let user_stats = self.get_all_user_stats().await?;
        let total_users = user_stats.len() as i64;

        let today = chrono::Utc::now().date_naive();
        let active_users_today = user_stats
            .iter()
            .filter(|s| {
                if let Some(last_activity) = s.last_activity {
                    last_activity.date_naive() == today
                } else {
                    false
                }
            })
            .count() as i64;

        // Count total meals logged today across all users
        let total_meals_today: i64 = self.get_total_meals_today().await?;

        let total_conversations_today = user_stats
            .iter()
            .map(|s| s.total_conversations)
            .sum();

        // Calculate averages for today
        let total_calories_today: f64 = user_stats.iter().map(|s| s.total_calories_today).sum();
        let total_water_today: i64 = user_stats.iter().map(|s| s.total_water_today).sum();

        let avg_calories_per_user_today = if total_users > 0 {
            total_calories_today / total_users as f64
        } else {
            0.0
        };

        let avg_water_per_user_today = if total_users > 0 {
            total_water_today / total_users
        } else {
            0
        };

        // Generate weekly trends
        let weekly_trends = self.get_weekly_trends().await?;

        Ok(AdminDashboardData {
            total_users,
            active_users_today,
            total_meals_today,
            total_conversations_today,
            avg_calories_per_user_today,
            avg_water_per_user_today,
            weekly_trends,
            users: user_stats,
        })
    }

    /// Get weekly trends for the dashboard
    async fn get_weekly_trends(&self) -> Result<Vec<WeeklyTrend>> {
        use chrono::Datelike;

        let today = chrono::Utc::now().date_naive();
        let mut trends = Vec::new();

        for i in (0..7).rev() {
            let date = today - chrono::Duration::days(i);
            let users = self.db.get_all_users().await?;

            let mut active_count = 0i64;
            let mut total_meals = 0i64;
            let mut total_calories = 0.0;
            let mut total_water = 0i64;

            for user in users {
                let daily_stats = self.db.get_daily_stats(&user.phone_number, date).await?;

                if daily_stats.meals_count > 0 || daily_stats.total_water_ml > 0 {
                    active_count += 1;
                }

                total_meals += daily_stats.meals_count;
                total_calories += daily_stats.total_calories;
                total_water += daily_stats.total_water_ml;
            }

            let day_name = match date.weekday() {
                chrono::Weekday::Mon => "Pzt",
                chrono::Weekday::Tue => "Sal",
                chrono::Weekday::Wed => "Çar",
                chrono::Weekday::Thu => "Per",
                chrono::Weekday::Fri => "Cum",
                chrono::Weekday::Sat => "Cmt",
                chrono::Weekday::Sun => "Paz",
            };

            let avg_calories = if active_count > 0 {
                total_calories / active_count as f64
            } else {
                0.0
            };

            trends.push(WeeklyTrend {
                day: format!("{} {}", day_name, date.format("%d.%m")),
                active_users: active_count,
                total_meals,
                avg_calories,
                total_water_ml: total_water,
            });
        }

        Ok(trends)
    }

    /// Get total meals logged today across all users
    async fn get_total_meals_today(&self) -> Result<i64> {
        let users = self.db.get_all_users().await?;

        let mut total = 0i64;
        for user in users {
            // Use user's timezone for accurate "today" calculation
            let user_tz: chrono_tz::Tz = user.timezone.parse()
                .unwrap_or(chrono_tz::Europe::Istanbul);
            let today = chrono::Utc::now().with_timezone(&user_tz).date_naive();

            let daily_stats = self.db.get_daily_stats(&user.phone_number, today).await?;
            total += daily_stats.meals_count;
        }

        Ok(total)
    }

    /// Get specific user's meals
    pub async fn get_user_meals(&self, phone_number: &str, limit: i32) -> Result<Vec<Meal>> {
        self.db.get_recent_meals(phone_number, limit).await
    }

    /// Get specific user's conversations
    pub async fn get_user_conversations(&self, phone_number: &str, limit: i32) -> Result<Vec<Conversation>> {
        self.db.get_conversation_history(phone_number, limit).await
    }

    /// Get total meal count for a user
    async fn get_user_total_meals(&self, phone_number: &str) -> Result<i64> {
        // This is a helper to get total meals count across all time
        let meals = self.db.get_recent_meals(phone_number, 1000).await?;
        Ok(meals.len() as i64)
    }

    /// Get user's last activity timestamp (based on incoming messages only)
    async fn get_user_last_activity(&self, phone_number: &str) -> Result<Option<DateTime<Utc>>> {
        let conversations = self.db.get_conversation_history(phone_number, 100).await?;

        // Find the most recent incoming message
        let last_incoming = conversations
            .iter()
            .filter(|c| matches!(c.direction, crate::models::ConversationDirection::Incoming))
            .map(|c| c.created_at)
            .max();

        Ok(last_incoming)
    }

    /// Toggle user active status
    pub async fn toggle_user_active(&self, phone_number: &str) -> Result<bool> {
        self.db.toggle_user_active(phone_number).await
    }

    /// Reset user completely - deletes all data and resets to fresh state
    pub async fn reset_user(&self, phone_number: &str) -> Result<()> {
        self.db.reset_user(phone_number).await
    }
}
