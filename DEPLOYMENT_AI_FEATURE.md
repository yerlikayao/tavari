# 🚀 AI Command Suggestion Feature - Deployment Guide

## Overview
This deployment adds an AI-powered command suggestion system that helps users when they make typos or use invalid commands.

## What's New
- ✨ AI analyzes invalid commands and suggests corrections
- 🔄 User confirmation with "1" (Yes) / "0" (No) buttons
- 🛡️ Graceful fallback if AI fails
- 📊 Enhanced logging for debugging

## Deployment Steps

### 1. Pull Latest Code
```bash
git pull origin main
```

### 2. Run Database Migration
```bash
# Make sure you're in the project directory
cd /path/to/tavari

# Run the migration script
./run_migration.sh
```

Or manually:
```bash
psql "$DATABASE_URL" -f migrations/add_pending_command.sql
```

### 3. Rebuild and Restart
```bash
# If using Docker Compose / Dokploy
docker-compose down
docker-compose build
docker-compose up -d

# Or if running directly
cargo build --release
# Restart your systemd service or process manager
```

### 4. Verify Deployment
Check logs for successful startup:
```bash
docker-compose logs -f app
# or
journalctl -u tavari -f
```

## Testing

### Test Case 1: Typo in Command
```
User: "rapr"
Bot: 🤔 'rapor' komutunu mu demek istedin?
     1 - Evet
     0 - Hayır

User: "1"
Bot: [Shows daily report]
```

### Test Case 2: Similar Command
```
User: "ayrlr"
Bot: 🤔 'ayarlar' komutunu mu demek istedin?
     1 - Evet
     0 - Hayır

User: "1"
Bot: [Shows settings]
```

### Test Case 3: Normal Conversation
```
User: "merhaba"
Bot: [Shows help message - AI recognizes it's not a command]
```

### Test Case 4: Rejection
```
User: "tvsiye"
Bot: 🤔 'tavsiye' komutunu mu demek istedin?
     1 - Evet
     0 - Hayır

User: "0"
Bot: Tamam, iptal edildi.
```

## Rollback (If Needed)

If you need to rollback:

1. Revert the database migration:
```sql
ALTER TABLE users DROP COLUMN IF EXISTS pending_command;
```

2. Revert to previous commit:
```bash
git revert HEAD
docker-compose build
docker-compose up -d
```

## Monitoring

Watch for these log messages:
- `🤔 Invalid command received` - AI suggestion triggered
- `💡 AI suggested command` - AI found a match
- `✅ User X confirmed command` - User accepted suggestion
- `❌ User X rejected command` - User declined suggestion
- `⚠️ AI command suggestion failed` - AI error (graceful fallback)

## Performance Notes

- AI suggestions only trigger for invalid commands
- Max tokens: 50 (very fast, ~0.5s response)
- Graceful fallback: If AI fails, shows help message
- No impact on valid command processing

## Database Schema Change

```sql
-- Added column
pending_command TEXT DEFAULT NULL
-- Stores the AI-suggested command waiting for user confirmation
```

## Cost Considerations

- AI calls: Only for invalid commands
- Model: `meta-llama/llama-4-scout:free` (free tier)
- Max tokens per call: 50 (minimal cost)
- Estimated additional cost: ~$0 (using free model)

## Support

If you encounter issues:
1. Check logs: `docker-compose logs -f app`
2. Verify migration: `psql "$DATABASE_URL" -c "\d users"`
3. Test AI service: Check OpenRouter dashboard
4. Fallback: System works without AI (shows help message)

## Success Criteria

✅ Migration completed without errors
✅ Application starts successfully
✅ Typo commands trigger AI suggestions
✅ User can confirm/reject suggestions
✅ Normal commands work as before
✅ AI failures don't break the system
