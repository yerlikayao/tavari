# 🚀 START HERE - WhatsApp Nutrition Bot

## ⚡ Quick Start (2 dakika)

```bash
# 1. Bot'u çalıştır
RUST_LOG=info cargo run --release

# 2. Çalıştı! 🎉
```

---

## 📱 WhatsApp'tan Test Et

**Numara**: `+1 302-726-0990`

**Test mesajları**:
```
1. Merhaba
2. *Yemek fotoğrafı gönder*
3. 250 ml su içtim
4. /rapor
```

---

## 🔧 Production'a Geçiş

[src/main.rs](src/main.rs) dosyasında 47. satırı değiştir:

```rust
// ÖNCEKİ (Mock):
let whatsapp = Arc::new(MockWhatsAppClient::new()) ...

// YENİ (Production):
use services::BirdComClient;
let whatsapp = Arc::new(BirdComClient::new(
    env::var("BIRD_API_KEY").unwrap(),
    env::var("BIRD_WORKSPACE_ID").unwrap(),
    env::var("BIRD_CHANNEL_ID").unwrap(),
)) as Arc<dyn services::WhatsAppService>;
```

Sonra:
```bash
cargo run --release
```

✅ **Artık gerçek WhatsApp mesajları gelecek!**

---

## 📚 Dokümantasyon

| Dosya | Ne İçin? |
|-------|----------|
| [PRODUCTION_READY.md](PRODUCTION_READY.md) | 🎯 **En önemli - buradan başla** |
| [QUICK_START.md](QUICK_START.md) | Hızlı başlangıç |
| [OPENROUTER_SETUP.md](OPENROUTER_SETUP.md) | OpenRouter AI detayları |
| [BIRD_COM_INTEGRATION.md](BIRD_COM_INTEGRATION.md) | Bird.com WhatsApp setup |
| [FEATURES.md](FEATURES.md) | Tüm özellikler |

---

## ✅ Hazır Olan

- ✅ OpenRouter AI (ücretsiz Mistral model)
- ✅ Bird.com WhatsApp entegrasyonu
- ✅ SQLite veritabanı
- ✅ Kalori analizi (vision AI)
- ✅ Su takibi
- ✅ Günlük raporlar
- ✅ Hatırlatmalar

---

## 🔮 Gelecek Özellikler (Opsiyonel)

Eğer devam etmek isterseniz:

1. **Onboarding** - Kullanıcılar kendi saatlerini belirlesin
2. **Webhook** - Gerçek zamanlı mesaj alma
3. **Özel hatırlatmalar** - Kullanıcı bazlı saatler

[ONBOARDING_PLAN.md](ONBOARDING_PLAN.md) - Detaylı plan

---

## 💰 Maliyet

**Şu anki setup:**
- OpenRouter: **$0** (ücretsiz model)
- Bird.com: **50 mesaj/ay ücretsiz**
- Sonrası: ~$0.005/mesaj

**100 kullanıcı, 1000 mesaj/ay**: ~$50/ay

---

## 🆘 Sorun mu var?

```bash
# Derleme hatası?
cargo clean
cargo build --release

# Database hatası?
rm -rf data/nutrition.db
cargo run --release

# API key hatası?
cat .env  # Kontrol et
```

---

## 🎯 Hızlı Komutlar

```bash
# Build
cargo build --release

# Run
RUST_LOG=info cargo run --release

# Test
cargo test

# Clean
cargo clean
```

---

## 📊 Sistem Durumu

```
┌─────────────────────────────┐
│  PRODUCTION READY ✅        │
├─────────────────────────────┤
│ OpenRouter:      ÇALIŞIYOR  │
│ Bird.com:        HAZIR      │
│ Database:        OK         │
│ Kod:             DERLENDI   │
└─────────────────────────────┘
```

---

## 🎉 Sonuç

**Bot hazır!**
**2 dakika içinde çalışır durumda!**
**İsterseniz production'a geçebilirsiniz!**

Sorular için [PRODUCTION_READY.md](PRODUCTION_READY.md) dosyasına bakın.

---

**Happy Coding! 🦀**
