#!/usr/bin/env python3
"""Reader parity for the shared conf (`herdr_common.read_config`).

The Rust helper, plain Vim and these Python fallbacks all read the same file, so
a divergence here is a behaviour divergence on a real keypress. In particular
each key must be parsed *symmetrically*: flipping a default must not make the
opposite value unrepresentable (that was the bug when `nav_at_edge` became
`stop` and this reader only knew how to recognise `stop`).
"""

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))
import herdr_common  # noqa: E402


class ReadConfigTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.conf = Path(self._tmp.name) / "herdr-splits.conf"
        self._env = herdr_common.os.environ.get("HERDR_SPLITS_CONFIG")
        herdr_common.os.environ["HERDR_SPLITS_CONFIG"] = str(self.conf)
        self.addCleanup(self._restore_env)

    def _restore_env(self):
        if self._env is None:
            herdr_common.os.environ.pop("HERDR_SPLITS_CONFIG", None)
        else:
            herdr_common.os.environ["HERDR_SPLITS_CONFIG"] = self._env

    def write(self, text):
        self.conf.write_text(text, encoding="utf-8")

    def test_missing_file_uses_the_shipped_defaults(self):
        cfg = herdr_common.read_config("nav", "left")
        self.assertEqual(
            cfg,
            {
                "key": "ctrl+h",
                "unzoom": False,
                "nav_at_edge": "wrap",
                "most_recently_focused": True,
            },
        )
        self.assertEqual(
            herdr_common.read_config("resize", "right")["key"], "alt+l"
        )

    def test_every_value_is_honoured_in_both_directions(self):
        self.write(
            "\n".join(
                [
                    "nav_key_left=alt+left",
                    "unzoom_on_nav=false",
                    "nav_at_edge=wrap",
                    "most_recently_focused=false",
                ]
            )
        )
        cfg = herdr_common.read_config("nav", "left")
        self.assertEqual(cfg["key"], "alt+left")
        self.assertFalse(cfg["unzoom"])
        self.assertEqual(cfg["nav_at_edge"], "wrap")
        self.assertFalse(cfg["most_recently_focused"])

        self.write(
            "\n".join(
                [
                    "unzoom_on_nav=true",
                    "nav_at_edge=stop",
                    "most_recently_focused=true",
                ]
            )
        )
        cfg = herdr_common.read_config("nav", "left")
        self.assertTrue(cfg["unzoom"])
        self.assertEqual(cfg["nav_at_edge"], "stop")
        self.assertTrue(cfg["most_recently_focused"])

    def test_last_occurrence_wins_and_comments_are_ignored(self):
        self.write(
            "\n".join(
                [
                    "# nav_at_edge=stop",
                    "nav_at_edge=stop",
                    "nav_at_edge = wrap   # later one wins",
                    "most_recently_focused=false",
                    "most_recently_focused=true",
                ]
            )
        )
        cfg = herdr_common.read_config("nav", "left")
        self.assertEqual(cfg["nav_at_edge"], "wrap")
        self.assertTrue(cfg["most_recently_focused"])

    def test_a_generated_file_parses_like_a_hand_written_one(self):
        self.write(
            "\n".join(
                [
                    "# herdr-splits-generated-v1",
                    "# Written because setup({ overwrite_conf = true }): this file is a projection",
                    "# of the Lua options — remove an opt in setup() to revert it.",
                    "unzoom_on_nav=false",
                    "nav_at_edge=wrap",
                    "most_recently_focused=false",
                ]
            )
        )
        cfg = herdr_common.read_config("nav", "left")
        self.assertFalse(cfg["unzoom"])
        self.assertEqual(cfg["nav_at_edge"], "wrap")
        self.assertFalse(cfg["most_recently_focused"])


if __name__ == "__main__":
    unittest.main()
