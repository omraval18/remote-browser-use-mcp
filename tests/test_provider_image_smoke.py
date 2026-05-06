from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from llm_browser.provider.image_smoke import run_image_smoke
from llm_browser.provider.types import ModelEvent


class ImageSmokeProvider:
    def __init__(self) -> None:
        self.messages = []

    def start_turn(self, messages, tools):
        self.messages = messages
        yield ModelEvent.text("red then blue")


class ProviderImageSmokeTest(unittest.TestCase):
    def test_image_smoke_builds_two_frame_tool_visual_context(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            provider = ImageSmokeProvider()
            result = run_image_smoke(provider, Path(tmp))

            self.assertTrue(result["ok"])
            self.assertEqual([image["label"] for image in result["images"]], ["frame_1", "frame_2"])
            self.assertTrue(Path(result["images"][0]["path"]).exists())
            tool_message = provider.messages[2]
            self.assertEqual(tool_message["role"], "tool")
            self.assertIsInstance(tool_message["content"], list)
            image_items = [item for item in tool_message["content"] if item.get("type") == "input_image"]
            self.assertEqual(len(image_items), 2)
            self.assertTrue((Path(tmp) / "image-smoke-result.json").exists())


if __name__ == "__main__":
    raise SystemExit(unittest.main())
