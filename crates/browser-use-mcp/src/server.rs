use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

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
use uuid::Uuid;

use crate::auth::AUTHED_USER_ID;

// --- Active session registry (in-memory, process-lifetime) ---

#[derive(Debug, Clone, Serialize)]
pub struct SessionMeta {
    pub session_id: String,
    pub user_id: Option<String>,
    pub profile_path: Option<String>,
    pub connected_at_secs: u64,
}

static ACTIVE_SESSIONS: OnceLock<Mutex<HashMap<String, SessionMeta>>> = OnceLock::new();

pub fn active_sessions() -> &'static Mutex<HashMap<String, SessionMeta>> {
    ACTIVE_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn profiles_base_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("BROWSER_USE_PROFILES_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".browser-use").join("profiles")
}

// --- Tool parameter types ---

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BrowserCommandParams {
    /// Browser control command: "connect local", "disconnect", "doctor", "profiles", "cloud start", etc.
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BrowserScriptParams {
    /// Python code to execute. CDP helpers pre-imported: goto_url, click_at_xy, type_text,
    /// fill_input, screenshot, js, page_info, wait_for_load, wait_for_element, scroll, cdp, etc.
    pub code: String,
    /// Timeout in seconds (default: 60)
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    60
}

// --- BrowserServer ---

#[derive(Clone)]
pub struct BrowserServer {
    session_id: String,
    user_id: Option<String>,
    cwd: PathBuf,
    artifact_dir: PathBuf,
    tool_router: ToolRouter<Self>,
}

impl BrowserServer {
    pub fn new() -> Self {
        let session_id = Uuid::new_v4().to_string();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let artifact_dir = cwd.join(".browser-use").join("artifacts");

        // Read user identity set by the auth middleware via task-local.
        // Works because the factory is called within next.run(req) which is
        // scoped inside AUTHED_USER_ID.scope(...) in the middleware.
        // Falls back to None in stdio mode (no middleware).
        let user_id = AUTHED_USER_ID.try_with(|id| id.clone()).ok();

        // Key the browser session by user_id so it is stable across requests.
        // LocalSessionManager creates a new BrowserServer per HTTP request, so
        // using a per-instance UUID would give each tool call an empty session.
        // With a user-scoped key all requests for "alice" share one Chrome session.
        let session_id = user_id
            .as_ref()
            .map(|uid| format!("user-{uid}"))
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let connected_at_secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let profile_path = user_id.as_ref().map(|uid| {
            profiles_base_dir().join(uid).display().to_string()
        });

        active_sessions()
            .lock()
            .expect("session registry poisoned")
            .entry(session_id.clone())
            .or_insert(SessionMeta {
                session_id: session_id.clone(),
                user_id: user_id.clone(),
                profile_path: profile_path.clone(),
                connected_at_secs,
            });

        let server = Self {
            session_id: session_id.clone(),
            user_id: user_id.clone(),
            cwd: cwd.clone(),
            artifact_dir: artifact_dir.clone(),
            tool_router: Self::tool_router(),
        };

        // Auto-connect only when the browser session isn't already connected.
        // Subsequent requests reuse the same session_id so Chrome only launches once.
        if let Some(uid) = &user_id {
            let already_connected = run_browser_command(
                &session_id, &cwd, &artifact_dir, "status",
            )
            .ok()
            .and_then(|o| o.content.get("connection").and_then(|v| v.as_str()).map(|s| s == "connected"))
            .unwrap_or(false);

            if !already_connected {
                let profile_path = profiles_base_dir().join(uid);
                let cmd = format!("connect managed --profile {}", profile_path.display());
                if let Err(e) = run_browser_command(&session_id, &cwd, &artifact_dir, &cmd) {
                    eprintln!("[session {session_id}] auto-connect for {uid} failed: {e:#}");
                }
            }
        }

        server
    }
}

impl Drop for BrowserServer {
    fn drop(&mut self) {
        // Session cleanup intentionally omitted: multiple BrowserServer instances
        // created per HTTP request all share one browser session keyed by user_id.
        // The process-global SESSIONS map owns the connection; dropping a request
        // handler should not tear it down.
        let _ = &self.session_id;
        let _ = &self.user_id;
    }
}

#[tool_router(router = tool_router)]
impl BrowserServer {
    /// Raw browser control plane. Use this for connect/disconnect/doctor/recover/profiles.
    /// In HTTP mode the browser is already connected to your persistent profile — you
    /// only need this for recovery or manual overrides.
    #[tool(description = "Browser control plane: connect/disconnect/doctor/recover/profiles/cloud. Pass a command like 'status', 'doctor', 'disconnect', 'connect local', 'cloud start'. In HTTP mode your persistent profile is already connected automatically.")]
    async fn browser(
        &self,
        params: Parameters<BrowserCommandParams>,
    ) -> Result<CallToolResult, McpError> {
        let BrowserCommandParams { command } = params.0;
        let cwd = self.cwd.clone();
        let artifact_dir = self.artifact_dir.clone();
        let session_id = self.session_id.clone();

        let result = tokio::task::spawn_blocking(move || {
            run_browser_command(&session_id, &cwd, &artifact_dir, &command)
        })
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let json = serde_json::json!({
            "session_id": self.session_id,
            "user_id": self.user_id,
            "content": result.content,
            "events": result.events,
        });
        Ok(CallToolResult::success(vec![Content::text(json.to_string())]))
    }

    /// Run Python CDP interaction code against this session's browser.
    #[tool(description = "Run Python browser interaction code. CDP helpers are pre-imported: goto_url, click_at_xy, type_text, fill_input, screenshot, js, page_info, wait_for_load, wait_for_element, scroll, cdp, etc.")]
    async fn browser_script(
        &self,
        params: Parameters<BrowserScriptParams>,
    ) -> Result<CallToolResult, McpError> {
        let BrowserScriptParams { code, timeout_seconds } = params.0;
        let cwd = self.cwd.clone();
        let artifact_dir = self.artifact_dir.clone();
        let session_id = self.session_id.clone();

        let result = tokio::task::spawn_blocking(move || {
            run_browser_script(&session_id, &cwd, &artifact_dir, &code, timeout_seconds)
        })
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let mut contents: Vec<Content> = Vec::new();
        for img in &result.images {
            if let Some(path) = img.get("path").and_then(|p| p.as_str()) {
                if let Ok(bytes) = std::fs::read(path) {
                    contents.push(Content::image(BASE64.encode(&bytes), "image/png"));
                }
            }
        }

        let json = serde_json::to_value(&result)
            .unwrap_or_else(|_| serde_json::json!({"ok": false, "error": "serialization failed"}));
        contents.push(Content::text(json.to_string()));

        Ok(CallToolResult::success(contents))
    }

    /// List all active MCP sessions and known Chrome profiles on disk.
    #[tool(description = "List all active browser sessions on this MCP server and known persistent profiles on disk. Shows session IDs, user_ids, profile paths, and connect timestamps.")]
    async fn list_sessions(
        &self,
        _params: Parameters<serde_json::Value>,
    ) -> Result<CallToolResult, McpError> {
        let sessions: Vec<SessionMeta> = active_sessions()
            .lock()
            .expect("session registry poisoned")
            .values()
            .cloned()
            .collect();

        let profiles_dir = profiles_base_dir();
        let known_profiles = scan_profile_dirs(&profiles_dir);

        let json = serde_json::json!({
            "active_sessions": sessions,
            "active_count": sessions.len(),
            "profiles_dir": profiles_dir.display().to_string(),
            "known_profiles": known_profiles,
        });
        Ok(CallToolResult::success(vec![Content::text(json.to_string())]))
    }
}

fn scan_profile_dirs(base: &PathBuf) -> Vec<serde_json::Value> {
    let Ok(entries) = std::fs::read_dir(base) else {
        return vec![];
    };
    let mut profiles: Vec<_> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| {
            let user_id = e.file_name().to_string_lossy().to_string();
            let path = e.path();
            let size_kb = shallow_dir_size_kb(&path);
            serde_json::json!({
                "user_id": user_id,
                "path": path.display().to_string(),
                "size_kb": size_kb,
            })
        })
        .collect();
    profiles.sort_by(|a, b| {
        a["user_id"].as_str().unwrap_or("").cmp(b["user_id"].as_str().unwrap_or(""))
    });
    profiles
}

fn shallow_dir_size_kb(path: &PathBuf) -> u64 {
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .flatten()
                .fold(0u64, |acc, e| acc + e.metadata().map(|m| m.len() / 1024).unwrap_or(0))
        })
        .unwrap_or(0)
}

#[tool_handler(
    router = self.tool_router,
    name = "browser-use-mcp",
    instructions = "Browser automation server with per-user session isolation and persistent Chrome profiles. \
    In HTTP mode your Chrome profile (cookies, logins) is automatically attached when you connect — \
    just call browser_script() to start automating. Use browser('status') to check the connection. \
    Call list_sessions() to see all active users."
)]
impl ServerHandler for BrowserServer {}

/// Run the server in stdio mode (no auth, for local MCP clients).
pub async fn run_stdio() -> Result<()> {
    let server = BrowserServer::new();
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
