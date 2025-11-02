# 🚀 Kurulum ve Yapılandırma Rehberi

## 📋 İçindekiler
- [Hızlı Başlangıç](#-hızlı-başlangıç)
- [OpenRouter Kurulumu](#-openrouter-kurulumu)
- [Bird.com WhatsApp Kurulumu](#-birdcom-whatsapp-kurulumu)
- [Webhook Yapılandırması](#-webhook-yapılandırması)
- [Çalıştırma](#-çalıştırma)
- [Test](#-test)

---

## ⚡ Hızlı Başlangıç

### 1. Gereksinimler
```bash
# Rust kurulumu (eğer yoksa)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Projeyi klonla ve çalıştır
cargo run --release
```

### 2. API Anahtarları
```bash
# .env dosyasını düzenle
cp .env.example .env
nano .env

# Gerekli anahtarlar:
OPENROUTER_API_KEY=sk-or-v1-xxxxx
BIRD_API_KEY=xxxxx
BIRD_WORKSPACE_ID=xxxxx
BIRD_CHANNEL_ID=xxxxx
```

### 3. Çalıştır
```bash
RUST_LOG=info cargo run --release
```

---

## 🤖 OpenRouter Kurulumu

### 1. Hesap Oluştur
1. [https://openrouter.ai](https://openrouter.ai) adresine git
2. Sign up / Login yap
3. **Keys** sekmesine tıkla
4. **Create Key** ile yeni key oluştur
5. Key'i kopyala

### 2. Yapılandırma
```bash
# .env dosyasına ekle
OPENROUTER_API_KEY=sk-or-v1-xxxxxxxxxxxxxxxxxxxxxx
OPENROUTER_MODEL=mistralai/mistral-small-3.2-24b-instruct:free
```

### 3. Test
```bash
curl https://openrouter.ai/api/v1/chat/completions \
  -H "Authorization: Bearer $OPENROUTER_API_KEY" \
  -H "Content-Type: application/json" \
  -H "HTTP-Referer: https://github.com/tavari-bot" \
  -d '{"model": "mistralai/mistral-small-3.2-24b-instruct:free", "messages": [{"role": "user", "content": "Merhaba"}]}'
```

---

## 🐦 Bird.com WhatsApp Kurulumu

### 1. Bird.com Hesabı
1. [https://bird.com](https://bird.com) adresine git
2. Hesap oluştur
3. Email doğrulama yap
4. Dashboard'a giriş yap

### 2. WhatsApp Channel Ekle
1. Dashboard → **Channels** → **Add Channel**
2. **WhatsApp** seçin
3. Phone number ekleyin
4. API Key'i kopyalayın

### 3. Credentials Al
Dashboard'da:
- **Workspace ID** (örn: workspace_123)
- **API Key** (örn: sk_live_xxx)
- **Channel ID** (örn: channel_456)

### 4. Yapılandırma
```bash
# .env dosyasına ekle
BIRD_API_KEY=SmPJEH2znLCegFTPLwUwKz73iR4ZZ5hPfcpq
BIRD_WORKSPACE_ID=4387402214821863
BIRD_CHANNEL_ID=cbf5c959-fc42-566c-ade1-5a6b9ae2ae78
```

---

## 🌐 Webhook Yapılandırması

### Seçenek 1: ngrok (Önerilen)

```bash
# ngrok indir ve kur
brew install ngrok

# Hesap oluştur: https://ngrok.com/
ngrok config add-authtoken YOUR_AUTH_TOKEN

# Tunnel aç
ngrok http 8080

# Output: https://abc123.ngrok.io
```

### Seçenek 2: localhost.run

```bash
ssh -R 80:localhost:8080 localhost.run
```

### Bird.com Dashboard'da Webhook Ayarla

1. Dashboard → **Channels** → WhatsApp channel seçin
2. **Webhooks** sekmesi
3. **Webhook URL**: `https://97bdc1f55325.ngrok-free.app/webhook/whatsapp`
4. **Signing Key**: `6e7e922204e830ab7fe42fea3b564c2a25a9534e67684f5e8cb3792bb5d2a7cb`
5. **Events**: `message.created` seçin
6. **Save**

---

## ▶️ Çalıştırma

### Development
```bash
RUST_LOG=info cargo run
```

### Production
```bash
cargo build --release
RUST_LOG=info ./target/release/whatsapp-nutrition-bot
```

### Output
```
🚀 Starting WhatsApp Nutrition Bot...
✅ Database initialized
✅ OpenRouter service initialized with model: mistralai/mistral-small-3.2-24b-instruct:free
✅ WhatsApp service initialized (Bird.com Production)
✅ Message handler initialized
✅ Reminder service started
🌐 Webhook server starting on 0.0.0.0:8080
✅ Webhook server started
🎉 Bot is ready!

📱 Bot çalışıyor!
📞 WhatsApp Numarası: +1 302-726-0990
🌐 Webhook Server: http://localhost:8080
⏰ Hatırlatma servisi aktif
```

---

## 🧪 Test

### 1. Webhook Test
```bash
curl -X POST http://localhost:8080/webhook/whatsapp \
  -H "Content-Type: application/json" \
  -d '{
    "id": "msg_123",
    "type": "message.created",
    "contact": {
      "identifierValue": "+905551234567"
    },
    "message": {
      "type": "text",
      "text": {
        "text": "Merhaba"
      }
    }
  }'
```

### 2. WhatsApp Test
WhatsApp'tan `+1 302-726-0990` numarasına mesaj gönder:
- `Merhaba` - Onboarding başlar
- Yemek fotoğrafı - Kalori analizi
- `250 ml su içtim` - Su kaydı
- `/rapor` - Günlük rapor

### 3. API Test
```bash
# OpenRouter bağlantısı
curl -H "Authorization: Bearer $OPENROUTER_API_KEY" \
     https://openrouter.ai/api/v1/models

# Bird.com bağlantısı (dashboard'dan test edebilirsiniz)
```

---

## 🔧 Sorun Giderme

### "Webhook mesajları gelmiyor"
- ngrok tunnel çalışıyor mu?
- Bird.com dashboard'da webhook URL doğru mu?
- Logs'da webhook çağrısı görünüyor mu?

### "OpenRouter API hatası"
```bash
# API key kontrolü
echo $OPENROUTER_API_KEY

# Model testi
curl https://openrouter.ai/api/v1/chat/completions \
  -H "Authorization: Bearer $OPENROUTER_API_KEY" \
  -d '{"model": "mistralai/mistral-small-3.2-24b-instruct:free", "messages": [{"role": "user", "content": "test"}]}'
```

### "Bird.com bağlantı hatası"
- API key geçerli mi?
- Workspace ID ve Channel ID doğru mu?
- Bird.com dashboard'da channel aktif mi?

### Database Hatası
```bash
# Eski database'i sil
rm -f data/nutrition.db

# Tekrar çalıştır
cargo run --release
```

---

## 📊 Sistem Durumu

Bot çalışırken bu logları görmelisiniz:
```
✅ Database initialized
✅ OpenRouter service initialized
✅ WhatsApp service initialized (Bird.com Production)
✅ Message handler initialized
✅ Reminder service started
✅ Webhook server started
🎉 Bot is ready!
```

---

## 🎯 Sonraki Adımlar

1. ✅ Kurulum tamamlandı
2. ⏳ Webhook URL'yi Bird.com'a kaydet
3. ⏳ WhatsApp'tan test mesajı gönder
4. ⏳ Onboarding'i test et
5. ⏳ Kalori analizi test et

**🎉 Hazır! Bot çalışıyor ve mesaj bekliyor!**
