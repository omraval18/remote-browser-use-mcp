from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from llm_browser.agent import Agent
from llm_browser.session.store import SessionStore
from llm_browser.session.trace import build_self_eval_prompt, build_trace_bundle, write_trace_bundle
from llm_browser.tool.result import ToolImage


class TraceBundleTest(unittest.TestCase):
    def test_trace_bundle_and_self_eval_prompt(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = SessionStore(Path(tmp))
            session = Agent(store).run("trace me", cwd=Path(tmp))

            bundle = build_trace_bundle(store, session.id)
            path = write_trace_bundle(store, session.id)
            prompt = build_self_eval_prompt(store, session.id)

            self.assertEqual(bundle["session"]["id"], session.id)
            self.assertIn("image_events", bundle)
            self.assertTrue(path.exists())
            self.assertIn("Evaluate this browser-use-terminal session trace", prompt)
            self.assertIn(str(path), prompt)

    def test_trace_bundle_records_image_timeline_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = SessionStore(Path(tmp))
            session = store.create(cwd=Path(tmp))
            image = ToolImage(
                label="loaded",
                path=str(session.artifact_dir / "browser" / "screenshots" / "001_loaded.png"),
                url="https://example.com",
                title="Example",
                viewport={"width": 1280, "height": 900},
            )
            Path(image.path).parent.mkdir(parents=True, exist_ok=True)
            Path(image.path).write_bytes(b"png")
            store.emit(session.id, "tool.image", {"tool_call_id": "call_1", "name": "python", "image": image.to_dict()})

            bundle = build_trace_bundle(store, session.id)

            self.assertEqual(bundle["image_events"][0]["label"], "loaded")
            self.assertEqual(bundle["image_events"][0]["viewport"]["width"], 1280)
            self.assertEqual(bundle["artifacts"][0]["kind"], "image")


if __name__ == "__main__":
    raise SystemExit(unittest.main())
