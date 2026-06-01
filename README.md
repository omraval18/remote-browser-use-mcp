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

## License

MIT
