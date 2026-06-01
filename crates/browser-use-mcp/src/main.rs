use std::path::PathBuf;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use anyhow::Result;
use browser_use_browser::{cleanup_session, run_browser_command, run_browser_script};
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content},
    tool, tool_handler, tool_router,
    transport::stdio,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const SESSION_ID: &str = "mcp-session";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BrowserCommandParams {
    /// Browser control command, e.g. "connect local", "disconnect", "doctor", "profiles", "cloud start"
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BrowserScriptParams {
    /// Python code to execute. Uses pre-imported CDP helpers: goto_url, click_at_xy, type_text,
    /// fill_input, screenshot, js, page_info, wait_for_load, wait_for_element, scroll, cdp, etc.
    pub code: String,
    /// Timeout in seconds (default: 60)
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    60
}

#[derive(Clone)]
pub struct BrowserServer {
    cwd: PathBuf,
    artifact_dir: PathBuf,
    tool_router: ToolRouter<Self>,
}

impl BrowserServer {
    fn new() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let artifact_dir = cwd.join(".browser-use").join("artifacts");
        Self {
            cwd,
            artifact_dir,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router(router = tool_router)]
impl BrowserServer {
    #[tool(description = "Browser control plane: connect/disconnect/doctor/recover/profiles/cloud. Pass a command string like 'connect local', 'disconnect', 'doctor', 'profiles', 'cloud start'.")]
    async fn browser(
        &self,
        params: Parameters<BrowserCommandParams>,
    ) -> Result<CallToolResult, McpError> {
        let BrowserCommandParams { command } = params.0;
        let cwd = self.cwd.clone();
        let artifact_dir = self.artifact_dir.clone();

        let result = tokio::task::spawn_blocking(move || {
            run_browser_command(SESSION_ID, &cwd, &artifact_dir, &command)
        })
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let json = serde_json::json!({
            "content": result.content,
            "events": result.events,
        });
        Ok(CallToolResult::success(vec![Content::text(json.to_string())]))
    }

    #[tool(description = "Run Python browser interaction code. CDP helpers are pre-imported: goto_url, click_at_xy, type_text, fill_input, screenshot, js, page_info, wait_for_load, wait_for_element, scroll, cdp, etc. Call browser('connect local') first.")]
    async fn browser_script(
        &self,
        params: Parameters<BrowserScriptParams>,
    ) -> Result<CallToolResult, McpError> {
        let BrowserScriptParams { code, timeout_seconds } = params.0;
        let cwd = self.cwd.clone();
        let artifact_dir = self.artifact_dir.clone();

        let result = tokio::task::spawn_blocking(move || {
            run_browser_script(SESSION_ID, &cwd, &artifact_dir, &code, timeout_seconds)
        })
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let mut contents: Vec<Content> = Vec::new();

        // Inline any screenshot images as base64 so clients don't need filesystem access
        for img in &result.images {
            if let Some(path) = img.get("path").and_then(|p| p.as_str()) {
                if let Ok(bytes) = std::fs::read(path) {
                    let b64 = BASE64.encode(&bytes);
                    contents.push(Content::image(b64, "image/png"));
                }
            }
        }

        let json = serde_json::to_value(&result)
            .unwrap_or_else(|_| serde_json::json!({"ok": false, "error": "serialization failed"}));
        contents.push(Content::text(json.to_string()));

        Ok(CallToolResult::success(contents))
    }
}

#[tool_handler(
    router = self.tool_router,
    name = "browser-use-mcp",
    instructions = "Browser automation server. Call browser('connect local') to attach to Chrome, then use browser_script to run Python CDP interaction code."
)]
impl ServerHandler for BrowserServer {}

fn print_usage() {
    eprintln!("Usage: browser-use-mcp [--http [--port <PORT>] [--host <HOST>]]");
    eprintln!();
    eprintln!("Modes:");
    eprintln!("  (no flags)      stdio — for local MCP clients (Claude Desktop, Cursor, Claude Code)");
    eprintln!("  --http          HTTP  — listens for remote MCP clients (VPS, shared server)");
    eprintln!();
    eprintln!("HTTP options:");
    eprintln!("  --port <PORT>   Port to listen on (default: 3000)");
    eprintln!("  --host <HOST>   Host to bind to (default: 0.0.0.0)");
    eprintln!();
    eprintln!("HTTP client config (Claude Desktop / Cursor):");
    eprintln!("  {{ \"url\": \"http://<host>:<port>/mcp\" }}");
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        print_usage();
        return Ok(());
    }

    if args.contains(&"--http".to_string()) {
        use rmcp::transport::streamable_http_server::{
            StreamableHttpServerConfig, StreamableHttpService,
            session::local::LocalSessionManager,
        };
        use tokio_util::sync::CancellationToken;

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

        let addr: std::net::SocketAddr = format!("{host}:{port}").parse()?;

        let ct = CancellationToken::new();
        let config = StreamableHttpServerConfig::default()
            .disable_allowed_hosts()
            .with_cancellation_token(ct.child_token());

        let service: StreamableHttpService<BrowserServer, LocalSessionManager> =
            StreamableHttpService::new(|| Ok(BrowserServer::new()), Default::default(), config);

        let router = axum::Router::new().nest_service("/mcp", service);
        let listener = tokio::net::TcpListener::bind(addr).await?;

        eprintln!("browser-use-mcp HTTP transport listening on http://{addr}/mcp");
        eprintln!("Connect from MCP client: {{ \"url\": \"http://{addr}/mcp\" }}");

        tokio::select! {
            result = axum::serve(listener, router) => { result?; }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("Shutting down...");
                ct.cancel();
            }
        }
    } else {
        let server = BrowserServer::new();
        let service = server.serve(stdio()).await?;
        service.waiting().await?;
        cleanup_session(SESSION_ID);
    }

    Ok(())
}
