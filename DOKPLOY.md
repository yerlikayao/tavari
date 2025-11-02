# Dokploy Deployment Guide

Bu dosya, tavari botunu Dokploy ile deploy etmek için gerekli adımları içerir.

## Önemli Notlar

### Database Connection

Dokploy'da containerlar arasında bağlantı için **service ismi** kullanılmalıdır, `localhost` ÇALIŞMAZ.

**Doğru:**
```
DATABASE_URL=postgresql://nutrition_user:nutrition_password@postgres:5432/nutrition_bot
```

**Yanlış (çalışmaz):**
```
DATABASE_URL=postgresql://nutrition_user:nutrition_password@localhost:5432/nutrition_bot
```

## Deployment Adımları

### 1. Dokploy'da Yeni Proje Oluştur

1. Dokploy dashboard'a giriş yap
2. "New Project" butonuna tıkla
3. GitHub repository'yi bağla

### 2. Environment Variables Ayarla

Dokploy'da aşağıdaki environment variable'ları ayarla:

```bash
# OpenRouter API
OPENROUTER_API_KEY=sk-or-v1-xxxxx
OPENROUTER_MODEL=meta-llama/llama-4-scout:free

# PostgreSQL (Dokploy için 'postgres' hostname kullan)
DATABASE_URL=postgresql://nutrition_user:nutrition_password@postgres:5432/nutrition_bot

# Bird.com WhatsApp API
BIRD_API_KEY=your_bird_api_key
BIRD_WORKSPACE_ID=your_workspace_id
BIRD_CHANNEL_ID=your_channel_id
BIRD_WEBHOOK_SECRET=your_webhook_secret

# Logging
RUST_LOG=info
```

### 3. Docker Compose Kullan

Dokploy'da "Docker Compose" seçeneğini seç ve `docker-compose.yml` dosyasını kullan.

### 4. Deploy Et

"Deploy" butonuna tıkla. Dokploy otomatik olarak:
- PostgreSQL container'ı başlatacak
- Uygulama container'ını build edecek
- Health check yapacak
- Her şey hazır olduğunda servisi başlatacak

## Sorun Giderme

### DNS Hatası: "failed to lookup address information"

Bu hata, `DATABASE_URL`'de `localhost` kullandığınızda oluşur. Çözüm:

```bash
# Yanlış (localhost kullanıyor)
DATABASE_URL=postgresql://nutrition_user:nutrition_password@localhost:5432/nutrition_bot

# Doğru (service ismini kullanıyor)
DATABASE_URL=postgresql://nutrition_user:nutrition_password@postgres:5432/nutrition_bot
```

### Container Başlamıyor

Dokploy logs'larını kontrol edin:
```bash
docker-compose logs -f app
docker-compose logs -f postgres
```

### Database Bağlantı Hatası

1. PostgreSQL container'ının çalıştığını kontrol edin:
```bash
docker-compose ps
```

2. PostgreSQL health check'ini kontrol edin:
```bash
docker-compose exec postgres pg_isready -U nutrition_user -d nutrition_bot
```

3. Network bağlantısını test edin (app container içinden):
```bash
docker-compose exec app ping postgres
```

## Güvenlik Notları

- `.env` dosyasını asla Git'e commit etmeyin
- Production'da güçlü şifreler kullanın
- API key'leri güvenli bir şekilde saklayın (Dokploy secrets kullanın)
- `BIRD_WEBHOOK_SECRET` için rastgele, güçlü bir değer kullanın

## Monitoring

Logları izlemek için:
```bash
# Tüm servislerin logları
docker-compose logs -f

# Sadece app logları
docker-compose logs -f app

# Sadece postgres logları
docker-compose logs -f postgres
```

## Yeniden Deploy

Kod değişikliği yaptıktan sonra:
1. Git'e push edin
2. Dokploy otomatik olarak yeniden deploy edecek
3. Veya manuel olarak "Redeploy" butonuna tıklayın
