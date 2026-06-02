use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
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
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::broadcast;
use tokio_stream::wrappers::ReceiverStream;

use crate::AppState;
use crate::auth::bearer_from_headers;
use crate::server::active_sessions;

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

// ---------------------------------------------------------------------------
// Browser-tab stream — chromiumoxide → CDP Page.startScreencast → JPEG
// ---------------------------------------------------------------------------
// chromiumoxide provides a typed, async CDP client. Chrome pushes JPEG frames
// via Page.screencastFrame events; we ACK each one and broadcast the bytes.

static BROWSER_TX: OnceLock<broadcast::Sender<Arc<Vec<u8>>>> = OnceLock::new();
static BROWSER_STARTED: AtomicBool = AtomicBool::new(false);

fn subscribe_to_browser() -> broadcast::Receiver<Arc<Vec<u8>>> {
    let tx = BROWSER_TX.get_or_init(|| broadcast::channel(32).0);
    if !BROWSER_STARTED.swap(true, Ordering::AcqRel) {
        let tx2 = tx.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = run_browser_screencast(&tx2).await {
                    eprintln!("[browser-stream] {e}");
                }
                BROWSER_STARTED.store(false, Ordering::Release);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });
    }
    tx.subscribe()
}


async fn run_browser_screencast(tx: &broadcast::Sender<Arc<Vec<u8>>>) -> anyhow::Result<()> {
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    eprintln!("[browser-stream] fetching CDP page WS URL...");
    let ws_url = cdp_page_ws_url(9222).await?;
    eprintln!("[browser-stream] connecting to {ws_url}");
    let (ws, _) = connect_async(&ws_url).await?;
    eprintln!("[browser-stream] connected, starting capture loop");
    let (mut write, mut read) = ws.split();

    let mut cmd_id: u64 = 1;
    let mut frame_count: u64 = 0;
    loop {
        write.send(Message::Text(serde_json::json!({
            "id": cmd_id,
            "method": "Page.captureScreenshot",
            "params": { "format": "jpeg", "quality": 80 }
        }).to_string())).await?;
        cmd_id += 1;

        loop {
            match read.next().await {
                Some(Ok(Message::Text(text))) => {
                    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                    if v.get("id").is_some() {
                        if let Some(data) = v["result"]["data"].as_str() {
                            if let Ok(jpeg) = BASE64.decode(data) {
                                frame_count += 1;
                                if frame_count % 30 == 1 {
                                    eprintln!("[browser-stream] frame {frame_count}, {} bytes", jpeg.len());
                                }
                                let _ = tx.send(Arc::new(jpeg));
                            }
                        } else {
                            eprintln!("[browser-stream] no data in result: {}", &text[..text.len().min(200)]);
                        }
                        break;
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e.into()),
                None => return Ok(()),
            }
        }
    }
}

/// GET /json → page-level WebSocket URL for the first open page tab.
async fn cdp_page_ws_url(port: u16) -> anyhow::Result<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    let mut s = TcpStream::connect(("localhost", port)).await?;
    s.write_all(
        format!("GET /json HTTP/1.1\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n")
            .as_bytes(),
    ).await?;
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).await?;
    let body = buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
        .ok_or_else(|| anyhow::anyhow!("no HTTP body from /json"))?;
    let tabs: Vec<serde_json::Value> = serde_json::from_slice(&buf[body..])?;
    tabs.iter()
        .find(|t| t["type"] == "page")
        .and_then(|t| t["webSocketDebuggerUrl"].as_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow::anyhow!("no page tab in Chrome CDP"))
}

// ---------------------------------------------------------------------------
// Desktop stream — ffmpeg MJPEG pipe → parsed JPEG frames → SSE
// ---------------------------------------------------------------------------
// ffmpeg captures the full display via AVFoundation (macOS) / x11grab (Linux)
// and outputs MJPEG frames back-to-back to stdout. We split the raw stream on
// JPEG SOI/EOI boundaries to extract individual frames.

static DESKTOP_TX: OnceLock<broadcast::Sender<Arc<Vec<u8>>>> = OnceLock::new();
static DESKTOP_STARTED: AtomicBool = AtomicBool::new(false);

fn subscribe_to_desktop() -> broadcast::Receiver<Arc<Vec<u8>>> {
    let tx = DESKTOP_TX.get_or_init(|| broadcast::channel(16).0);
    if !DESKTOP_STARTED.swap(true, Ordering::AcqRel) {
        let tx2 = tx.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = run_ffmpeg_desktop(&tx2).await {
                    eprintln!("[desktop-stream] {e}");
                }
                DESKTOP_STARTED.store(false, Ordering::Release);
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        });
    }
    tx.subscribe()
}

/// Detect the AVFoundation video device index for the main screen.
/// Runs `ffmpeg -list_devices` and finds the first "Capture screen" entry.
#[cfg(target_os = "macos")]
async fn avf_screen_device() -> String {
    let Ok(out) = tokio::process::Command::new("ffmpeg")
        .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .output()
        .await
    else {
        return "1".to_owned();
    };
    let stderr = String::from_utf8_lossy(&out.stderr);
    for line in stderr.lines() {
        if line.contains("Capture screen") {
            // Lines look like: "[AVFoundation indev @ ...] [1] Capture screen 0"
            if let Some(start) = line.rfind('[') {
                let rest = &line[start + 1..];
                if let Some(end) = rest.find(']') {
                    let idx = rest[..end].trim().to_owned();
                    if idx.parse::<u32>().is_ok() {
                        return idx;
                    }
                }
            }
        }
    }
    "1".to_owned()
}

async fn run_ffmpeg_desktop(tx: &broadcast::Sender<Arc<Vec<u8>>>) -> anyhow::Result<()> {
    use tokio::io::AsyncReadExt;
    use tokio::process::Command;

    #[cfg(target_os = "macos")]
    let (input_args, fps) = {
        let dev = avf_screen_device().await;
        let dev: &'static str = Box::leak(dev.into_boxed_str());
        (
            vec!["-f", "avfoundation", "-capture_cursor", "1", "-framerate", "15", "-i", dev],
            "15",
        )
    };

    #[cfg(target_os = "linux")]
    let (input_args, fps) = (
        vec!["-f", "x11grab", "-framerate", "15", "-i", ":0.0"],
        "15",
    );

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    return Err(anyhow::anyhow!("desktop capture not supported on this platform"));

    let _ = fps; // used above for documentation clarity
    let mut child = Command::new("ffmpeg")
        .args(&input_args)
        .args([
            "-vf", "scale=1280:-2",
            "-c:v", "mjpeg",
            "-q:v", "4",        // 1=best, 31=worst; 4 ≈ 85% JPEG quality
            "-f", "image2pipe",
            "pipe:1",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()?;

    let mut stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("no stdout"))?;
    let mut buf: Vec<u8> = Vec::with_capacity(1 << 20); // 1 MB initial
    let mut chunk = vec![0u8; 65_536];

    loop {
        let n = stdout.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);

        // Drain complete JPEG frames from the front of buf.
        // A frame starts at the first SOI (0xFF 0xD8) and ends just before
        // the next SOI — ffmpeg outputs valid, back-to-back JPEG frames.
        loop {
            // Trim leading bytes before the first SOI.
            let first_soi = buf.windows(2).position(|w| w == [0xFF, 0xD8]);
            let first_soi = match first_soi {
                Some(p) => p,
                None => {
                    buf.clear();
                    break;
                }
            };
            if first_soi > 0 {
                buf.drain(..first_soi);
            }
            // A second SOI marks the start of the next frame = end of current frame.
            let second_soi = buf[2..].windows(2).position(|w| w == [0xFF, 0xD8]);
            match second_soi {
                None => break, // current frame not yet complete — wait for more data
                Some(offset) => {
                    let frame_end = offset + 2;
                    let jpeg: Vec<u8> = buf.drain(..frame_end).collect();
                    let _ = tx.send(Arc::new(jpeg));
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Shared: wrap a broadcast receiver as an SSE stream of base64-JPEG events
// ---------------------------------------------------------------------------

fn sse_from_jpeg_broadcast(
    mut rx: broadcast::Receiver<Arc<Vec<u8>>>,
) -> impl axum::response::IntoResponse {
    let (tx, stream_rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(8);
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(jpeg) => {
                    if tx
                        .send(Ok(Event::default().data(BASE64.encode(&*jpeg))))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    });
    Sse::new(ReceiverStream::new(stream_rx)).keep_alive(KeepAlive::default())
}

// ---------------------------------------------------------------------------
// Route: GET /stream/{user_id}  — browser tab stream
// ---------------------------------------------------------------------------

pub async fn stream_sse(
    State(state): State<Arc<AppState>>,
    Path(_user_id): Path<String>,
    Query(q): Query<KeyQuery>,
    headers: HeaderMap,
) -> Result<impl axum::response::IntoResponse, (StatusCode, &'static str)> {
    if !admin_check(&state, &headers, q.key.as_deref()) {
        return Err((StatusCode::UNAUTHORIZED, "admin auth required"));
    }
    Ok(sse_from_jpeg_broadcast(subscribe_to_browser()))
}

// ---------------------------------------------------------------------------
// Route: GET /desktop/{user_id}  — full desktop stream
// ---------------------------------------------------------------------------

pub async fn stream_desktop(
    State(state): State<Arc<AppState>>,
    Path(_user_id): Path<String>,
    Query(q): Query<KeyQuery>,
    headers: HeaderMap,
) -> Result<impl axum::response::IntoResponse, (StatusCode, &'static str)> {
    if !admin_check(&state, &headers, q.key.as_deref()) {
        return Err((StatusCode::UNAUTHORIZED, "admin auth required"));
    }
    Ok(sse_from_jpeg_broadcast(subscribe_to_desktop()))
}

// ---------------------------------------------------------------------------
// Route: GET /view/{user_id}  — HTML viewer with Browser Tab / Desktop tabs
// ---------------------------------------------------------------------------

pub async fn stream_viewer(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    Query(q): Query<KeyQuery>,
    headers: HeaderMap,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    if !admin_check(&state, &headers, q.key.as_deref()) {
        return Err((StatusCode::UNAUTHORIZED, "admin auth required"));
    }

    let session_info = {
        let sessions = active_sessions().lock().unwrap_or_else(|e| e.into_inner());
        sessions.get(&user_id).cloned()
    };
    let status = if session_info.is_some() {
        format!("Active session: <strong>{user_id}</strong>")
    } else {
        format!("Waiting for session: <strong>{user_id}</strong>")
    };

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Browser View — {user_id}</title>
<style>
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{ background: #0d0d0d; color: #e0e0e0; font-family: system-ui, sans-serif;
         display: flex; flex-direction: column; height: 100vh; }}
  header {{ padding: 8px 14px; background: #1a1a1a; border-bottom: 1px solid #2a2a2a;
            display: flex; align-items: center; gap: 10px; flex-shrink: 0; }}
  h1 {{ font-size: 13px; font-weight: 600; white-space: nowrap; }}
  .tabs {{ display: flex; gap: 2px; background: #111; border-radius: 6px; padding: 2px; }}
  .tab {{ padding: 3px 11px; font-size: 12px; border-radius: 4px; cursor: pointer;
          color: #777; border: none; background: none; }}
  .tab.active {{ background: #2d2d2d; color: #e0e0e0; }}
  #dot {{ width: 8px; height: 8px; border-radius: 50%; background: #444;
          flex-shrink: 0; margin-left: auto; transition: background .3s; }}
  #dot.live {{ background: #4ade80; box-shadow: 0 0 5px #4ade80; }}
  #dot.err  {{ background: #f87171; }}
  #fps  {{ font-size: 12px; color: #60a5fa; font-variant-numeric: tabular-nums; min-width: 48px; text-align: right; }}
  #info {{ font-size: 11px; color: #555; white-space: nowrap; overflow: hidden;
           text-overflow: ellipsis; max-width: 30%; }}
  main  {{ flex: 1; display: flex; align-items: center; justify-content: center;
           overflow: hidden; padding: 8px; }}
  canvas {{ max-width: 100%; max-height: 100%; object-fit: contain;
            border-radius: 4px; border: 1px solid #222; display: none; }}
  #ph   {{ color: #444; font-size: 13px; text-align: center; line-height: 1.8; }}
</style>
</head>
<body>
<header>
  <h1>browser-use &mdash; {user_id}</h1>
  <div class="tabs">
    <button class="tab active" data-src="/stream/{user_id}">Browser Tab</button>
    <button class="tab"        data-src="/desktop/{user_id}">Desktop</button>
  </div>
  <div id="dot"></div>
  <div id="fps"></div>
  <div id="info">{status}</div>
</header>
<main>
  <canvas id="cv"></canvas>
  <div id="ph">Connecting…</div>
</main>
<script>
(function() {{
  const cv  = document.getElementById('cv');
  const ctx = cv.getContext('2d');
  const ph  = document.getElementById('ph');
  const dot = document.getElementById('dot');
  const fpsEl = document.getElementById('fps');

  const qs  = new URLSearchParams(window.location.search);
  const key = qs.get('key') || '';
  const auth = key ? '?key=' + encodeURIComponent(key) : '';

  let es = null, pending = null, rafPending = false;
  let fc = 0, lastT = performance.now();

  function renderFrame() {{
    rafPending = false;
    if (!pending) return;
    const bm = pending; pending = null;
    if (cv.width !== bm.width || cv.height !== bm.height) {{
      cv.width = bm.width; cv.height = bm.height;
    }}
    ctx.drawImage(bm, 0, 0); bm.close();
    fc++;
    const now = performance.now();
    if (now - lastT >= 1000) {{
      fpsEl.textContent = (fc / ((now - lastT) / 1000)).toFixed(0) + ' fps';
      fc = 0; lastT = now;
    }}
  }}

  function connect(src) {{
    if (es) {{ es.close(); es = null; }}
    if (pending) {{ pending.close(); pending = null; }}
    dot.className = ''; fpsEl.textContent = '';
    cv.style.display = 'none'; ph.style.display = '';
    ph.textContent = 'Connecting…';

    es = new EventSource(src + auth);

    es.onmessage = function(e) {{
      const raw = atob(e.data);
      const buf = new Uint8Array(raw.length);
      for (let i = 0; i < raw.length; i++) buf[i] = raw.charCodeAt(i);
      createImageBitmap(new Blob([buf], {{ type: 'image/jpeg' }})).then(function(bm) {{
        cv.style.display = ''; ph.style.display = 'none';
        dot.className = 'live';
        if (pending) pending.close();
        pending = bm;
        if (!rafPending) {{ rafPending = true; requestAnimationFrame(renderFrame); }}
      }});
    }};

    es.onerror = function() {{
      dot.className = 'err';
      fpsEl.textContent = '';
      ph.style.display = '';
      ph.textContent = 'Reconnecting…';
    }};
  }}

  document.querySelectorAll('.tab').forEach(function(btn) {{
    btn.addEventListener('click', function() {{
      document.querySelectorAll('.tab').forEach(function(b) {{ b.classList.remove('active'); }});
      btn.classList.add('active');
      connect(btn.dataset.src);
    }});
  }});

  connect('/stream/{user_id}');
}})();
</script>
</body>
</html>"#
    );
    Ok(Html(html))
}
