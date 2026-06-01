use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;

#[derive(Deserialize)]
pub struct CreateUserBody {
    pub user_id: String,
}

pub async fn create_user(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateUserBody>,
) -> impl IntoResponse {
    if !is_valid_user_id(&body.user_id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "user_id may only contain letters, digits, hyphens, underscores, and dots"
            })),
        )
            .into_response();
    }

    if state.db.user_exists(&body.user_id) {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "error": "user already exists — use rotate-key to issue a new key" })),
        )
            .into_response();
    }

    match state.db.create_user(&body.user_id) {
        Ok(api_key) => (
            StatusCode::CREATED,
            Json(json!({
                "user_id": body.user_id,
                "api_key": api_key,
                "note": "Store this key — it will not be shown again.",
                "mcp_config": {
                    "url": "<your-vps-url>/mcp",
                    "headers": { "Authorization": format!("Bearer {api_key}") }
                }
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("{e:#}") })),
        )
            .into_response(),
    }
}

pub async fn list_users(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.list_users() {
        Ok(users) => Json(json!({ "users": users, "count": users.len() })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("{e:#}") })),
        )
            .into_response(),
    }
}

pub async fn revoke_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
) -> impl IntoResponse {
    match state.db.revoke_user(&user_id) {
        Ok(true) => Json(json!({
            "revoked": true,
            "user_id": user_id,
            "note": "Chrome profile preserved on disk. Use rotate-key to re-enable."
        }))
        .into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "user not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("{e:#}") })),
        )
            .into_response(),
    }
}

pub async fn rotate_key(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
) -> impl IntoResponse {
    match state.db.rotate_key(&user_id) {
        Ok(Some(api_key)) => Json(json!({
            "user_id": user_id,
            "api_key": api_key,
            "note": "Previous key is now invalid."
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "user not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("{e:#}") })),
        )
            .into_response(),
    }
}

fn is_valid_user_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
}
