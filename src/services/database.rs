use anyhow::Result;
use chrono::NaiveDate;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};

use crate::models::{DailyStats, Meal, MealType, User, WaterLog};

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
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                phone_number TEXT PRIMARY KEY,
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
                water_reminder_interval INTEGER DEFAULT 120,
                daily_water_goal INTEGER DEFAULT 2000
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

        // Migration: Add new columns if they don't exist (for existing deployments)
        // This is safe to run multiple times
        sqlx::query(
            r#"
            DO $$
            BEGIN
                -- Add water_reminder_interval column if not exists
                IF NOT EXISTS (
                    SELECT 1 FROM information_schema.columns
                    WHERE table_name='users' AND column_name='water_reminder_interval'
                ) THEN
                    ALTER TABLE users ADD COLUMN water_reminder_interval INTEGER DEFAULT 120;
                END IF;

                -- Add daily_water_goal column if not exists
                IF NOT EXISTS (
                    SELECT 1 FROM information_schema.columns
                    WHERE table_name='users' AND column_name='daily_water_goal'
                ) THEN
                    ALTER TABLE users ADD COLUMN daily_water_goal INTEGER DEFAULT 2000;
                END IF;
            END $$;
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Update existing users with NULL values to have defaults
        sqlx::query("UPDATE users SET water_reminder_interval = 120 WHERE water_reminder_interval IS NULL")
            .execute(&self.pool)
            .await?;

        sqlx::query("UPDATE users SET daily_water_goal = 2000 WHERE daily_water_goal IS NULL")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn create_user(&self, user: &User) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO users (
                phone_number, created_at, onboarding_completed, onboarding_step,
                breakfast_reminder, lunch_reminder, dinner_reminder, water_reminder,
                breakfast_time, lunch_time, dinner_time, opted_in, timezone,
                water_reminder_interval, daily_water_goal
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            ON CONFLICT (phone_number) DO NOTHING
            "#,
        )
        .bind(&user.phone_number)
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
        .bind(user.water_reminder_interval)
        .bind(user.daily_water_goal)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_user(&self, phone_number: &str) -> Result<Option<User>> {
        let user = sqlx::query(
            r#"
            SELECT phone_number, created_at, onboarding_completed, onboarding_step,
                   breakfast_reminder, lunch_reminder, dinner_reminder, water_reminder,
                   breakfast_time, lunch_time, dinner_time, opted_in, timezone,
                   water_reminder_interval, daily_water_goal
            FROM users WHERE phone_number = $1
            "#,
        )
        .bind(phone_number)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| User {
            phone_number: row.get(0),
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
            water_reminder_interval: row.get(13),
            daily_water_goal: row.get(14),
        });

        Ok(user)
    }

    pub async fn get_all_users(&self) -> Result<Vec<User>> {
        let rows = sqlx::query(
            r#"
            SELECT phone_number, created_at, onboarding_completed, onboarding_step,
                   breakfast_reminder, lunch_reminder, dinner_reminder, water_reminder,
                   breakfast_time, lunch_time, dinner_time, opted_in, timezone,
                   water_reminder_interval, daily_water_goal
            FROM users
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let users = rows
            .into_iter()
            .map(|row| User {
                phone_number: row.get(0),
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
                water_reminder_interval: row.get(13),
                daily_water_goal: row.get(14),
            })
            .collect();

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

        // Use separate subqueries to avoid Cartesian product
        let result = sqlx::query(
            r#"
            SELECT
                COALESCE((
                    SELECT SUM(calories)
                    FROM meals
                    WHERE user_phone = $1
                        AND created_at >= $2::DATE
                        AND created_at < ($2::DATE + INTERVAL '1 day')
                ), 0.0) as total_calories,
                COALESCE((
                    SELECT COUNT(*)
                    FROM meals
                    WHERE user_phone = $1
                        AND created_at >= $2::DATE
                        AND created_at < ($2::DATE + INTERVAL '1 day')
                ), 0) as meals_count,
                COALESCE((
                    SELECT SUM(amount_ml)
                    FROM water_logs
                    WHERE user_phone = $1
                        AND created_at >= $2::DATE
                        AND created_at < ($2::DATE + INTERVAL '1 day')
                ), 0) as total_water,
                COALESCE((
                    SELECT COUNT(*)
                    FROM water_logs
                    WHERE user_phone = $1
                        AND created_at >= $2::DATE
                        AND created_at < ($2::DATE + INTERVAL '1 day')
                ), 0) as water_count
            "#,
        )
        .bind(user_phone)
        .bind(date)
        .fetch_one(&self.pool)
        .await?;

        let total_calories: f64 = result.get(0);
        let meals_count: i64 = result.get(1);
        let total_water_ml: i64 = result.get(2);
        let water_logs_count: i64 = result.get(3);

        Ok(DailyStats {
            user_phone: user_phone.to_string(),
            date: date_str,
            total_calories,
            total_water_ml,
            meals_count,
            water_logs_count,
        })
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
        let column = match meal_type {
            "breakfast" => "breakfast_time",
            "lunch" => "lunch_time",
            "dinner" => "dinner_time",
            _ => return Err(anyhow::anyhow!("Invalid meal type")),
        };

        sqlx::query(&format!("UPDATE users SET {} = $1 WHERE phone_number = $2", column))
            .bind(time)
            .bind(phone_number)
            .execute(&self.pool)
            .await?;

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

    pub async fn update_water_reminder_interval(&self, phone_number: &str, interval_minutes: i32) -> Result<()> {
        sqlx::query(
            "UPDATE users SET water_reminder_interval = $1 WHERE phone_number = $2",
        )
        .bind(interval_minutes)
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
            SELECT COUNT(*)
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

        let count: i64 = result.get(0);
        Ok(count)
    }
}
