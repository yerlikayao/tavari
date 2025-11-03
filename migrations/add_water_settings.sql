-- Migration: Add water reminder interval and daily water goal settings
-- Date: 2025-01-XX

-- Add water_reminder_interval column if not exists
ALTER TABLE users
ADD COLUMN IF NOT EXISTS water_reminder_interval INTEGER DEFAULT 120;

-- Add daily_water_goal column if not exists
ALTER TABLE users
ADD COLUMN IF NOT EXISTS daily_water_goal INTEGER DEFAULT 2000;

-- Update existing users with NULL values to have defaults
UPDATE users
SET water_reminder_interval = 120
WHERE water_reminder_interval IS NULL;

UPDATE users
SET daily_water_goal = 2000
WHERE daily_water_goal IS NULL;
