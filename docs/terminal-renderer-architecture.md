# Terminal Renderer Architecture

This document describes the current renderer architecture. It replaces the old
phased build plan: the transcript model is now the default path, not a sidecar
experiment.

## What Owns What

`crates/browser-use-tui/src/transcript.rs` owns the transcript model.

- It reads the selected session's events from the in-memory app cache.
- It reduces those events into `TranscriptNode`s.
- It separates committed transcript nodes from the currently active node.
- It filters noisy or transient events such as raw model deltas and child
  session internals.
- It produces both styled Ratatui lines and plain terminal scrollback lines.

`crates/browser-use-tui/src/main.rs` owns the terminal lifecycle.

- `TerminalDriver` creates the inline Ratatui terminal.
- It decides when the terminal viewport must be recreated, which should happen
  only for real terminal resize or explicit session replay.
- It emits committed transcript lines into native scrollback through
  `maybe_emit_native_transcript`.
- It then asks Ratatui to draw the live app surface.

`crates/browser-use-tui/src/render.rs` owns app surfaces.

- It draws setup, ready, result, running, browser, history, model, and command
  surfaces.
- In normal task mode, it does not rebuild committed transcript output. Native
  terminal scrollback owns that.
- It asks `transcript.rs` only for the active viewport tail that is still
  changing.
- It owns the composer, footer, and overlays.

## Data Flow

```text
SQLite / event store
        |
        v
App in-memory state cache
        |
        v
transcript::transcript_model(app, state)
        |
        +--> committed nodes
        |        |
        |        v
        |   maybe_emit_native_transcript()
        |        |
        |        v
        |   terminal native scrollback
        |
        +--> active node
                 |
                 v
            render.rs active viewport
                 |
                 v
            composer / status dock
```

The important split is that committed transcript and live changing content have
different owners:

```text
completed text     -> native terminal scrollback
running text       -> Ratatui active viewport
composer/status    -> stable Ratatui dock
overlays           -> Ratatui surfaces
```

## Normal Task Mode

Normal task mode is append-oriented.

```text
+--------------------------------------------+
| terminal scrollback                         |
|                                            |
|  > old prompt                               |
|  final answer                               |
|                                            |
|  > current prompt                           |
|  committed tool/subagent summary            |
|                                            |
+--------------------------------------------+
| active viewport                             |
|  current model text, waiting state, or      |
|  subagent activity that has not finalized   |
+--------------------------------------------+
| composer / footer                           |
+--------------------------------------------+
```

The app should not clear, replay, or resize the terminal for ordinary keypresses,
slash palette changes, model streaming, status changes, or composer edits.

## Invariants

1. `transcript.rs` is the only event-to-transcript reducer.
2. Normal mode has exactly one owner for committed transcript: native terminal
   scrollback.
3. Normal mode has exactly one owner for live changing task content: the active
   viewport.
4. A finalized active node is inserted into native scrollback once, then removed
   from the active viewport.
5. Child/subagent events never leak into the parent transcript as top-level tool
   calls. The parent sees a child summary and, while running, a bounded active
   child activity tail.
6. SQLite is persistence and hydration. The live renderer reads through the app's
   in-memory state/cache, not directly from SQLite on every frame.
7. Composer, slash commands, history, model, browser, and developer surfaces
   must not purge native scrollback.
8. Resize is the only routine path that may rebuild terminal viewport state.

## Why This Fixes The Old Bugs

The old renderer mixed four jobs in one control path:

- projecting events into user-facing transcript;
- drawing committed transcript in Ratatui;
- inserting terminal-native scrollback;
- drawing composer, overlays, and active status.

That coupling caused duplicate transcript blocks, blank gaps, flicker, subagent
events appearing as parent tool calls, and layout jumps when unrelated UI state
changed.

The current design makes those bugs structurally harder:

- committed lines are appended by terminal driver code;
- changing active content is drawn only in the active viewport;
- overlays and composer are separate surfaces;
- the transcript model filters child and transient events before rendering.

## Remaining Cleanup

The architecture is now migrated, but these cleanups are still reasonable:

- Move `TerminalDriver` from `main.rs` into a dedicated `terminal.rs` module once
  the surrounding helper functions are small enough to extract cleanly.
- Add more focused transcript unit tests for child-agent summaries, streaming
  transitions, and resize replay.
- Keep improving visual polish in `render.rs`, but do not add another transcript
  renderer there.

## Verification Standard

Renderer changes must pass the repo-owned TUI verification loop:

```bash
scripts/verify-terminal-ui.sh
```

That command combines Rust/Python tests, deterministic Ratatui dumps, and a real
tmux smoke test. Dumps alone are not enough because many renderer failures only
appear in a real terminal: flicker, stale redraws, unconsumed keys, bracketed
paste leakage, native scrollback replay bugs, and broken resize behavior.
