use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::AppState;

// Task-local that carries the authenticated user_id through the async call chain
// from the auth middleware into the BrowserServer factory. This avoids any need
// to thread user identity through rmcp's factory closure via shared state.
tokio::task_local! {
    pub static AUTHED_USER_ID: String;
}

/// Guards `/mcp` — validates Bearer token, sets AUTHED_USER_ID for the factory.
pub async fn user_auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let Some(user_id) = resolve_bearer(&state, &req) else {
        return (StatusCode::UNAUTHORIZED, "Invalid or missing API key\n").into_response();
    };
    AUTHED_USER_ID.scope(user_id, next.run(req)).await
}

/// Guards `/api/*` — checks the ADMIN_SECRET env var / request header.
pub async fn admin_auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    if bearer_token(&req)
        .map(|t| t == state.admin_secret)
        .unwrap_or(false)
    {
        next.run(req).await
    } else {
        (StatusCode::UNAUTHORIZED, "Invalid admin secret\n").into_response()
    }
}

fn resolve_bearer(state: &AppState, req: &Request) -> Option<String> {
    let token = bearer_token(req)?;
    state.db.validate_key(token).ok()?
}

pub fn bearer_token(req: &Request) -> Option<&str> {
    bearer_from_headers(req.headers())
}

pub fn bearer_from_headers(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}
