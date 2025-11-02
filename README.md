# 🍽️ WhatsApp Nutrition Bot (Rust)

WhatsApp üzerinden çalışan, AI destekli beslenme ve su takip botu.

**OpenRouter + Bird.com entegrasyonu ile production-ready!**

## ✨ Özellikler

- 📸 **Yemek Fotoğrafı Analizi**: OpenRouter Vision API ile yemek resminden kalori hesaplama
- 💧 **Su Tüketimi Takibi**: Günlük su içme kayıtları
- 👤 **Kişiselleştirilmiş Onboarding**: Kullanıcıların kendi yemek saatlerini belirlemesi
- ⏰ **Akıllı Hatırlatmalar**: Kişisel saatlere göre bildirimler
- 📊 **Günlük Raporlar**: Kalori ve su tüketimi istatistikleri
- 💾 **SQLite Veritabanı**: Kullanıcı bazlı kayıt tutma
- 🤖 **AI Tavsiyeler**: Beslenme önerileri
- � **Bird.com WhatsApp**: Production-ready WhatsApp entegrasyonu

## 🛠️ Kurulum

### Gereksinimler

- Rust 1.70+
- SQLite3
- OpenAI API Key

### 1. Projeyi Klonlayın

```bash
git clone <repo-url>
cd tavari
```

### 2. Environment Ayarları

`.env` dosyası oluşturun:

```bash
cp .env.example .env
```

`.env` dosyasını düzenleyin:

```env
# OpenAI API Configuration
OPENAI_API_KEY=sk-your-api-key-here

# Meal reminder times (24-hour format)
BREAKFAST_TIME=09:00
LUNCH_TIME=13:00
DINNER_TIME=19:00

# Water reminder interval (in minutes)
WATER_REMINDER_INTERVAL=120

# Database path
DB_PATH=./data/nutrition.db
```

### 3. Bağımlılıkları Yükleyin ve Çalıştırın

```bash
# Build
cargo build --release

# Run
cargo run --release
```

Veya development mode:

```bash
RUST_LOG=info cargo run
```

## 📱 Kullanım

### Komutlar

- 🍽️ **Yemek Resmi Gönder** → Kalori analizi
- 💧 `250 ml su içtim` → Su tüketimi kaydı
- 📊 `/rapor` → Günlük özet
- 📜 `/gecmis` → Son 5 öğün
- 💡 `/tavsiye` → AI beslenme tavsiyesi
- ❓ `/yardim` → Yardım mesajı

### Örnek Kullanım

1. **Yemek Analizi**:
   - Yemek fotoğrafı gönderin
   - Bot kalori bilgisini verir
   - Otomatik olarak kaydeder

2. **Su Kaydı**:
   - "250 ml su içtim" yazın
   - Veya "1 bardak su" yazın
   - Günlük toplam gösterilir

3. **Günlük Rapor**:
   - `/rapor` komutu ile
   - Toplam kalori, su, öğün sayısı
   - Motivasyon mesajı

## 🏗️ Proje Yapısı

```
tavari/
├── src/
│   ├── main.rs                 # Ana uygulama
│   ├── models/
│   │   └── mod.rs              # Veri modelleri (User, Meal, WaterLog)
│   ├── services/
│   │   ├── mod.rs
│   │   ├── database.rs         # SQLite veritabanı
│   │   ├── openai.rs           # OpenAI Vision API
│   │   └── whatsapp.rs         # WhatsApp entegrasyonu
│   └── handlers/
│       ├── mod.rs
│       ├── message_handler.rs  # Mesaj işleme
│       └── reminder.rs         # Hatırlatma servisi
├── data/                       # SQLite veritabanı
├── Cargo.toml
├── .env.example
└── README.md
```

## 🔧 WhatsApp Entegrasyonu

Şu anda kod **Mock WhatsApp Client** kullanıyor. Gerçek WhatsApp entegrasyonu için:

### Seçenek 1: WhatsApp Business API

```rust
// main.rs içinde
let whatsapp_api_key = env::var("WHATSAPP_API_KEY").unwrap();
let phone_number_id = env::var("WHATSAPP_PHONE_NUMBER_ID").unwrap();
let whatsapp = Arc::new(WhatsAppBusinessClient::new(
    whatsapp_api_key,
    phone_number_id
)) as Arc<dyn WhatsAppService>;
```

`.env` dosyasına ekleyin:
```env
WHATSAPP_API_KEY=your-meta-api-key
WHATSAPP_PHONE_NUMBER_ID=your-phone-number-id
```

### Seçenek 2: whatsmeow (Go) ile Bridge

[whatsmeow](https://github.com/tulir/whatsmeow) kullanarak Go bridge yapabilirsiniz.

### Seçenek 3: Python whatsapp-web.js Bridge

Node.js [whatsapp-web.js](https://github.com/pedroslopez/whatsapp-web.js) ile bridge.

## 📊 Veritabanı Şeması

### users
```sql
- phone_number (TEXT, PRIMARY KEY)
- created_at (TEXT)
- breakfast_reminder (INTEGER)
- lunch_reminder (INTEGER)
- dinner_reminder (INTEGER)
- water_reminder (INTEGER)
```

### meals
```sql
- id (INTEGER, PRIMARY KEY)
- user_phone (TEXT)
- meal_type (TEXT)
- calories (REAL)
- description (TEXT)
- image_path (TEXT)
- created_at (TEXT)
```

### water_logs
```sql
- id (INTEGER, PRIMARY KEY)
- user_phone (TEXT)
- amount_ml (INTEGER)
- created_at (TEXT)
```

## 🔐 Güvenlik

- API anahtarlarını `.env` dosyasında saklayın
- `.env` dosyasını Git'e eklemeyin (`.gitignore`'da var)
- Gerçek kullanımda rate limiting ekleyin
- WhatsApp webhook'ları için signature doğrulama yapın

## 🧪 Test

```bash
# Unit testler
cargo test

# Integration testler
cargo test --test integration_tests
```

## 📝 Geliştirme Notları

- **OpenAI Model**: `gpt-4o-mini` kullanılıyor (maliyet optimizasyonu)
- **Resim Formatı**: PNG, JPG, JPEG destekleniyor
- **Cron Schedule**: UTC timezone kullanılıyor
- **Logging**: `env_logger` ile `RUST_LOG=info` seviyesinde

## 🚀 Deployment

### Docker ile

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libsqlite3-0 ca-certificates
COPY --from=builder /app/target/release/whatsapp-nutrition-bot /usr/local/bin/
COPY .env /app/.env
CMD ["whatsapp-nutrition-bot"]
```

```bash
docker build -t nutrition-bot .
docker run -d --env-file .env nutrition-bot
```

## 🤝 Katkıda Bulunma

1. Fork yapın
2. Feature branch oluşturun (`git checkout -b feature/amazing-feature`)
3. Commit yapın (`git commit -m 'feat: Add amazing feature'`)
4. Push yapın (`git push origin feature/amazing-feature`)
5. Pull Request açın

## 📄 Lisans

MIT License

## 🙏 Teşekkürler

- [OpenAI](https://openai.com/) - Vision API
- [whatsapp-web.js](https://github.com/pedroslopez/whatsapp-web.js) - WhatsApp Web implementasyonu
- Rust Community

## 📮 İletişim

Sorularınız için issue açabilirsiniz.

---

**Not**: Bu proje eğitim amaçlıdır. Gerçek kullanımda WhatsApp Terms of Service'i kontrol edin.
