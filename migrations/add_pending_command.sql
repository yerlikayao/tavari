-- Migration: Add pending_command column for AI command suggestions
-- Date: 2025-01-13

-- Add pending_command column if not exists
ALTER TABLE users
ADD COLUMN IF NOT EXISTS pending_command TEXT DEFAULT NULL;

-- Comment explaining the column
COMMENT ON COLUMN users.pending_command IS 'AI suggested command waiting for user confirmation (1=yes, 0=no)';
