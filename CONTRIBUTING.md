# Contributing

Thanks for wanting to contribute. Browser Use Terminal is early, and we welcome thoughtful contributions.

## Project Structure

```
crates/
  browser-use-browser/     CDP browser runtime
  browser-use-cli/         CLI entry point and install wrappers
  browser-use-core/        Agent loop, tools, subagents, compaction
  browser-use-providers/   LLM provider integrations
  browser-use-protocol/    Internal protocol types
  browser-use-python-worker/  Python subprocess worker for model execution
  browser-use-store/       SQLite event log
  browser-use-tui/         Terminal UI (Ratatui)
docs/                      Architecture and design docs
prompts/                   Model prompt templates
scripts/                   Build, install, and test scripts
python/                    Python test and worker code
```

## Local Setup

```bash
# Clone and enter the repo
git clone https://github.com/browser-use/terminal.git
cd terminal

# Rust toolchain (stable)
rustup default stable

# Python for worker tests
uv sync
```

## Before Opening a PR

- Open an issue before large changes so we can align on scope.
- Keep PRs small and focused.
- Run the relevant checks:

```bash
# Formatting
cargo fmt --check

# Rust tests
cargo test

# Python tests
uv run --with pytest python -m pytest -q
```

## Terminal UI Changes

Terminal UI changes have stricter requirements. A Ratatui screen is a terminal protocol plus keyboard behavior — compilation alone does not prove correctness.

**For any TUI change**, run the full verification suite:

```bash
scripts/verify-terminal-ui.sh
```

This runs formatting, Rust tests, Python tests, deterministic Ratatui dumps, and a real tmux terminal smoke test.

For focused iteration on live terminal behavior:

```bash
scripts/tui-terminal-smoke.py
```

Inspect outputs under `/tmp/but-design-loop/` before finalizing TUI changes.

See `docs/terminal-ui-testing.md` for the testing heuristic.

## Coding Expectations

- Follow existing patterns in the codebase. The app is Rust-first.
- Keep the transcript model in `crates/browser-use-tui/src/transcript.rs` as the single event-to-transcript reducer.
- Overlays, composer, and status are separate surfaces in `render.rs`. Do not mix them into one control path.
- Child/subagent events must not leak into the parent transcript as top-level tool calls.
- Use `anyhow::Result` for fallible functions unless a specific error type is warranted.

## Architecture Docs

- `docs/terminal-ui-product-ux.md` — UX design and product vocabulary
- `docs/terminal-renderer-architecture.md` — Renderer data flow and invariants
- `docs/terminal-ui-testing.md` — TUI testing standards
- `docs/public-launch-checklist.md` — Release readiness checklist

## Questions?

Open a Discussion or Issue on GitHub.
