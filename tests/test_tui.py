from __future__ import annotations

import unittest

from llm_browser.events import Event
from llm_browser.tui.app import _current_tool, _final_line, _format_events_for_log, _short_task_list, _summarize_task_text
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

    def test_format_tool_output_truncates_stream_chunk(self) -> None:
        event = Event(
            type="tool.output",
            session_id="s1",
            payload={"name": "shell", "stream": "stdout", "text": "b" * 200},
        )

        formatted = format_event(event)
        self.assertIn("[s1] tool output: shell stdout", formatted)
        self.assertLess(len(formatted), 230)

    def test_format_events_for_log_coalesces_model_deltas(self) -> None:
        events = [
            Event(type="model.delta", session_id="s1", payload={"text": "Hel"}),
            Event(type="model.delta", session_id="s1", payload={"text": "lo"}),
            Event(type="tool.started", session_id="s1", payload={"name": "python", "tool_call_id": "c1"}),
        ]

        lines = _format_events_for_log(events)

        self.assertEqual(lines[0], ("[s1] model: Hello", "model.delta"))
        self.assertIn("tool start: python", lines[1][0])

    def test_summarize_task_text_extracts_dataset_task(self) -> None:
        text = (
            "You are running a browser-use-terminal dataset task.\n"
            "Dataset: real_v8\n\n"
            "Task:\n"
            "Visit a site and save the result.\n\n"
            "Runtime budget: this task has a hard timeout."
        )

        self.assertEqual(_summarize_task_text(text), "Visit a site and save the result.")

    def test_current_tool_tracks_latest_unfinished_call(self) -> None:
        events = [
            Event(type="tool.started", session_id="s1", payload={"name": "python", "tool_call_id": "c1"}),
            Event(type="tool.finished", session_id="s1", payload={"name": "python", "tool_call_id": "c1"}),
            Event(type="tool.started", session_id="s1", payload={"name": "python", "tool_call_id": "c2"}),
        ]

        self.assertEqual(_current_tool(events), "python c2")

    def test_current_tool_falls_back_to_last_finished_call(self) -> None:
        events = [
            Event(type="tool.started", session_id="s1", payload={"name": "python", "tool_call_id": "c1"}),
            Event(type="tool.finished", session_id="s1", payload={"name": "python", "tool_call_id": "c1"}),
        ]

        self.assertEqual(_current_tool(events), "python done")

    def test_final_line_prefers_terminal_session_event(self) -> None:
        events = [
            Event(type="tool.finished", session_id="s1", payload={"name": "python", "output": {"text": "intermediate"}}),
            Event(type="session.done", session_id="s1", payload={"result": "complete"}),
        ]

        self.assertEqual(_final_line(events), "complete")

    def test_short_task_list_limits_long_runs(self) -> None:
        self.assertEqual(_short_task_list([]), "-")
        self.assertEqual(_short_task_list([str(index) for index in range(14)], limit=3), "0, 1, 2 +11")


if __name__ == "__main__":
    raise SystemExit(unittest.main())
