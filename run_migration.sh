#!/bin/bash
# Migration script for adding pending_command column
# Run this on the server after deploying the new code

set -e

echo "🔄 Running database migration: add_pending_command"

# Load environment variables
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
fi

# Check if DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
    echo "❌ ERROR: DATABASE_URL is not set"
    exit 1
fi

echo "📊 Database: $DATABASE_URL"

# Run the migration using psql
psql "$DATABASE_URL" -f migrations/add_pending_command.sql

echo "✅ Migration completed successfully!"
echo ""
echo "📝 Note: The pending_command column has been added to the users table."
echo "   This enables AI-powered command suggestions with user confirmation."
