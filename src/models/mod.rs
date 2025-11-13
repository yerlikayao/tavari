use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub phone_number: String,
    pub name: Option<String>,  // WhatsApp profil ismi
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
    pub water_reminder_interval: Option<i32>,  // Su hatırlatma aralığı (dakika cinsinden, varsayılan: 120)
    pub daily_water_goal: Option<i32>,  // Günlük su hedefi (ml cinsinden, varsayılan: 2000)
    pub daily_calorie_goal: Option<i32>,  // Günlük kalori hedefi (kcal cinsinden, varsayılan: 2000)
    pub silent_hours_start: Option<String>,  // Sessiz saatler başlangıcı (HH:MM, varsayılan: "23:00")
    pub silent_hours_end: Option<String>,    // Sessiz saatler bitişi (HH:MM, varsayılan: "07:00")
    pub is_active: bool,  // Kullanıcı aktif mi? (false ise sistem ona mesaj atmaz)
    pub pending_command: Option<String>,  // AI tarafından önerilen komut (onay bekliyor)
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

impl std::fmt::Display for MealType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            MealType::Breakfast => "Kahvaltı",
            MealType::Lunch => "Öğle Yemeği",
            MealType::Dinner => "Akşam Yemeği",
            MealType::Snack => "Ara Öğün",
        };
        write!(f, "{}", s)
    }
}

impl MealType {
    pub fn from_string(s: &str) -> Option<Self> {
        // Normalize Turkish characters: İ->i, I->ı before lowercase
        let normalized = s
            .replace('İ', "i")
            .replace('I', "ı")
            .to_lowercase();

        match normalized.as_str() {
            "kahvaltı" | "breakfast" => Some(MealType::Breakfast),
            "öğle yemeği" | "öğle" | "lunch" => Some(MealType::Lunch),
            "akşam yemeği" | "akşam" | "dinner" => Some(MealType::Dinner),
            "ara öğün" | "ara" | "snack" => Some(MealType::Snack),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Option<i64>,
    pub user_phone: String,
    pub direction: ConversationDirection,
    pub message_type: MessageType,
    pub content: String,
    pub metadata: Option<serde_json::Value>,  // Extra metadata as JSON
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConversationDirection {
    Incoming,  // User → Bot
    Outgoing,  // Bot → User
}

impl std::fmt::Display for ConversationDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ConversationDirection::Incoming => "incoming",
            ConversationDirection::Outgoing => "outgoing",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    Text,       // Regular text message
    Image,      // Image with food
    Command,    // User command (rapor, ayarlar, etc.)
    Response,   // Bot response to command
    Reminder,   // Automatic reminder
    Error,      // Error message
}
