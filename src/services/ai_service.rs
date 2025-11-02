use anyhow::Result;

#[derive(Debug, Clone)]
pub struct CalorieInfo {
    pub calories: f64,
    pub description: String,
}

/// Trait for AI services (OpenAI, OpenRouter, etc.)
#[async_trait::async_trait]
pub trait AIService: Send + Sync {
    async fn analyze_food_image(&self, image_path: &str) -> Result<CalorieInfo>;
    async fn get_nutrition_advice(&self, daily_calories: f64, daily_water: i64) -> Result<String>;
}
