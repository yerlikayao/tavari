# 🔐 Admin Dashboard Kullanım Kılavuzu

## Genel Bakış

Admin Dashboard, WhatsApp Nutrition Bot'unuza gelen tüm kullanıcı aktivitelerini izlemenizi sağlayan güvenli bir web arayüzüdür.

## Özellikler

### 📊 Dashboard Özellikleri
- **Toplam Kullanıcı Sayısı**: Sisteme kayıtlı tüm kullanıcılar
- **Bugün Aktif Kullanıcı**: Bugün mesaj gönderen kullanıcı sayısı
- **Bugün Yemek Sayısı**: Bugün kaydedilen yemek sayısı
- **Toplam Mesaj**: Tüm konuşma sayısı

### 👥 Kullanıcı Detayları
Her kullanıcı için:
- Telefon numarası
- Son aktivite zamanı
- Toplam yemek sayısı
- Toplam mesaj sayısı
- Bugün tüketilen kalori
- Bugün içilen su miktarı

### 💬 Konuşma İzleme
- Kullanıcıların gönderdiği tüm mesajları görüntüleme
- Gelen/Giden mesaj ayrımı
- Mesaj türleri (metin, resim, komut, yanıt, hatırlatma, hata)
- Zaman damgaları

### 🍽️ Yemek Takibi
- Kullanıcıların gönderdiği yemek kayıtları
- Kalori bilgileri
- Yemek açıklamaları
- Yemek türleri (Kahvaltı, Öğle, Akşam, Ara Öğün)

## Kurulum

### 1. Environment Ayarları

`.env` dosyanıza admin token ekleyin:

```env
# Admin Dashboard Configuration
ADMIN_TOKEN=your_secure_random_token_here
```

**ÖNEMLİ:** Production ortamında güçlü, rastgele bir token kullanın!

Token oluşturma örnekleri:
```bash
# Option 1: OpenSSL kullanarak
openssl rand -hex 32

# Option 2: Node.js kullanarak
node -e "console.log(require('crypto').randomBytes(32).toString('hex'))"

# Option 3: Python kullanarak
python3 -c "import secrets; print(secrets.token_hex(32))"
```

### 2. Uygulamayı Çalıştırın

```bash
RUST_LOG=info cargo run
```

### 3. Dashboard'a Erişim

Uygulama başlatıldığında konsola şu şekilde bilgi verir:

```
🔐 Admin dashboard: http://localhost:8080/admin?token=your_token_here
```

Bu URL'yi tarayıcınızda açın.

## Kullanım

### Dashboard Ana Sayfa

1. **URL ile Erişim**:
   ```
   http://localhost:8080/admin?token=YOUR_TOKEN
   ```

2. **Otomatik Yenileme**: Dashboard her 30 saniyede bir otomatik olarak yenilenir

3. **İstatistikler**: Üst kısımda dört ana metrik kartı gösterilir

### Kullanıcı Detayları

1. Bir kullanıcı kartına tıklayın
2. Modal pencerede iki sekme görünür:
   - **💬 Mesajlar**: Tüm konuşma geçmişi
   - **🍽️ Yemekler**: Tüm yemek kayıtları

### Mesaj Renk Kodları

- **Mavi** (sol kenarlık): Gelen mesajlar (kullanıcıdan)
- **Yeşil** (sol kenarlık): Giden mesajlar (bottan)

## API Endpoints

Dashboard aşağıdaki API endpoint'lerini kullanır:

### 1. Dashboard Verileri
```
GET /admin/api/dashboard?token=YOUR_TOKEN
```

Response:
```json
{
  "total_users": 10,
  "active_users_today": 5,
  "total_meals_today": 15,
  "total_conversations_today": 50,
  "users": [...]
}
```

### 2. Kullanıcı Mesajları
```
GET /admin/api/users/:phone/conversations?token=YOUR_TOKEN
```

Response:
```json
[
  {
    "id": 123,
    "user_phone": "+905551234567",
    "direction": "incoming",
    "message_type": "text",
    "content": "Merhaba",
    "metadata": null,
    "created_at": "2025-11-08T10:00:00Z"
  }
]
```

### 3. Kullanıcı Yemekleri
```
GET /admin/api/users/:phone/meals?token=YOUR_TOKEN
```

Response:
```json
[
  {
    "id": 456,
    "user_phone": "+905551234567",
    "meal_type": "Kahvaltı",
    "calories": 350.0,
    "description": "Yumurta ve ekmek",
    "image_path": "./data/images/img_123.jpg",
    "created_at": "2025-11-08T08:00:00Z"
  }
]
```

## Güvenlik

### Token Doğrulama
- Her istek `?token=YOUR_TOKEN` parametresi ile yapılır
- Yanlış token kullanılırsa `401 Unauthorized` hatası döner
- Token environment variable'da saklanır

### Öneriler
1. **Güçlü Token**: En az 32 karakter uzunluğunda rastgele token kullanın
2. **HTTPS**: Production'da HTTPS kullanın
3. **Firewall**: Dashboard'u sadece güvenilir IP'lerden erişilebilir yapın
4. **Token Rotation**: Düzenli olarak token'ı değiştirin
5. **Audit Logs**: Dashboard erişimlerini loglayın (gelecek özellik)

## Deployment

### Docker ile

Dashboard webhook server ile birlikte çalışır:

```dockerfile
# Webhook server port'u expose edin
EXPOSE 8080
```

### Environment Variables

```env
ADMIN_TOKEN=production_secure_token_123abc
```

### Reverse Proxy (Nginx)

```nginx
server {
    listen 443 ssl;
    server_name admin.yourdomain.com;

    location /admin {
        proxy_pass http://localhost:8080/admin;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

## Sorun Giderme

### "Yetkisiz erişim" Hatası
- Token'ın doğru olduğundan emin olun
- `.env` dosyasında `ADMIN_TOKEN` değişkeninin tanımlı olduğunu kontrol edin
- Uygulamayı yeniden başlatın

### Dashboard Yüklenmiyor
- Webhook server'ın çalıştığından emin olun (`http://localhost:8080/health` kontrol edin)
- Browser console'da hata olup olmadığına bakın
- `RUST_LOG=debug cargo run` ile detaylı logları inceleyin

### Veriler Görünmüyor
- PostgreSQL veritabanının çalıştığından emin olun
- `DATABASE_URL` environment variable'ının doğru olduğunu kontrol edin
- En az bir kullanıcının sisteme kayıtlı olduğundan emin olun

## Gelecek Özellikler

- [ ] Kullanıcı arama ve filtreleme
- [ ] Tarih aralığı seçimi
- [ ] CSV/Excel export
- [ ] Grafik ve istatistikler
- [ ] Kullanıcı engelleme/yönetimi
- [ ] Gerçek zamanlı bildirimler (WebSocket)
- [ ] Audit log (kimin ne zaman eriştiği)

## Katkıda Bulunma

Dashboard geliştirmelerine katkıda bulunmak için:
1. Feature branch oluşturun
2. `src/webhook/admin.rs` ve `static/admin_dashboard.html` dosyalarını düzenleyin
3. Pull request açın

## Lisans

Bu özellik ana projenin lisansı altındadır (MIT License).
