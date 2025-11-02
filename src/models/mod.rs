use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub phone_number: String,
    pub created_at: DateTime<Utc>,
    pub onboarding_completed: bool,
    pub onboarding_step: Option<String>,
    pub breakfast_reminder: bool,
    pub lunch_reminder: bool,
    pub dinner_reminder: bool,
    pub water_reminder: bool,
    pub breakfast_time: Option<String>,  // HH:MM format (örn: "09:00")
    pub lunch_time: Option<String>,      // HH:MM format
    pub dinner_time: Option<String>,     // HH:MM format
    pub opted_in: bool,  // Kullanıcı mesaj gönderdi mi?
    pub timezone: String,  // IANA timezone (örn: "Europe/Istanbul", "America/New_York")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meal {
    pub id: Option<i64>,
    pub user_phone: String,
    pub meal_type: MealType,
    pub calories: f64,
    pub description: String,
    pub image_path: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MealType {
    Breakfast,
    Lunch,
    Dinner,
    Snack,
}

impl MealType {
    pub fn to_string(&self) -> String {
        match self {
            MealType::Breakfast => "Kahvaltı".to_string(),
            MealType::Lunch => "Öğle Yemeği".to_string(),
            MealType::Dinner => "Akşam Yemeği".to_string(),
            MealType::Snack => "Ara Öğün".to_string(),
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "kahvaltı" | "breakfast" => Some(MealType::Breakfast),
            "öğle" | "lunch" => Some(MealType::Lunch),
            "akşam" | "dinner" => Some(MealType::Dinner),
            "ara" | "snack" => Some(MealType::Snack),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterLog {
    pub id: Option<i64>,
    pub user_phone: String,
    pub amount_ml: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStats {
    pub user_phone: String,
    pub date: String,
    pub total_calories: f64,
    pub total_water_ml: i64,
    pub meals_count: i64,
    pub water_logs_count: i64,
}
