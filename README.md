<p align="center">
  <img src="static/browser-use-terminal-banner.png" alt="Browser Use Terminal" />
</p>

# Browser Use Terminal

**A browser agent you can steer — from the terminal.**

```bash
curl -fsSL https://browser-use.com/terminal/install.sh | sh
browser-use
```

<p align="center">
  <img src="static/terminal-preview.png" alt="Browser Use Terminal preview" />
</p>

---

## What It Does

Browser Use Terminal automates browser work from a terminal UI. Give it a task in plain language, and the agent browses real websites, fills forms, extracts data, and returns results.

- **Real browser control** — uses your logged-in Chrome, headless Chromium, or Browser Use cloud.
- **You stay in control** — watch, steer, stop, retry, and resume tasks.
- **Local history** — screenshots, artifacts, and follow-ups saved locally.
- **Fast and cheap** — a Rust LLM harness built to be 2x cheaper and 2x faster than previous approaches.

## Quickstart

```bash
# Install
curl -fsSL https://browser-use.com/terminal/install.sh | sh

# Launch
browser-use
```

The first launch walks you through setup: sign in, pick a model, and choose a browser. After that, you're ready to run tasks.

```text
Find the top 5 Hacker News posts and summarize each.
```

```text
Give this employee admin permission in Azure.
```

```text
Find the cancellation policy for my current hotel reservation.
```

## Provider Setup

Browser Use Terminal works with several model providers. Pick the one you already have:

| Provider      | How to connect            |
| ------------- | ------------------------- |
| Codex         | `browser-use /auth` in the TUI |
| OpenAI        | Set `OPENAI_API_KEY` in `~/.browser-use-terminal/.env` |
| Anthropic     | Set `ANTHROPIC_API_KEY` in `~/.browser-use-terminal/.env` |
| OpenRouter    | Set `OPENROUTER_API_KEY` in `~/.browser-use-terminal/.env` |

You can also configure credentials from the TUI with `/auth`.

## How It Works

Browser Use Terminal is a browser-first LLM harness: Rust owns the agent loop and durable state, while the browser runtime gives the model direct CDP control over Chrome.

```text
you
 │
 ▼
browser-use terminal
 │
 ├─ Ratatui TUI         watch · steer · stop · resume
 ├─ Rust LLM harness    tools · subagents · compaction · cancellation
 ├─ SQLite event log    history · screenshots · artifacts · traces
 └─ CDP browser runtime profiles · doctor · recovery · ownership
      │
      ▼
 real Chrome  |  headless Chromium  |  Browser Use cloud
```

## What Works Today

- Running browser tasks from a terminal with live streaming.
- Steering, stopping, retrying, and resuming tasks.
- Following up on completed results.
- Browsing task history and re-running previous work.
- Connecting to your logged-in Chrome for tasks that need real account state.
- Switching between local Chrome, headless Chromium, and Browser Use cloud.
- Multiple model providers: Codex, OpenAI, Anthropic, OpenRouter.

## Known Limitations

- **macOS and Linux only.** Windows is not yet supported.
- **Chrome/Chromium required** for local browser mode. The tool does not bundle a browser.
- **Early software.** APIs, UX, and configuration may change between releases.
- **Browser Use cloud** is available but may require a separate account.

## Local State

Browser Use Terminal stores configuration, history, and credentials at:

```text
~/.browser-use-terminal/
```

Protect this directory like any other application data.

## Telemetry

Anonymous product analytics are enabled by default. They fail open and do not block the app. To opt out:

```bash
export BUT_TELEMETRY=0
```

## Development

```bash
cargo fmt --check
cargo test
uv run --with pytest python -m pytest -q
scripts/verify-terminal-ui.sh
```

Terminal UI changes must pass the full verification script. It runs Rust tests, Python tests, deterministic Ratatui dumps, and a real tmux smoke test.

### Project Layout

```
crates/
  browser-use-browser/     CDP browser runtime
  browser-use-cli/         CLI entry point
  browser-use-core/        Agent loop, tools, subagents
  browser-use-mcp/         MCP server (multi-user HTTP mode)
  browser-use-providers/   LLM provider integrations
  browser-use-protocol/    Internal protocol types
  browser-use-python-worker/  Python subprocess worker
  browser-use-store/       SQLite event log
  browser-use-tui/         Terminal UI (Ratatui)
docs/                      Architecture and design docs
```

## Docs

- `docs/terminal-ui-product-ux.md` — UX design and product vocabulary
- `docs/terminal-ui-testing.md` — TUI testing standards
- `docs/terminal-renderer-architecture.md` — Renderer architecture

---

## MCP Server

`browser-use-mcp` exposes browser automation as an [MCP](https://modelcontextprotocol.io) server. It runs in two modes:

- **stdio** — for local MCP clients (Claude Desktop, Cursor, Claude Code). No auth required.
- **HTTP** — for a shared VPS with multiple users. Each user gets an isolated Chrome profile with persistent cookies and login state.

### Local (stdio)

```json
{
  "mcpServers": {
    "browser-use": {
      "command": "browser-use-mcp"
    }
  }
}
```

### Self-hosted (HTTP, multi-user)

**1. Build**

```bash
cargo build -p browser-use-mcp --release
```

**2. Start the server**

```bash
ADMIN_SECRET=your-strong-secret \
BROWSER_USE_PROFILES_DIR=/data/browser-profiles \
BROWSER_USE_DB_PATH=/data/mcp-users.db \
browser-use-mcp --http --port 3000
```

| Env var | Required | Description |
|---|---|---|
| `ADMIN_SECRET` | **Yes** | Protects the `/api/*` admin routes. Use a long random string. |
| `BROWSER_USE_PROFILES_DIR` | No | Where Chrome profiles are stored (default: `~/.browser-use/profiles/`). Each user gets a subdirectory. |
| `BROWSER_USE_DB_PATH` | No | SQLite DB for user records (default: `~/.browser-use/mcp-users.db`). |

**3. Create users**

```bash
# Create a user — returns an API key shown once
curl -X POST https://your-server.com/api/users \
  -H "Authorization: Bearer your-strong-secret" \
  -H "Content-Type: application/json" \
  -d '{"user_id": "alice"}'

# Response:
# {
#   "user_id": "alice",
#   "api_key": "buak_alice_a1b2c3...",
#   "note": "Store this key — it will not be shown again."
# }
```

**4. Give the key to the user**

Each user adds this to their MCP client config:

```json
{
  "mcpServers": {
    "browser-use": {
      "url": "https://your-server.com/mcp",
      "headers": {
        "Authorization": "Bearer buak_alice_a1b2c3..."
      }
    }
  }
}
```

When they connect, the server automatically launches a headless Chrome instance with their persistent profile. Cookies and login state from previous sessions are loaded automatically.

### Admin API

All admin routes require `Authorization: Bearer <ADMIN_SECRET>`.

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/users` | Create user, returns API key |
| `GET` | `/api/users` | List all users, last seen timestamps |
| `DELETE` | `/api/users/:user_id` | Revoke access (Chrome profile kept on disk) |
| `POST` | `/api/users/:user_id/rotate-key` | Issue new key, invalidate old one |

```bash
# List all users
curl https://your-server.com/api/users \
  -H "Authorization: Bearer your-strong-secret"

# Revoke a user (their Chrome profile is preserved — rotate-key re-enables them)
curl -X DELETE https://your-server.com/api/users/alice \
  -H "Authorization: Bearer your-strong-secret"

# Rotate a user's key
curl -X POST https://your-server.com/api/users/alice/rotate-key \
  -H "Authorization: Bearer your-strong-secret"
```

### MCP Tools

| Tool | Description |
|---|---|
| `browser(command)` | Browser control plane: `status`, `doctor`, `connect local`, `disconnect`, `cloud start` |
| `browser_script(code)` | Run Python CDP interaction code. Helpers pre-imported: `goto_url`, `js`, `screenshot`, `click_at_xy`, `type_text`, `fill_input`, `scroll`, `wait_for_load`, etc. |
| `list_sessions()` | List all active sessions and known Chrome profiles on disk |

In HTTP mode, Chrome connects to the user's profile automatically on first tool call — no manual `connect` needed.

### How profiles work

Each user gets their own Chrome `--user-data-dir`:

```
$BROWSER_USE_PROFILES_DIR/
  alice/    ← Chrome profile (cookies, localStorage, login state)
  bob/
  carol/
```

Profiles are never shared. Revoking a user sets `active=0` in the DB but keeps the Chrome profile directory on disk — if the user is re-enabled via `rotate-key`, their previous session state is still there.

API keys are stored as SHA-256 hashes. The raw key is returned once at creation and never stored.

## License

MIT
