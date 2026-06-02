use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        Html,
        sse::{Event, KeepAlive, Sse},
    },
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use browser_use_browser::run_browser_script;
use serde::Deserialize;
use tokio_stream::wrappers::ReceiverStream;

use crate::AppState;
use crate::auth::bearer_from_headers;
use crate::server::{active_sessions, profiles_base_dir};

#[derive(Deserialize)]
pub struct KeyQuery {
    pub key: Option<String>,
}

fn admin_check(state: &AppState, headers: &HeaderMap, key: Option<&str>) -> bool {
    let token = key
        .filter(|k| !k.is_empty())
        .or_else(|| bearer_from_headers(headers));
    token.map(|t| t == state.admin_secret).unwrap_or(false)
}

// --- JPEG frame capture ---

async fn capture_jpeg(session_id: String) -> anyhow::Result<Vec<u8>> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let artifact_dir = cwd.join(".browser-use").join("artifacts");
    tokio::task::spawn_blocking(move || {
        let code = r#"
result = cdp("Page.captureScreenshot", format="jpeg", quality=55)
print(result["data"], end="")
"#;
        let out = run_browser_script(&session_id, &cwd, &artifact_dir, code, 10)?;
        if !out.ok {
            return Err(anyhow::anyhow!(
                "screenshot failed: {}",
                out.error.unwrap_or_default()
            ));
        }
        let b64 = out.text.trim();
        if b64.is_empty() {
            return Err(anyhow::anyhow!("empty screenshot response"));
        }
        Ok(BASE64.decode(b64)?)
    })
    .await?
}

// --- Session lookup ---

fn session_id_for_user(user_id: &str) -> Option<String> {
    // In HTTP mode the session_id is keyed by user_id, so they're identical.
    // Confirm the session is actually registered before returning.
    let sessions = active_sessions().lock().ok()?;
    if sessions.contains_key(user_id) {
        Some(user_id.to_string())
    } else {
        // Fallback: scan for a session whose user_id field matches
        sessions
            .values()
            .find(|m| m.user_id.as_deref() == Some(user_id))
            .map(|m| m.session_id.clone())
    }
}

// --- SSE stream route ---

pub async fn stream_sse(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    Query(q): Query<KeyQuery>,
    headers: HeaderMap,
) -> Result<impl axum::response::IntoResponse, (StatusCode, &'static str)> {
    if !admin_check(&state, &headers, q.key.as_deref()) {
        return Err((StatusCode::UNAUTHORIZED, "admin auth required"));
    }

    let session_id = session_id_for_user(&user_id).ok_or((
        StatusCode::NOT_FOUND,
        "no active session for this user — have the AI connect a browser first",
    ))?;

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(4);

    tokio::spawn(async move {
        loop {
            match capture_jpeg(session_id.clone()).await {
                Ok(jpeg) => {
                    let b64 = BASE64.encode(&jpeg);
                    if tx.send(Ok(Event::default().data(b64))).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let msg = format!("capture_error: {e}");
                    if tx
                        .send(Ok(Event::default().event("error").data(msg)))
                        .await
                        .is_err()
                    {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
            tokio::time::sleep(Duration::from_millis(800)).await;
        }
    });

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default()))
}

// --- HTML viewer route ---

pub async fn stream_viewer(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    Query(q): Query<KeyQuery>,
    headers: HeaderMap,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    if !admin_check(&state, &headers, q.key.as_deref()) {
        return Err((StatusCode::UNAUTHORIZED, "admin auth required"));
    }

    let profiles_dir = profiles_base_dir();
    let session_info = {
        let sessions = active_sessions().lock().unwrap_or_else(|e| e.into_inner());
        sessions.get(&user_id).cloned()
    };

    let status = if session_info.is_some() {
        format!("Active session for <strong>{user_id}</strong>")
    } else {
        format!(
            "No active session for <strong>{user_id}</strong> — \
             the AI needs to run <code>browser connect local</code> first."
        )
    };

    let profile_path = session_info
        .and_then(|s| s.profile_path)
        .unwrap_or_else(|| profiles_dir.join(&user_id).to_string_lossy().to_string());

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Browser View — {user_id}</title>
<style>
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{ background: #0d0d0d; color: #e0e0e0; font-family: system-ui, sans-serif; display: flex; flex-direction: column; height: 100vh; }}
  header {{ padding: 10px 16px; background: #1a1a1a; border-bottom: 1px solid #333; display: flex; align-items: center; gap: 12px; flex-shrink: 0; }}
  header h1 {{ font-size: 14px; font-weight: 600; }}
  #status-dot {{ width: 10px; height: 10px; border-radius: 50%; background: #555; flex-shrink: 0; }}
  #status-dot.live {{ background: #4ade80; box-shadow: 0 0 6px #4ade80; }}
  #status-dot.error {{ background: #f87171; }}
  #info {{ font-size: 12px; color: #888; margin-left: auto; }}
  #fps-counter {{ font-size: 12px; color: #60a5fa; }}
  main {{ flex: 1; display: flex; align-items: center; justify-content: center; overflow: hidden; padding: 8px; }}
  #frame {{ max-width: 100%; max-height: 100%; object-fit: contain; border-radius: 4px; border: 1px solid #333; }}
  #placeholder {{ color: #555; font-size: 14px; text-align: center; line-height: 1.6; }}
  footer {{ padding: 8px 16px; background: #1a1a1a; border-top: 1px solid #333; font-size: 11px; color: #555; }}
</style>
</head>
<body>
<header>
  <div id="status-dot"></div>
  <h1>browser-use &mdash; {user_id}</h1>
  <span id="fps-counter"></span>
  <span id="info">{status} &nbsp;|&nbsp; profile: <code>{profile_path}</code></span>
</header>
<main>
  <img id="frame" alt="waiting for first frame..." style="display:none">
  <div id="placeholder">Waiting for browser session&hellip;<br><small>The AI needs to connect a browser before frames appear.</small></div>
</main>
<footer>Stream: <code>/stream/{user_id}</code> &nbsp;&bull;&nbsp; ~1 fps JPEG via SSE &nbsp;&bull;&nbsp; read-only view</footer>
<script>
(function() {{
  const img = document.getElementById('frame');
  const placeholder = document.getElementById('placeholder');
  const dot = document.getElementById('status-dot');
  const fpsEl = document.getElementById('fps-counter');
  let frameCount = 0, lastFpsTime = Date.now();

  const src = '/stream/{user_id}';
  const qs = new URLSearchParams(window.location.search);
  const key = qs.get('key') || '';
  const url = key ? src + '?key=' + encodeURIComponent(key) : src;

  const es = new EventSource(url);

  es.onmessage = function(e) {{
    img.src = 'data:image/jpeg;base64,' + e.data;
    img.style.display = '';
    placeholder.style.display = 'none';
    dot.className = 'live';
    frameCount++;
    const now = Date.now();
    if (now - lastFpsTime >= 2000) {{
      const fps = (frameCount / ((now - lastFpsTime) / 1000)).toFixed(1);
      fpsEl.textContent = fps + ' fps';
      frameCount = 0; lastFpsTime = now;
    }}
  }};

  es.addEventListener('error', function(e) {{
    dot.className = 'error';
  }});

  es.onerror = function() {{
    dot.className = 'error';
    fpsEl.textContent = 'reconnecting…';
  }};
}})();
</script>
</body>
</html>"#
    );

    Ok(Html(html))
}
