mod admin;
mod auth;
mod db;
mod server;

use std::sync::Arc;

use anyhow::{bail, Result};
use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use db::Db;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use tokio_util::sync::CancellationToken;

use auth::{admin_auth_middleware, user_auth_middleware};
use server::BrowserServer;

pub struct AppState {
    pub db: Arc<Db>,
    pub admin_secret: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        print_usage();
        return Ok(());
    }

    if args.contains(&"--http".to_string()) {
        run_http(&args).await
    } else {
        server::run_stdio().await
    }
}

async fn run_http(args: &[String]) -> Result<()> {
    let port: u16 = args
        .iter()
        .position(|a| a == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let host = args
        .iter()
        .position(|a| a == "--host")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("0.0.0.0")
        .to_string();

    let admin_secret = std::env::var("ADMIN_SECRET").unwrap_or_default();
    if admin_secret.trim().is_empty() {
        bail!("ADMIN_SECRET env var is required in HTTP mode. Set a strong random secret.");
    }

    let db_path = std::env::var("BROWSER_USE_DB_PATH")
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            format!("{home}/.browser-use/mcp-users.db")
        });

    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db = Db::open(&db_path)?;
    let state = Arc::new(AppState {
        db,
        admin_secret,
    });

    // MCP service: each new HTTP connection gets a fresh BrowserServer instance
    // with its own UUID session. The auth middleware sets AUTHED_USER_ID via
    // task-local so the factory can read the user identity and auto-connect
    // the right Chrome profile.
    let ct = CancellationToken::new();
    let mcp_config = StreamableHttpServerConfig::default()
        .disable_allowed_hosts()
        .with_cancellation_token(ct.child_token());

    let mcp_service: StreamableHttpService<BrowserServer, LocalSessionManager> =
        StreamableHttpService::new(|| Ok(BrowserServer::new()), Default::default(), mcp_config);

    let addr: std::net::SocketAddr = format!("{host}:{port}").parse()?;

    let app = Router::new()
        // MCP endpoint — protected by user API key
        .nest_service("/mcp", mcp_service)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            user_auth_middleware,
        ))
        // Admin REST API — protected by ADMIN_SECRET
        .nest(
            "/api",
            Router::new()
                .route("/users", post(admin::create_user))
                .route("/users", get(admin::list_users))
                .route("/users/{user_id}", delete(admin::revoke_user))
                .route("/users/{user_id}/rotate-key", post(admin::rotate_key))
                .route_layer(middleware::from_fn_with_state(
                    state.clone(),
                    admin_auth_middleware,
                )),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    eprintln!("browser-use-mcp listening on http://{addr}");
    eprintln!("  MCP endpoint : http://{addr}/mcp   (requires user API key)");
    eprintln!("  Admin API    : http://{addr}/api/*  (requires ADMIN_SECRET)");
    eprintln!("  Profiles dir : {}", server::profiles_base_dir().display());
    eprintln!("  DB           : {}", std::env::var("BROWSER_USE_DB_PATH")
        .unwrap_or_else(|_| "~/.browser-use/mcp-users.db".to_string()));

    tokio::select! {
        result = axum::serve(listener, app) => { result?; }
        _ = tokio::signal::ctrl_c() => {
            eprintln!("Shutting down...");
            ct.cancel();
        }
    }

    Ok(())
}

fn print_usage() {
    eprintln!("Usage: browser-use-mcp [--http [--port <PORT>] [--host <HOST>]]");
    eprintln!();
    eprintln!("Modes:");
    eprintln!("  (no flags)      stdio — for local MCP clients (no auth)");
    eprintln!("  --http          HTTP  — multi-user server with auth");
    eprintln!();
    eprintln!("Required env vars (HTTP mode):");
    eprintln!("  ADMIN_SECRET=<strong-random-secret>   protects /api/* routes");
    eprintln!();
    eprintln!("Optional env vars:");
    eprintln!("  BROWSER_USE_PROFILES_DIR=<path>       Chrome profiles base dir");
    eprintln!("                                         (default: ~/.browser-use/profiles/)");
    eprintln!("  BROWSER_USE_DB_PATH=<path>             SQLite DB path");
    eprintln!("                                         (default: ~/.browser-use/mcp-users.db)");
    eprintln!();
    eprintln!("Admin API (all require: Authorization: Bearer $ADMIN_SECRET):");
    eprintln!("  POST   /api/users                     Create user, returns api_key");
    eprintln!("  GET    /api/users                     List all users + last_seen");
    eprintln!("  DELETE /api/users/:user_id             Revoke access (keeps Chrome profile)");
    eprintln!("  POST   /api/users/:user_id/rotate-key  Issue new key, invalidate old");
    eprintln!();
    eprintln!("MCP client config (requires user API key from POST /api/users):");
    eprintln!("  {{ \"url\": \"http://<host>:<port>/mcp\",");
    eprintln!("    \"headers\": {{ \"Authorization\": \"Bearer buak_...\" }} }}");
}
