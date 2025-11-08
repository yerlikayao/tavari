use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::services::AdminService;

#[derive(Clone)]
pub struct AdminState {
    pub admin_service: Arc<AdminService>,
    pub admin_token: String,
}

#[derive(Deserialize)]
pub struct AuthQuery {
    token: String,
}

/// Create admin router with all routes
pub fn create_admin_router(admin_service: Arc<AdminService>, admin_token: String) -> Router {
    let state = AdminState {
        admin_service,
        admin_token,
    };

    Router::new()
        .route("/", get(admin_dashboard_page))
        .route("/api/dashboard", get(get_dashboard_data))
        .route("/api/users/:phone/meals", get(get_user_meals))
        .route("/api/users/:phone/conversations", get(get_user_conversations))
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
    verify_token(&query, &state.admin_token)?;

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
