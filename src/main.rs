mod handlers;
mod models;
mod services;
mod webhook; // Bird.com webhook handler

#[cfg(feature = "webhook-server")]
use webhook::server::create_webhook_router;
#[cfg(feature = "webhook-server")]

use anyhow::Result;
use dotenv::dotenv;
use std::env;
use std::sync::Arc;

use handlers::{MessageHandler, ReminderService};
use services::{Database, BirdComClient, OpenRouterService, AdminService};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::init();

    // Load environment variables
    dotenv().ok();

    log::info!("🚀 Starting WhatsApp Nutrition Bot...");

    // Load configuration
    let openrouter_api_key = env::var("OPENROUTER_API_KEY")
        .expect("OPENROUTER_API_KEY must be set in .env file");

    let openrouter_model = env::var("OPENROUTER_MODEL")
        .unwrap_or_else(|_| "meta-llama/llama-4-scout:free".to_string());

    // Initialize PostgreSQL database
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let db = Arc::new(Database::new(&database_url).await?);
    log::info!("✅ PostgreSQL database initialized");

    let openai = Arc::new(OpenRouterService::new(openrouter_api_key, openrouter_model.clone()));
    log::info!("✅ OpenRouter service initialized with model: {}", openrouter_model);

    // Bird.com WhatsApp service (Production)
    let bird_api_key = env::var("BIRD_API_KEY")
        .expect("BIRD_API_KEY must be set in .env file");
    let bird_workspace_id = env::var("BIRD_WORKSPACE_ID")
        .expect("BIRD_WORKSPACE_ID must be set in .env file");
    let bird_channel_id = env::var("BIRD_CHANNEL_ID")
        .expect("BIRD_CHANNEL_ID must be set in .env file");

    let bird_client = Arc::new(BirdComClient::new(
        bird_api_key,
        bird_workspace_id,
        bird_channel_id,
    ));
    let whatsapp = bird_client.clone() as Arc<dyn services::WhatsAppService>;
    log::info!("✅ WhatsApp service initialized (Bird.com Production)");

    // Initialize message handler
    let message_handler = Arc::new(MessageHandler::new(
        db.clone(),
        openai.clone(),
        whatsapp.clone(),
    ));
    log::info!("✅ Message handler initialized");

    // Initialize and start reminder service
    let mut reminder_service = ReminderService::new(db.clone(), whatsapp.clone()).await?;
    reminder_service.start().await?;
    log::info!("✅ Reminder service started");

    // Start webhook server with admin dashboard
    #[cfg(feature = "webhook-server")]
    {
        use webhook::admin::create_admin_router;

        let webhook_addr = "0.0.0.0:8080";
        let mut webhook_app = create_webhook_router(message_handler.clone(), bird_client.clone());

        // Add admin dashboard routes with token authentication
        let admin_token = env::var("ADMIN_TOKEN")
            .unwrap_or_else(|_| {
                log::warn!("⚠️ ADMIN_TOKEN not set, using default 'admin123' (INSECURE!)");
                "admin123".to_string()
            });

        let admin_service = Arc::new(AdminService::new(db.clone()));
        let admin_router = create_admin_router(admin_service, admin_token.clone());

        webhook_app = webhook_app.nest("/admin", admin_router);

        log::info!("🌐 Webhook server starting on {}", webhook_addr);
        log::info!("🔐 Admin dashboard: http://localhost:8080/admin?token={}", admin_token);

        tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(webhook_addr)
                .await
                .expect("Failed to bind webhook server");
            axum::serve(listener, webhook_app)
                .await
                .expect("Failed to start webhook server");
        });

        log::info!("✅ Webhook server started");
    }

    log::info!("🎉 Bot is ready!");

    println!("\n📱 Bot çalışıyor!");
    println!("📞 WhatsApp Numarası: +1 302-726-0990");
    println!("🌐 Webhook Server: http://localhost:8080");
    #[cfg(feature = "webhook-server")]
    {
        let admin_url = format!("http://localhost:8080/admin?token={}",
            env::var("ADMIN_TOKEN").unwrap_or_else(|_| "admin123".to_string()));
        println!("🔐 Admin Dashboard: {}", admin_url);
    }
    println!("⏰ Hatırlatma servisi aktif");
    println!("\n💬 WhatsApp'tan mesaj gönderin:");
    println!("   'Merhaba' - Hoşgeldin mesajı");
    println!("   *Yemek fotoğrafı* - Kalori analizi");
    println!("   '250 ml su içtim' - Su kaydı");
    println!("   '/rapor' - Günlük rapor");
    println!("\n🛑 Durdurmak için Ctrl+C basın\n");

    // Keep running
    tokio::signal::ctrl_c().await?;

    log::info!("🛑 Shutting down...");
    reminder_service.stop().await?;

    Ok(())
}
