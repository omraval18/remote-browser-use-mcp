from __future__ import annotations

import json
import mimetypes
from pathlib import Path
from typing import Any, Dict, List

from llm_browser.session.store import SessionStore


def build_trace_bundle(store: SessionStore, session_id: str, max_events: int = 300) -> Dict[str, Any]:
    session = store.load(session_id)
    if session is None:
        raise KeyError(f"session not found: {session_id}")
    events = store.events.read(session_id)
    artifacts = []
    if session.artifact_dir.exists():
        for path in sorted(session.artifact_dir.rglob("*")):
            if path.is_file() and "chrome-profile" not in path.relative_to(session.artifact_dir).parts:
                artifacts.append(_artifact_record(path))
    state_dir = session.state_dir.resolve()
    cwd = session.cwd.resolve()
    if cwd.exists() and cwd != session.artifact_dir.resolve() and state_dir in cwd.parents:
        for path in sorted(cwd.rglob("*")):
            if path.is_file():
                record = _artifact_record(path)
                record["kind"] = "workspace"
                artifacts.append(record)
    image_events = []
    for event in events:
        if event.type != "tool.image":
            continue
        image = event.payload.get("image") if isinstance(event.payload.get("image"), dict) else {}
        image_events.append(
            {
                "event_id": event.id,
                "ts_ms": event.ts_ms,
                "tool_call_id": event.payload.get("tool_call_id"),
                "label": image.get("label"),
                "path": image.get("path"),
                "url": image.get("url"),
                "title": image.get("title"),
                "order": image.get("order"),
                "viewport": image.get("viewport") or {},
            }
        )
    return {
        "session": session.to_dict(),
        "events": [event.to_dict() for event in events[-max_events:]],
        "event_count": len(events),
        "image_events": image_events[-100:],
        "artifacts": artifacts[-300:],
    }


def write_trace_bundle(store: SessionStore, session_id: str) -> Path:
    session = store.load(session_id)
    if session is None:
        raise KeyError(f"session not found: {session_id}")
    bundle = build_trace_bundle(store, session_id)
    path = session.artifact_dir / "trace-bundle.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(bundle, indent=2) + "\n", encoding="utf-8")
    return path


def build_self_eval_prompt(store: SessionStore, session_id: str) -> str:
    bundle_path = write_trace_bundle(store, session_id)
    bundle = build_trace_bundle(store, session_id, max_events=120)
    compact = {
        "session": bundle["session"],
        "event_count": bundle["event_count"],
        "recent_events": bundle["events"],
        "image_events": bundle["image_events"],
        "artifacts": bundle["artifacts"],
        "trace_bundle_path": str(bundle_path),
    }
    return (
        "Evaluate this browser-use-terminal session trace. "
        "Decide whether the original task appears completed, list evidence, identify likely failures, "
        "and propose the next concrete retry/fix if it failed. Be strict.\n\n"
        f"Trace bundle JSON:\n{json.dumps(compact, indent=2)}"
    )


def _artifact_record(path: Path) -> Dict[str, Any]:
    stat = path.stat()
    suffix = path.suffix.lower()
    kind = "file"
    if suffix in {".png", ".jpg", ".jpeg", ".webp"}:
        kind = "image"
    elif suffix in {".json"} and "trace" in path.name:
        kind = "trace"
    elif suffix in {".json", ".jsonl", ".txt", ".md", ".csv", ".tsv", ".html"}:
        kind = "text"
    elif suffix in {".pdf"}:
        kind = "pdf"
    return {
        "path": str(path),
        "bytes": stat.st_size,
        "kind": kind,
        "mime_type": mimetypes.guess_type(str(path))[0],
    }
