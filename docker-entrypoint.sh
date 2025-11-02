#!/bin/bash
set -e

# Extract database connection details from DATABASE_URL
# Format: postgresql://user:password@host:port/dbname
if [ -n "$DATABASE_URL" ]; then
    # Extract host and port from DATABASE_URL
    DB_HOST=$(echo $DATABASE_URL | sed -E 's|.*@([^:]+):.*|\1|')
    DB_PORT=$(echo $DATABASE_URL | sed -E 's|.*:([0-9]+)/.*|\1|')

    echo "Waiting for PostgreSQL at $DB_HOST:$DB_PORT..."

    # Wait for PostgreSQL to be ready (max 30 seconds)
    for i in {1..30}; do
        if nc -z "$DB_HOST" "$DB_PORT" 2>/dev/null; then
            echo "PostgreSQL is ready!"
            break
        fi

        if [ $i -eq 30 ]; then
            echo "ERROR: PostgreSQL is not available after 30 seconds"
            echo "Attempting to resolve DNS for $DB_HOST..."
            getent hosts "$DB_HOST" || echo "DNS resolution failed for $DB_HOST"
            exit 1
        fi

        echo "Waiting for PostgreSQL... ($i/30)"
        sleep 1
    done
fi

# Start the application
exec "$@"
