from __future__ import annotations

import unittest

from llm_browser.events import Event
from llm_browser.tui import format_event


class TuiTest(unittest.TestCase):
    def test_format_image_event(self) -> None:
        event = Event(
            type="tool.image",
            session_id="s1",
            payload={"image": {"label": "loaded", "path": "/tmp/loaded.png"}},
        )

        self.assertEqual(format_event(event), "[s1] image: loaded -> /tmp/loaded.png")

    def test_format_tool_finished_truncates_output(self) -> None:
        event = Event(
            type="tool.finished",
            session_id="s1",
            payload={"name": "shell", "output": {"text": "a" * 200}},
        )

        formatted = format_event(event)
        self.assertIn("[s1] tool done: shell", formatted)
        self.assertLess(len(formatted), 210)


if __name__ == "__main__":
    raise SystemExit(unittest.main())
