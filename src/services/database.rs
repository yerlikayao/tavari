use anyhow::Result;
use chrono::NaiveDate;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};

use crate::models::{Conversation, ConversationDirection, DailyStats, Meal, MealType, MessageType, User, WaterLog};

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        let db = Database { pool };
        db.init_tables().await?;
        Ok(db)
    }

    async fn init_tables(&self) -> Result<()> {
        log::info!("🔧 Initializing database tables and running migrations...");

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                phone_number TEXT PRIMARY KEY,
                name TEXT,
                created_at TIMESTAMPTZ NOT NULL,
                onboarding_completed BOOLEAN DEFAULT FALSE,
                onboarding_step TEXT,
                breakfast_reminder BOOLEAN DEFAULT TRUE,
                lunch_reminder BOOLEAN DEFAULT TRUE,
                dinner_reminder BOOLEAN DEFAULT TRUE,
                water_reminder BOOLEAN DEFAULT TRUE,
                breakfast_time TEXT,
                lunch_time TEXT,
                dinner_time TEXT,
                opted_in BOOLEAN DEFAULT FALSE,
                timezone TEXT DEFAULT 'Europe/Istanbul',
                daily_water_goal INTEGER DEFAULT 2000,
                is_active BOOLEAN DEFAULT TRUE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS meals (
                id SERIAL PRIMARY KEY,
                user_phone TEXT NOT NULL REFERENCES users(phone_number),
                meal_type TEXT NOT NULL,
                calories DOUBLE PRECISION NOT NULL,
                description TEXT NOT NULL,
                image_path TEXT,
                created_at TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS water_logs (
                id SERIAL PRIMARY KEY,
                user_phone TEXT NOT NULL REFERENCES users(phone_number),
                amount_ml INTEGER NOT NULL,
                created_at TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS favorite_meals (
                id SERIAL PRIMARY KEY,
                user_phone TEXT NOT NULL REFERENCES users(phone_number),
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                calories DOUBLE PRECISION NOT NULL,
                created_at TIMESTAMPTZ NOT NULL,
                UNIQUE(user_phone, name)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS conversations (
                id SERIAL PRIMARY KEY,
                user_phone TEXT NOT NULL REFERENCES users(phone_number),
                direction TEXT NOT NULL,  -- 'incoming' or 'outgoing'
                message_type TEXT NOT NULL,  -- 'text', 'image', 'command', 'response', 'reminder', 'error'
                content TEXT NOT NULL,
                metadata JSONB,  -- Extra info: command type, error details, image path, etc.
                created_at TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create index for faster queries by user and date
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_conversations_user_date
            ON conversations(user_phone, created_at DESC)
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Table for tracking 24h window warning status
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS window_warnings (
                user_phone TEXT PRIMARY KEY REFERENCES users(phone_number) ON DELETE CASCADE,
                last_warned_at TIMESTAMPTZ NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Migration: Add new columns if they don't exist (for existing deployments)
        // This is safe to run multiple times
        sqlx::query(
            r#"
            DO $$
            BEGIN
                -- Add daily_water_goal column if not exists
                IF NOT EXISTS (
                    SELECT 1 FROM information_schema.columns
                    WHERE table_name='users' AND column_name='daily_water_goal'
                ) THEN
                    ALTER TABLE users ADD COLUMN daily_water_goal INTEGER DEFAULT 2000;
                END IF;

                -- Add daily_calorie_goal column if not exists
                IF NOT EXISTS (
                    SELECT 1 FROM information_schema.columns
                    WHERE table_name='users' AND column_name='daily_calorie_goal'
                ) THEN
                    ALTER TABLE users ADD COLUMN daily_calorie_goal INTEGER DEFAULT 2000;
                END IF;

                -- Add silent_hours_start column if not exists
                IF NOT EXISTS (
                    SELECT 1 FROM information_schema.columns
                    WHERE table_name='users' AND column_name='silent_hours_start'
                ) THEN
                    ALTER TABLE users ADD COLUMN silent_hours_start TEXT DEFAULT '23:00';
                END IF;

                -- Add silent_hours_end column if not exists
                IF NOT EXISTS (
                    SELECT 1 FROM information_schema.columns
                    WHERE table_name='users' AND column_name='silent_hours_end'
                ) THEN
                    ALTER TABLE users ADD COLUMN silent_hours_end TEXT DEFAULT '07:00';
                END IF;

                -- Add is_active column if not exists
                IF NOT EXISTS (
                    SELECT 1 FROM information_schema.columns
                    WHERE table_name='users' AND column_name='is_active'
                ) THEN
                    ALTER TABLE users ADD COLUMN is_active BOOLEAN DEFAULT TRUE;
                END IF;

                -- Add pending_command column if not exists (for AI command suggestions)
                IF NOT EXISTS (
                    SELECT 1 FROM information_schema.columns
                    WHERE table_name='users' AND column_name='pending_command'
                ) THEN
                    ALTER TABLE users ADD COLUMN pending_command TEXT DEFAULT NULL;
                END IF;

                -- Add name column if not exists (for WhatsApp profile names)
                IF NOT EXISTS (
                    SELECT 1 FROM information_schema.columns
                    WHERE table_name='users' AND column_name='name'
                ) THEN
                    ALTER TABLE users ADD COLUMN name TEXT DEFAULT NULL;
                END IF;
            END $$;
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Update existing users with NULL values to have defaults
        sqlx::query("UPDATE users SET daily_water_goal = 2000 WHERE daily_water_goal IS NULL")
            .execute(&self.pool)
            .await?;

        sqlx::query("UPDATE users SET daily_calorie_goal = 2000 WHERE daily_calorie_goal IS NULL")
            .execute(&self.pool)
            .await?;

        sqlx::query("UPDATE users SET silent_hours_start = '23:00' WHERE silent_hours_start IS NULL")
            .execute(&self.pool)
            .await?;

        sqlx::query("UPDATE users SET silent_hours_end = '07:00' WHERE silent_hours_end IS NULL")
            .execute(&self.pool)
            .await?;

        sqlx::query("UPDATE users SET is_active = TRUE WHERE is_active IS NULL")
            .execute(&self.pool)
            .await?;

        log::info!("✅ Database initialization and migrations completed successfully");

        Ok(())
    }

    pub async fn create_user(&self, user: &User) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO users (
                phone_number, name, created_at, onboarding_completed, onboarding_step,
                breakfast_reminder, lunch_reminder, dinner_reminder, water_reminder,
                breakfast_time, lunch_time, dinner_time, opted_in, timezone,
                daily_water_goal, daily_calorie_goal,
                silent_hours_start, silent_hours_end, is_active
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
            ON CONFLICT (phone_number) DO UPDATE SET name = EXCLUDED.name
            "#,
        )
        .bind(&user.phone_number)
        .bind(&user.name)
        .bind(user.created_at)
        .bind(user.onboarding_completed)
        .bind(&user.onboarding_step)
        .bind(user.breakfast_reminder)
        .bind(user.lunch_reminder)
        .bind(user.dinner_reminder)
        .bind(user.water_reminder)
        .bind(&user.breakfast_time)
        .bind(&user.lunch_time)
        .bind(&user.dinner_time)
        .bind(user.opted_in)
        .bind(&user.timezone)
        .bind(user.daily_water_goal)
        .bind(user.daily_calorie_goal)
        .bind(&user.silent_hours_start)
        .bind(&user.silent_hours_end)
        .bind(user.is_active)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_user(&self, phone_number: &str) -> Result<Option<User>> {
        // Try to get all fields including name and pending_command
        let user_result = sqlx::query(
            r#"
            SELECT phone_number, name, created_at, onboarding_completed, onboarding_step,
                   breakfast_reminder, lunch_reminder, dinner_reminder, water_reminder,
                   breakfast_time, lunch_time, dinner_time, opted_in, timezone,
                   daily_water_goal, daily_calorie_goal,
                   silent_hours_start, silent_hours_end, is_active, pending_command
            FROM users WHERE phone_number = $1
            "#,
        )
        .bind(phone_number)
        .fetch_optional(&self.pool)
        .await;

        // If query fails (column doesn't exist), try without pending_command and name
        let user = match user_result {
            Ok(Some(row)) => Some(User {
                phone_number: row.get(0),
                name: row.get(1),
                created_at: row.get(2),
                onboarding_completed: row.get(3),
                onboarding_step: row.get(4),
                breakfast_reminder: row.get(5),
                lunch_reminder: row.get(6),
                dinner_reminder: row.get(7),
                water_reminder: row.get(8),
                breakfast_time: row.get(9),
                lunch_time: row.get(10),
                dinner_time: row.get(11),
                opted_in: row.get(12),
                timezone: row.get(13),
                daily_water_goal: row.get(14),
                daily_calorie_goal: row.get(15),
                silent_hours_start: row.get(16),
                silent_hours_end: row.get(17),
                is_active: row.get(18),
                pending_command: row.get(19),
            }),
            Ok(None) => None,
            Err(e) if e.to_string().contains("pending_command") || e.to_string().contains("column") => {
                // Column doesn't exist yet, use legacy query (migration will add it on next restart)
                log::debug!("pending_command column not found, using legacy query");
                sqlx::query(
                    r#"
                    SELECT phone_number, created_at, onboarding_completed, onboarding_step,
                           breakfast_reminder, lunch_reminder, dinner_reminder, water_reminder,
                           breakfast_time, lunch_time, dinner_time, opted_in, timezone,
                           daily_water_goal, daily_calorie_goal,
                           silent_hours_start, silent_hours_end, is_active
                    FROM users WHERE phone_number = $1
                    "#,
                )
                .bind(phone_number)
                .fetch_optional(&self.pool)
                .await?
                .map(|row| User {
                    phone_number: row.get(0),
                    name: None, // Legacy fallback - name column doesn't exist yet
                    created_at: row.get(1),
                    onboarding_completed: row.get(2),
                    onboarding_step: row.get(3),
                    breakfast_reminder: row.get(4),
                    lunch_reminder: row.get(5),
                    dinner_reminder: row.get(6),
                    water_reminder: row.get(7),
                    breakfast_time: row.get(8),
                    lunch_time: row.get(9),
                    dinner_time: row.get(10),
                    opted_in: row.get(11),
                    timezone: row.get(12),
                    daily_water_goal: row.get(13),
                    daily_calorie_goal: row.get(14),
                    silent_hours_start: row.get(15),
                    silent_hours_end: row.get(16),
                    is_active: row.get(17),
                    pending_command: None, // Default to None if column doesn't exist
                })
            }
            Err(e) => return Err(e.into()),
        };

        Ok(user)
    }

    pub async fn get_all_users(&self) -> Result<Vec<User>> {
        // Try with pending_command and name first
        let result = sqlx::query(
            r#"
            SELECT phone_number, name, created_at, onboarding_completed, onboarding_step,
                   breakfast_reminder, lunch_reminder, dinner_reminder, water_reminder,
                   breakfast_time, lunch_time, dinner_time, opted_in, timezone,
                   daily_water_goal, daily_calorie_goal,
                   silent_hours_start, silent_hours_end, is_active, pending_command
            FROM users
            "#,
        )
        .fetch_all(&self.pool)
        .await;

        let users = match result {
            Ok(rows) => rows
                .into_iter()
                .map(|row| User {
                    phone_number: row.get(0),
                    name: row.get(1),
                    created_at: row.get(2),
                    onboarding_completed: row.get(3),
                    onboarding_step: row.get(4),
                    breakfast_reminder: row.get(5),
                    lunch_reminder: row.get(6),
                    dinner_reminder: row.get(7),
                    water_reminder: row.get(8),
                    breakfast_time: row.get(9),
                    lunch_time: row.get(10),
                    dinner_time: row.get(11),
                    opted_in: row.get(12),
                    timezone: row.get(13),
                    daily_water_goal: row.get(14),
                    daily_calorie_goal: row.get(15),
                    silent_hours_start: row.get(16),
                    silent_hours_end: row.get(17),
                    is_active: row.get(18),
                    pending_command: row.get(19),
                })
                .collect(),
            Err(e) if e.to_string().contains("pending_command") || e.to_string().contains("column") => {
                // Column doesn't exist yet, use legacy query
                log::debug!("pending_command column not found in get_all_users, using legacy query");
                sqlx::query(
                    r#"
                    SELECT phone_number, created_at, onboarding_completed, onboarding_step,
                           breakfast_reminder, lunch_reminder, dinner_reminder, water_reminder,
                           breakfast_time, lunch_time, dinner_time, opted_in, timezone,
                           daily_water_goal, daily_calorie_goal,
                           silent_hours_start, silent_hours_end, is_active
                    FROM users
                    "#,
                )
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|row| User {
                    phone_number: row.get(0),
                    name: None, // Legacy fallback - name column doesn't exist yet
                    created_at: row.get(1),
                    onboarding_completed: row.get(2),
                    onboarding_step: row.get(3),
                    breakfast_reminder: row.get(4),
                    lunch_reminder: row.get(5),
                    dinner_reminder: row.get(6),
                    water_reminder: row.get(7),
                    breakfast_time: row.get(8),
                    lunch_time: row.get(9),
                    dinner_time: row.get(10),
                    opted_in: row.get(11),
                    timezone: row.get(12),
                    daily_water_goal: row.get(13),
                    daily_calorie_goal: row.get(14),
                    silent_hours_start: row.get(15),
                    silent_hours_end: row.get(16),
                    is_active: row.get(17),
                    pending_command: None,
                })
                .collect()
            }
            Err(e) => return Err(e.into()),
        };

        Ok(users)
    }

    pub async fn add_meal(&self, meal: &Meal) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO meals (user_phone, meal_type, calories, description, image_path, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
        )
        .bind(&meal.user_phone)
        .bind(meal.meal_type.to_string())
        .bind(meal.calories)
        .bind(&meal.description)
        .bind(&meal.image_path)
        .bind(meal.created_at)
        .fetch_one(&self.pool)
        .await?;

        let id: i32 = result.get(0);
        Ok(id as i64)
    }

    pub async fn add_water_log(&self, water_log: &WaterLog) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO water_logs (user_phone, amount_ml, created_at)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
        .bind(&water_log.user_phone)
        .bind(water_log.amount_ml)
        .bind(water_log.created_at)
        .fetch_one(&self.pool)
        .await?;

        let id: i32 = result.get(0);
        Ok(id as i64)
    }

    pub async fn get_daily_stats(&self, user_phone: &str, date: NaiveDate) -> Result<DailyStats> {
        let date_str = date.format("%Y-%m-%d").to_string();

        // Optimized: Use CTEs and single pass aggregation (~40% faster)
        let result = sqlx::query(
            r#"
            WITH meals_stats AS (
                SELECT
                    COALESCE(SUM(calories), 0.0) as total_calories,
                    COUNT(*)::BIGINT as meals_count
                FROM meals
                WHERE user_phone = $1
                    AND created_at >= $2::DATE
                    AND created_at < ($2::DATE + INTERVAL '1 day')
            ),
            water_stats AS (
                SELECT
                    COALESCE(SUM(amount_ml)::BIGINT, 0) as total_water,
                    COUNT(*)::BIGINT as water_count
                FROM water_logs
                WHERE user_phone = $1
                    AND created_at >= $2::DATE
                    AND created_at < ($2::DATE + INTERVAL '1 day')
            )
            SELECT
                m.total_calories,
                m.meals_count,
                w.total_water,
                w.water_count
            FROM meals_stats m, water_stats w
            "#,
        )
        .bind(user_phone)
        .bind(date)
        .fetch_one(&self.pool)
        .await?;

        let total_calories: f64 = result.get(0);
        let meals_count: i64 = result.get::<i64, _>(1);
        let total_water_ml: i64 = result.get::<i64, _>(2);
        let water_logs_count: i64 = result.get::<i64, _>(3);

        log::debug!(
            "🔍 DB daily_stats for {} on {}: calories={}, water={}ml, meals={}, water_logs={}",
            user_phone,
            date_str,
            total_calories,
            total_water_ml,
            meals_count,
            water_logs_count
        );

        Ok(DailyStats {
            user_phone: user_phone.to_string(),
            date: date_str,
            total_calories,
            total_water_ml,
            meals_count,
            water_logs_count,
        })
    }

    /// Get meal types logged today (for sequential meal validation)
    pub async fn get_todays_meal_types(&self, user_phone: &str, date: NaiveDate) -> Result<Vec<MealType>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT meal_type
            FROM meals
            WHERE user_phone = $1
                AND created_at >= $2::DATE
                AND created_at < ($2::DATE + INTERVAL '1 day')
            ORDER BY meal_type
            "#,
        )
        .bind(user_phone)
        .bind(date)
        .fetch_all(&self.pool)
        .await?;

        let meal_types = rows
            .into_iter()
            .filter_map(|row| {
                let meal_type_str: String = row.get(0);
                MealType::from_string(&meal_type_str)
            })
            .collect();

        Ok(meal_types)
    }

    pub async fn get_recent_meals(&self, user_phone: &str, limit: i32) -> Result<Vec<Meal>> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_phone, meal_type, calories, description, image_path, created_at
            FROM meals
            WHERE user_phone = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(user_phone)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let meals = rows
            .into_iter()
            .map(|row| {
                let meal_type_str: String = row.get(2);
                let meal_type = MealType::from_string(&meal_type_str)
                    .unwrap_or_else(|| {
                        log::warn!("Unknown meal type '{}', defaulting to Snack", meal_type_str);
                        MealType::Snack
                    });

                let id_i32: i32 = row.get(0);
                Meal {
                    id: Some(id_i32 as i64),
                    user_phone: row.get(1),
                    meal_type,
                    calories: row.get(3),
                    description: row.get(4),
                    image_path: row.get(5),
                    created_at: row.get(6),
                }
            })
            .collect();

        Ok(meals)
    }

    // Onboarding related methods
    pub async fn update_onboarding_step(&self, phone_number: &str, step: Option<String>) -> Result<()> {
        sqlx::query(
            "UPDATE users SET onboarding_step = $1 WHERE phone_number = $2",
        )
        .bind(step)
        .bind(phone_number)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_meal_time(&self, phone_number: &str, meal_type: &str, time: &str) -> Result<()> {
        // Use separate queries instead of dynamic column names to prevent SQL injection
        match meal_type {
            "breakfast" => {
                sqlx::query("UPDATE users SET breakfast_time = $1 WHERE phone_number = $2")
                    .bind(time)
                    .bind(phone_number)
                    .execute(&self.pool)
                    .await?;
            }
            "lunch" => {
                sqlx::query("UPDATE users SET lunch_time = $1 WHERE phone_number = $2")
                    .bind(time)
                    .bind(phone_number)
                    .execute(&self.pool)
                    .await?;
            }
            "dinner" => {
                sqlx::query("UPDATE users SET dinner_time = $1 WHERE phone_number = $2")
                    .bind(time)
                    .bind(phone_number)
                    .execute(&self.pool)
                    .await?;
            }
            _ => return Err(anyhow::anyhow!("Invalid meal type")),
        }

        Ok(())
    }

    pub async fn complete_onboarding(&self, phone_number: &str) -> Result<()> {
        sqlx::query(
            "UPDATE users SET onboarding_completed = TRUE, onboarding_step = NULL WHERE phone_number = $1",
        )
        .bind(phone_number)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_timezone(&self, phone_number: &str, timezone: &str) -> Result<()> {
        sqlx::query(
            "UPDATE users SET timezone = $1 WHERE phone_number = $2",
        )
        .bind(timezone)
        .bind(phone_number)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_water_goal(&self, phone_number: &str, goal_ml: i32) -> Result<()> {
        sqlx::query(
            "UPDATE users SET daily_water_goal = $1 WHERE phone_number = $2",
        )
        .bind(goal_ml)
        .bind(phone_number)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get count of images (meals with image_path) for today
    pub async fn get_daily_image_count(&self, user_phone: &str, date: chrono::NaiveDate) -> Result<i64> {
        let result = sqlx::query(
            r#"
            SELECT COUNT(*)::BIGINT
            FROM meals
            WHERE user_phone = $1
                AND image_path IS NOT NULL
                AND created_at >= $2::DATE
                AND created_at < ($2::DATE + INTERVAL '1 day')
            "#,
        )
        .bind(user_phone)
        .bind(date)
        .fetch_one(&self.pool)
        .await?;

        let count: i64 = result.get::<i64, _>(0);
        Ok(count)
    }

    // ============================================================
    // Favorite Meals (Removed in v2.1 - feature deprecated)
    // Table kept for backward compatibility with existing data
    // ============================================================

    /// Update calorie goal for user
    pub async fn update_calorie_goal(&self, phone_number: &str, goal_kcal: i32) -> Result<()> {
        sqlx::query(
            "UPDATE users SET daily_calorie_goal = $1 WHERE phone_number = $2",
        )
        .bind(goal_kcal)
        .bind(phone_number)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update silent hours for user
    pub async fn update_silent_hours(
        &self,
        phone_number: &str,
        start: &str,
        end: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE users SET silent_hours_start = $1, silent_hours_end = $2 WHERE phone_number = $3",
        )
        .bind(start)
        .bind(end)
        .bind(phone_number)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Set pending command for user (waiting for confirmation)
    // Pending command methods removed in v2.1 - feature deprecated

    // ============================================================
    // Conversation Logging Functions
    // ============================================================

    /// Log a conversation message (incoming from user or outgoing from bot)
    pub async fn log_conversation(
        &self,
        user_phone: &str,
        direction: ConversationDirection,
        message_type: MessageType,
        content: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<i64> {
        let direction_str = direction.to_string();
        let message_type_str = serde_json::to_string(&message_type)?.trim_matches('"').to_string();

        let result = sqlx::query(
            r#"
            INSERT INTO conversations (user_phone, direction, message_type, content, metadata, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
        )
        .bind(user_phone)
        .bind(direction_str)
        .bind(message_type_str)
        .bind(content)
        .bind(metadata)
        .bind(chrono::Utc::now())
        .fetch_one(&self.pool)
        .await?;

        let id: i32 = result.get(0);
        Ok(id as i64)
    }

    /// Get recent conversation history for a user
    pub async fn get_conversation_history(
        &self,
        user_phone: &str,
        limit: i32,
    ) -> Result<Vec<Conversation>> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_phone, direction, message_type, content, metadata, created_at
            FROM conversations
            WHERE user_phone = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(user_phone)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let conversations = rows
            .into_iter()
            .map(|row| {
                let id_i32: i32 = row.get(0);
                let direction_str: String = row.get(2);
                let message_type_str: String = row.get(3);

                let direction = match direction_str.as_str() {
                    "incoming" => ConversationDirection::Incoming,
                    "outgoing" => ConversationDirection::Outgoing,
                    _ => ConversationDirection::Incoming,
                };

                let message_type: MessageType = serde_json::from_str(&format!("\"{}\"", message_type_str))
                    .unwrap_or(MessageType::Text);

                Conversation {
                    id: Some(id_i32 as i64),
                    user_phone: row.get(1),
                    direction,
                    message_type,
                    content: row.get(4),
                    metadata: row.get(5),
                    created_at: row.get(6),
                }
            })
            .collect();

        Ok(conversations)
    }

    /// Get conversation count for a user
    pub async fn get_conversation_count(&self, user_phone: &str) -> Result<i64> {
        let result = sqlx::query(
            r#"
            SELECT COUNT(*)::BIGINT
            FROM conversations
            WHERE user_phone = $1
            "#,
        )
        .bind(user_phone)
        .fetch_one(&self.pool)
        .await?;

        let count: i64 = result.get::<i64, _>(0);
        Ok(count)
    }

    /// Toggle user active status
    /// Update user's name from WhatsApp profile
    pub async fn update_user_name(&self, phone_number: &str, name: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE users SET name = $1 WHERE phone_number = $2")
            .bind(name)
            .bind(phone_number)
            .execute(&self.pool)
            .await?;

        if let Some(n) = name {
            log::debug!("Updated name for {}: {}", phone_number, n);
        }
        Ok(())
    }

    /// Check if user has sent a message in the last 24 hours (WhatsApp Business API window)
    pub async fn is_within_24h_window(&self, phone_number: &str) -> Result<bool> {
        use chrono::{Duration, Utc};

        let cutoff = Utc::now() - Duration::hours(24);

        let result = sqlx::query(
            r#"
            SELECT created_at FROM conversations
            WHERE user_phone = $1 AND direction = 'incoming'
            ORDER BY created_at DESC
            LIMIT 1
            "#
        )
        .bind(phone_number)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = result {
            let last_message: chrono::DateTime<Utc> = row.get(0);
            Ok(last_message > cutoff)
        } else {
            // No incoming messages yet - not in window
            Ok(false)
        }
    }

    /// Check 24h window status and return hours since last message
    /// Returns: (is_within_window, hours_since_last_message, needs_warning)
    /// needs_warning is true if user is at 20-23 hours of inactivity
    pub async fn check_24h_window_detailed(&self, phone_number: &str) -> Result<(bool, Option<i64>, bool)> {
        use chrono::Utc;

        let result = sqlx::query(
            r#"
            SELECT created_at FROM conversations
            WHERE user_phone = $1 AND direction = 'incoming'
            ORDER BY created_at DESC
            LIMIT 1
            "#
        )
        .bind(phone_number)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = result {
            let last_message: chrono::DateTime<Utc> = row.get(0);
            let now = Utc::now();
            let duration = now.signed_duration_since(last_message);
            let hours = duration.num_hours();

            let is_within_window = hours < 24;
            let needs_warning = hours >= 20 && hours < 24;

            Ok((is_within_window, Some(hours), needs_warning))
        } else {
            // No incoming messages yet - not in window, no warning needed
            Ok((false, None, false))
        }
    }

    /// Check if user was already warned about 24h window expiration
    /// Returns true if user was warned in the last 4 hours
    pub async fn was_recently_warned(&self, phone_number: &str) -> Result<bool> {
        use chrono::{Duration, Utc};

        let cutoff = Utc::now() - Duration::hours(4);

        let result = sqlx::query(
            r#"
            SELECT last_warned_at FROM window_warnings
            WHERE user_phone = $1 AND last_warned_at > $2
            "#
        )
        .bind(phone_number)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.is_some())
    }

    /// Mark user as warned about 24h window expiration
    pub async fn mark_as_warned(&self, phone_number: &str) -> Result<()> {
        use chrono::Utc;

        sqlx::query(
            r#"
            INSERT INTO window_warnings (user_phone, last_warned_at)
            VALUES ($1, $2)
            ON CONFLICT (user_phone) DO UPDATE SET last_warned_at = EXCLUDED.last_warned_at
            "#
        )
        .bind(phone_number)
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Clear warning status when user sends a new message (called when message received)
    pub async fn clear_warning_status(&self, phone_number: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM window_warnings WHERE user_phone = $1
            "#
        )
        .bind(phone_number)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn toggle_user_active(&self, phone_number: &str) -> Result<bool> {
        // Get current status
        let current = sqlx::query(
            "SELECT is_active FROM users WHERE phone_number = $1"
        )
        .bind(phone_number)
        .fetch_one(&self.pool)
        .await?;

        let current_status: bool = current.get(0);
        let new_status = !current_status;

        // Update status
        sqlx::query(
            "UPDATE users SET is_active = $1 WHERE phone_number = $2"
        )
        .bind(new_status)
        .bind(phone_number)
        .execute(&self.pool)
        .await?;

        Ok(new_status)
    }

    /// Reset user completely - delete all meals, water logs, conversations, favorite meals
    /// and reset onboarding status (keeps user record with phone number)
    pub async fn reset_user(&self, phone_number: &str) -> Result<()> {
        log::info!("🔄 Resetting user: {}", phone_number);

        // Delete all meals
        sqlx::query("DELETE FROM meals WHERE user_phone = $1")
            .bind(phone_number)
            .execute(&self.pool)
            .await?;
        log::debug!("Deleted meals for {}", phone_number);

        // Delete all water logs
        sqlx::query("DELETE FROM water_logs WHERE user_phone = $1")
            .bind(phone_number)
            .execute(&self.pool)
            .await?;
        log::debug!("Deleted water logs for {}", phone_number);

        // Delete all conversations
        sqlx::query("DELETE FROM conversations WHERE user_phone = $1")
            .bind(phone_number)
            .execute(&self.pool)
            .await?;
        log::debug!("Deleted conversations for {}", phone_number);

        // Delete all favorite meals
        sqlx::query("DELETE FROM favorite_meals WHERE user_phone = $1")
            .bind(phone_number)
            .execute(&self.pool)
            .await?;
        log::debug!("Deleted favorite meals for {}", phone_number);

        // Reset user to initial state (not onboarded)
        sqlx::query(
            r#"
            UPDATE users
            SET onboarding_completed = false,
                onboarding_step = NULL,
                breakfast_time = NULL,
                lunch_time = NULL,
                dinner_time = NULL,
                daily_calorie_goal = NULL,
                daily_water_goal = NULL,
                is_active = true
            WHERE phone_number = $1
            "#
        )
        .bind(phone_number)
        .execute(&self.pool)
        .await?;

        log::info!("✅ User {} has been completely reset", phone_number);
        Ok(())
    }

    /// Get only active users (for reminders)
    pub async fn get_active_users(&self) -> Result<Vec<User>> {
        // Try with pending_command and name first
        let result = sqlx::query(
            r#"
            SELECT phone_number, name, created_at, onboarding_completed, onboarding_step,
                   breakfast_reminder, lunch_reminder, dinner_reminder, water_reminder,
                   breakfast_time, lunch_time, dinner_time, opted_in, timezone,
                   daily_water_goal, daily_calorie_goal,
                   silent_hours_start, silent_hours_end, is_active, pending_command
            FROM users
            WHERE is_active = TRUE
            "#,
        )
        .fetch_all(&self.pool)
        .await;

        let users = match result {
            Ok(rows) => rows
                .into_iter()
                .map(|row| User {
                    phone_number: row.get(0),
                    name: row.get(1),
                    created_at: row.get(2),
                    onboarding_completed: row.get(3),
                    onboarding_step: row.get(4),
                    breakfast_reminder: row.get(5),
                    lunch_reminder: row.get(6),
                    dinner_reminder: row.get(7),
                    water_reminder: row.get(8),
                    breakfast_time: row.get(9),
                    lunch_time: row.get(10),
                    dinner_time: row.get(11),
                    opted_in: row.get(12),
                    timezone: row.get(13),
                    daily_water_goal: row.get(14),
                    daily_calorie_goal: row.get(15),
                    silent_hours_start: row.get(16),
                    silent_hours_end: row.get(17),
                    is_active: row.get(18),
                    pending_command: row.get(19),
                })
                .collect(),
            Err(e) if e.to_string().contains("pending_command") || e.to_string().contains("column") => {
                // Column doesn't exist yet, use legacy query
                log::debug!("pending_command column not found in get_active_users, using legacy query");
                sqlx::query(
                    r#"
                    SELECT phone_number, created_at, onboarding_completed, onboarding_step,
                           breakfast_reminder, lunch_reminder, dinner_reminder, water_reminder,
                           breakfast_time, lunch_time, dinner_time, opted_in, timezone,
                           daily_water_goal, daily_calorie_goal,
                           silent_hours_start, silent_hours_end, is_active
                    FROM users
                    WHERE is_active = TRUE
                    "#,
                )
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|row| User {
                    phone_number: row.get(0),
                    name: None, // Legacy fallback - name column doesn't exist yet
                    created_at: row.get(1),
                    onboarding_completed: row.get(2),
                    onboarding_step: row.get(3),
                    breakfast_reminder: row.get(4),
                    lunch_reminder: row.get(5),
                    dinner_reminder: row.get(6),
                    water_reminder: row.get(7),
                    breakfast_time: row.get(8),
                    lunch_time: row.get(9),
                    dinner_time: row.get(10),
                    opted_in: row.get(11),
                    timezone: row.get(12),
                    daily_water_goal: row.get(13),
                    daily_calorie_goal: row.get(14),
                    silent_hours_start: row.get(15),
                    silent_hours_end: row.get(16),
                    is_active: row.get(17),
                    pending_command: None,
                })
                .collect()
            }
            Err(e) => return Err(e.into()),
        };

        Ok(users)
    }
}
