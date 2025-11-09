use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Router, Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::services::{AdminService, BirdComClient};

#[derive(Clone)]
pub struct AdminState {
    pub admin_service: Arc<AdminService>,
    pub admin_token: String,
    pub whatsapp: Arc<BirdComClient>,
}

#[derive(Deserialize)]
pub struct AuthQuery {
    token: String,
}

/// Create admin router with all routes
pub fn create_admin_router(admin_service: Arc<AdminService>, admin_token: String, whatsapp: Arc<BirdComClient>) -> Router {
    let state = AdminState {
        admin_service,
        admin_token,
        whatsapp,
    };

    Router::new()
        .route("/", get(admin_dashboard_page))
        .route("/api/dashboard", get(get_dashboard_data))
        .route("/api/users/:phone/meals", get(get_user_meals))
        .route("/api/users/:phone/conversations", get(get_user_conversations))
        .route("/api/users/:phone/toggle-active", post(toggle_user_active))
        .route("/api/users/:phone/send-message", post(send_user_message))
        .route("/api/broadcast", post(broadcast_message))
        .with_state(state)
}

/// Verify admin token
fn verify_token(query: &AuthQuery, admin_token: &str) -> Result<(), StatusCode> {
    if query.token == admin_token {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Main admin dashboard HTML page
async fn admin_dashboard_page(
    Query(query): Query<AuthQuery>,
    State(state): State<AdminState>,
) -> Result<Html<String>, StatusCode> {
    log::info!("Admin dashboard access attempt with token: {}...", &query.token[..query.token.len().min(8)]);
    verify_token(&query, &state.admin_token)?;
    log::info!("Admin dashboard access granted");

    let html = include_str!("../../static/admin_dashboard.html");
    Ok(Html(html.to_string()))
}

/// Get dashboard data (users, stats, etc.)
async fn get_dashboard_data(
    Query(query): Query<AuthQuery>,
    State(state): State<AdminState>,
) -> Result<impl IntoResponse, StatusCode> {
    verify_token(&query, &state.admin_token)?;

    let data = state
        .admin_service
        .get_dashboard_data()
        .await
        .map_err(|e| {
            log::error!("Failed to get dashboard data: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((StatusCode::OK, axum::Json(data)))
}

/// Get meals for a specific user
async fn get_user_meals(
    Path(phone): Path<String>,
    Query(query): Query<AuthQuery>,
    State(state): State<AdminState>,
) -> Result<impl IntoResponse, StatusCode> {
    verify_token(&query, &state.admin_token)?;

    let meals = state
        .admin_service
        .get_user_meals(&phone, 50)
        .await
        .map_err(|e| {
            log::error!("Failed to get user meals: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((StatusCode::OK, axum::Json(meals)))
}

/// Get conversations for a specific user
async fn get_user_conversations(
    Path(phone): Path<String>,
    Query(query): Query<AuthQuery>,
    State(state): State<AdminState>,
) -> Result<impl IntoResponse, StatusCode> {
    verify_token(&query, &state.admin_token)?;

    let conversations = state
        .admin_service
        .get_user_conversations(&phone, 100)
        .await
        .map_err(|e| {
            log::error!("Failed to get user conversations: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((StatusCode::OK, axum::Json(conversations)))
}

/// Toggle user active status
async fn toggle_user_active(
    Path(phone): Path<String>,
    Query(query): Query<AuthQuery>,
    State(state): State<AdminState>,
) -> Result<impl IntoResponse, StatusCode> {
    verify_token(&query, &state.admin_token)?;

    let new_status = state
        .admin_service
        .toggle_user_active(&phone)
        .await
        .map_err(|e| {
            log::error!("Failed to toggle user active status: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    log::info!("User {} active status toggled to: {}", phone, new_status);

    Ok((StatusCode::OK, axum::Json(serde_json::json!({
        "is_active": new_status
    }))))
}

#[derive(Deserialize)]
struct SendMessageRequest {
    message: String,
}

#[derive(Deserialize)]
struct BroadcastRequest {
    target: String,  // "all" or "active"
    message: String,
}

/// Send message to specific user
async fn send_user_message(
    Path(phone): Path<String>,
    Query(query): Query<AuthQuery>,
    State(state): State<AdminState>,
    Json(payload): Json<SendMessageRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    verify_token(&query, &state.admin_token)?;

    state
        .whatsapp
        .send_message(&phone, &payload.message)
        .await
        .map_err(|e| {
            log::error!("Failed to send message to {}: {}", phone, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    log::info!("Admin sent message to {}", phone);

    Ok((StatusCode::OK, axum::Json(serde_json::json!({
        "success": true
    }))))
}

/// Broadcast message to all or active users
async fn broadcast_message(
    Query(query): Query<AuthQuery>,
    State(state): State<AdminState>,
    Json(payload): Json<BroadcastRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    verify_token(&query, &state.admin_token)?;

    let users = if payload.target == "active" {
        state.admin_service.db.get_active_users().await
    } else {
        state.admin_service.db.get_all_users().await
    }
    .map_err(|e| {
        log::error!("Failed to get users for broadcast: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    log::info!("Broadcasting message to {} users (target: {})", users.len(), payload.target);

    let mut sent_count = 0;
    let mut failed_count = 0;

    for user in users {
        match state.whatsapp.send_message(&user.phone_number, &payload.message).await {
            Ok(_) => {
                sent_count += 1;
                log::debug!("Broadcast sent to {}", user.phone_number);
            }
            Err(e) => {
                failed_count += 1;
                log::error!("Failed to send broadcast to {}: {}", user.phone_number, e);
            }
        }
    }

    log::info!("Broadcast complete: {} sent, {} failed", sent_count, failed_count);

    Ok((StatusCode::OK, axum::Json(serde_json::json!({
        "sent": sent_count,
        "failed": failed_count
    }))))
}
