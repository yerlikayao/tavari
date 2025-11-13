# 🚀 Natural Language Feature - Deployment Guide

## Overview
This deployment adds AI-powered natural language processing that allows users to interact with the bot in a more conversational way, without needing to remember specific commands.

## What's New
- ✨ AI detects user intent from natural language messages
- 🍽️ Users can log meals by simply saying "kahvaltı yaptım" or "pizza yedim"
- 💧 Users can log water by saying "su içtim" or "250 ml içtim"
- 🔄 Removed rigid "ogun [description]" command requirement
- 📱 More conversational, user-friendly interface
- 🛡️ Graceful fallback if AI fails

## Breaking Changes
- **Removed**: `ogun [description]` command (users can now just say "pizza" or "tavuk göğsü")
- **Simplified**: Water logging no longer requires "su içtim" format (but still works)
- **Enhanced**: All messages go through AI intent detection if not recognized as explicit commands

## Deployment Steps

### 1. Pull Latest Code
```bash
git pull origin main
```

### 2. No Database Migration Required
This feature doesn't require any database schema changes - it works with the existing database structure.

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

## New User Experience

### Before (Old Way)
```
User: "ogun pizza"
Bot: ✅ Ara Öğün Kaydedildi! ...

User: "su içtim"
Bot: 💧 200 ml kaydedildi!
```

### After (Natural Language)
```
User: "pizza yedim"
Bot: ✅ Ara Öğün Kaydedildi! ...

User: "su içtim"
Bot: 💧 200 ml kaydedildi!

User: "kahvaltı yaptım"
Bot: ✅ Kahvaltı Kaydedildi! ...

User: "tavuk göğsü ve salata"
Bot: ✅ Öğle Yemeği Kaydedildi! ...
```

## Testing

### Test Case 1: Natural Meal Logging
```
User: "kahvaltı yaptım"
Bot: [AI analyzes meal and logs it]
```

### Test Case 2: Natural Water Logging
```
User: "250 ml içtim"
Bot: 💧 250 ml kaydedildi!
     Bugün: 250 ml / 2000 ml
     Kalan: 1750 ml
```

### Test Case 3: Meal Without Command Word
```
User: "biftek"
Bot: [AI detects meal intent and logs it]
```

### Test Case 4: Commands Still Work
```
User: "rapor"
Bot: [Shows daily report - unchanged]
```

### Test Case 5: Unknown Message
```
User: "merhaba nasılsın"
Bot: [Shows help message]
```

## How It Works

### AI Intent Detection Flow
1. User sends message: "kahvaltı yaptım"
2. System checks if it's a known command (rapor, ayarlar, etc.) → No
3. AI analyzes message and returns: `MEAL:kahvaltı`
4. System calls `handle_text_meal()` with "kahvaltı"
5. AI analyzes meal and returns calories/description
6. Meal is logged to database

### Intent Types
The AI can detect 4 types of intents:

1. **MEAL:[description]** - User wants to log food
   - "kahvaltı yaptım" → MEAL:kahvaltı
   - "pizza" → MEAL:pizza
   - "tavuk göğsü" → MEAL:tavuk göğsü

2. **WATER:[amount_ml]** - User wants to log water
   - "su içtim" → WATER:200
   - "250 ml" → WATER:250
   - "1 bardak su" → WATER:250

3. **COMMAND:[command]** - User wants to run a command
   - "rapor" → COMMAND:rapor
   - "ayarlar" → COMMAND:ayarlar

4. **UNKNOWN** - Unclear intent
   - "merhaba" → UNKNOWN (shows help)

## Performance Notes

- AI intent detection: ~0.5s per message (only for non-commands)
- Max tokens: 50 (very fast)
- Meal analysis: ~1s (unchanged)
- Graceful fallback: If AI fails, shows help message
- No impact on existing command processing

## Cost Considerations

- AI calls: Every message that's not a known command
- Model: `meta-llama/llama-4-scout:free` (free tier)
- Intent detection: 50 tokens per call (minimal cost)
- Meal analysis: 300 tokens per call (existing feature)
- Estimated additional cost per message: ~$0 (using free model)

## Rollback (If Needed)

If you need to rollback to the command-based system:

```bash
git revert HEAD
docker-compose build
docker-compose up -d
```

Note: No database changes to revert.

## Monitoring

Watch for these log messages:
- `🧠 Using AI to detect user intent` - Intent detection triggered
- `🍽️ User wants to log meal` - Meal intent detected
- `💧 User wants to log water` - Water intent detected
- `⚙️ User wants to run command` - Command intent detected
- `❓ AI couldn't determine intent` - Unknown message (shows help)
- `⚠️ AI intent detection failed` - AI error (shows help)

## Updated Help Message

The help message has been updated to reflect natural language usage:

```
📱 Beslenme Takip Botu

🍽️ Yemek Kaydet (Doğal Dil)
Sadece yaz:
• "kahvaltı yaptım"
• "pizza yedim"
• "tavuk göğsü ve salata"
• Fotoğraf gönder

💧 Su Kaydet (Doğal Dil)
Sadece yaz:
• "su içtim"
• "250 ml içtim"
• "1 bardak su"
• 1, 2, 3 (200/250/500ml)

📊 Ana Komutlar
rapor - Günlük özet
geçmiş - Son 5 öğün
tavsiye - AI beslenme önerisi
ayarlar - Tüm ayarlar

💡 İpucu: Normal konuşarak mesaj at!
```

## Code Changes Summary

### New Files
None - all changes to existing files

### Modified Files
1. **src/services/openrouter.rs**
   - Added `UserIntent` enum
   - Added `detect_user_intent()` method

2. **src/services/mod.rs**
   - Exported `UserIntent`

3. **src/handlers/message_handler.rs**
   - Replaced command suggestion flow with intent detection
   - Removed `ogun` command handler
   - Removed `handle_water_log()` and `parse_water_amount()` (replaced with direct amount)
   - Updated help message
   - Added natural language processing flow

## Success Criteria

✅ Application starts successfully
✅ Natural language meal logging works ("kahvaltı yaptım")
✅ Natural language water logging works ("su içtim")
✅ Existing commands still work (rapor, ayarlar, etc.)
✅ Images still work as before
✅ AI failures don't break the system (graceful fallback)
✅ Help message shows new natural language examples

## Support

If you encounter issues:
1. Check logs: `docker-compose logs -f app`
2. Verify AI service: Check OpenRouter dashboard
3. Test with explicit commands first (rapor, ayarlar)
4. Fallback: System shows help message for unrecognized messages

## FAQ

**Q: What if AI is down?**
A: The system gracefully falls back to showing the help message. Explicit commands (rapor, ayarlar, etc.) still work normally.

**Q: Can users still use "ogun [description]"?**
A: No, this command has been removed. Users should just type the meal description directly (e.g., "pizza" instead of "ogun pizza").

**Q: Does this use more AI credits?**
A: Yes, but minimally. Intent detection uses only 50 tokens per message and uses the free model. The existing meal analysis (300 tokens) remains unchanged.

**Q: What languages are supported?**
A: Currently Turkish, but the AI prompt can be easily adapted for other languages.

**Q: Can I disable natural language and go back to commands?**
A: Yes, see the Rollback section above.
