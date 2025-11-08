# Deployment Guide

## Önemli: Resim Persistence İçin

Resimlerin deploy sonrası kaybolmaması için şu adımları takip edin:

### 1. İlk Deployment

```bash
# Uzak sunucuda
cd /path/to/project
mkdir -p data/images  # Bu dizini MANUEL oluşturun!
chmod 755 data
chmod 755 data/images
```

### 2. Her Deployment Sonrası

```bash
# Docker container'ları durdur (ama volumes'ları SİLME!)
docker-compose down

# Yeni image'ı build et
docker-compose build

# Container'ları başlat
docker-compose up -d

# İzinleri kontrol et
docker-compose exec app ls -la /app/data/images
```

### 3. Volume Mount Kontrolü

`docker-compose.yml` dosyasında şu satırın olduğundan emin olun:

```yaml
volumes:
  - ./data:/app/data  # ✅ Bu satır OLMALI
```

### 4. Resimlerin Kaydedildiğini Kontrol Etme

```bash
# Host'ta
ls -la data/images/

# Container içinde
docker-compose exec app ls -la /app/data/images/
```

### 5. Sorun Giderme

Eğer resimler hala kayboluyor ise:

```bash
# Container loglarını kontrol et
docker-compose logs app | grep -i "image\|directory\|permission"

# Volume mount'u kontrol et
docker-compose exec app pwd
docker-compose exec app ls -la /app/data

# İzinleri düzelt
docker-compose exec app chown -R appuser:appuser /app/data
docker-compose exec app chmod -R 755 /app/data
```

## ⚠️ ASLA YAPMAYIN

```bash
# ❌ Bu komut volumes'ları SİLER!
docker-compose down -v

# ❌ data dizinini gitignore'a eklemeyin - zaten .dockerignore'da var
```

## ✅ Doğru Deployment Flow

```bash
cd /path/to/project
git pull origin main

# Eğer data dizini yoksa oluştur
if [ ! -d "data/images" ]; then
    mkdir -p data/images
    chmod 755 data data/images
fi

# Deploy
docker-compose down        # -v bayrağı OLMADAN!
docker-compose build --no-cache
docker-compose up -d

# Verify
docker-compose logs app | tail -20
curl http://localhost:8080/health
```

## Admin Dashboard

```
http://your-domain:8080/admin?token=YOUR_ADMIN_TOKEN
```

## Logs

```bash
# Tüm logları görüntüle
docker-compose logs -f app

# Sadece hataları
docker-compose logs app | grep -i error

# Resim kaydetme loglarını
docker-compose logs app | grep -i "image\|writing\|download"
```
