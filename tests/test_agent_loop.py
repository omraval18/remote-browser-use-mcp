from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from llm_browser.agent import Agent
from llm_browser.agent.service import MaxTurnsExceeded
from llm_browser.provider.types import ModelEvent, ToolCall
from llm_browser.session.store import SessionStore


class NeverDoneProvider:
    def start_turn(self, messages, tools):
        yield ModelEvent.call(ToolCall(id=f"call_{len(messages)}", name="echo", arguments={"text": "again"}))


class AgentLoopTest(unittest.TestCase):
    def test_fake_provider_executes_tools_and_finishes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = SessionStore(Path(tmp))
            agent = Agent(store)

            session = agent.run("Open example.com", cwd=Path(tmp))

            loaded = store.load(session.id)
            self.assertIsNotNone(loaded)
            self.assertEqual(loaded.status, "done")

            events = store.events.read(session.id)
            event_types = [event.type for event in events]
            self.assertIn("session.created", event_types)
            self.assertIn("session.input", event_types)
            self.assertIn("model.delta", event_types)
            self.assertEqual(event_types.count("tool.started"), 2)
            self.assertEqual(event_types.count("tool.finished"), 2)
            self.assertIn("session.done", event_types)

            tool_names = [
                event.payload["name"]
                for event in events
                if event.type == "tool.started"
            ]
            self.assertEqual(tool_names, ["echo", "done"])

    def test_max_turns_exhaustion_fails_session(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = SessionStore(Path(tmp))
            agent = Agent(store, provider=NeverDoneProvider(), max_turns=2)

            with self.assertRaises(MaxTurnsExceeded):
                agent.run("keep going", cwd=Path(tmp))

            sessions = store.list()
            self.assertEqual(len(sessions), 1)
            self.assertEqual(sessions[0].status, "failed")
            events = store.events.read(sessions[0].id)
            self.assertEqual(events[-1].type, "session.failed")
            self.assertIn("did not call done", events[-1].payload["error"])

    def test_large_tool_output_spills_to_artifact(self) -> None:
        class LargeOutputProvider:
            def __init__(self):
                self.turn = 0

            def start_turn(self, messages, tools):
                self.turn += 1
                if self.turn == 1:
                    yield ModelEvent.call(ToolCall(id="call_large", name="echo", arguments={"text": "x" * 21000}))
                else:
                    tool_message = [m for m in messages if m.get("role") == "tool"][-1]
                    yield ModelEvent.call(ToolCall(id="call_done", name="done", arguments={"result": tool_message["content"]}))

        with tempfile.TemporaryDirectory() as tmp:
            store = SessionStore(Path(tmp))
            session = Agent(store, provider=LargeOutputProvider()).run("large", cwd=Path(tmp))
            events = store.events.read(session.id)
            large_output = [
                event.payload["output"]
                for event in events
                if event.type == "tool.finished" and event.payload["name"] == "echo"
            ][0]

            self.assertTrue(large_output["data"]["truncated"])
            self.assertTrue(Path(large_output["data"]["output_path"]).exists())
            self.assertIn("full output saved", large_output["text"])


if __name__ == "__main__":
    raise SystemExit(unittest.main())
