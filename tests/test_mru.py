#!/usr/bin/env python3
"""Unit tests for pane-level most_recently_focused selection (no server).

The control socket is mocked; these pin the zoom behaviour. Herdr keeps
reporting the pre-zoom tiling while a tab is zoomed, so the directional
search must keep working with auto-unzoom disabled. Older servers collapse
the layout to the zoomed pane's full-area rectangle; that layout is not
searchable and callers fall back to Herdr's geometric focus.
"""

import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import herdr_common  # noqa: E402


class MruZoomTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.conf = Path(self.tmp.name) / "herdr-splits.conf"
        self.calls = []
        self.layouts = []
        self.cfg = {
            "key": "ctrl+h",
            "unzoom": False,
            "nav_at_edge": "wrap",
            "most_recently_focused": True,
        }
        for patcher in (
            mock.patch.object(herdr_common, "rpc_call", side_effect=self._rpc),
            mock.patch.object(herdr_common, "read_config", side_effect=lambda *a: self.cfg),
        ):
            patcher.start()
            self.addCleanup(patcher.stop)

    def _rpc(self, sock_path, method, params, req_id="req"):
        self.calls.append((method, params))
        if method == "pane.layout":
            return self.layouts.pop(0)
        return {"result": {}}

    @staticmethod
    def layout(zoomed, *panes):
        return {
            "result": {
                "layout": {
                    "zoomed": zoomed,
                    "panes": [
                        {
                            "pane_id": pane_id,
                            "rect": {"x": x, "y": 0, "width": width, "height": 20},
                        }
                        for pane_id, x, width in panes
                    ],
                },
            },
        }

    def test_pre_zoom_layout_wraps_when_unzoom_is_disabled(self):
        self.layouts = [self.layout(True, ("A", 0, 40), ("B", 40, 40))]

        got = herdr_common.mru_focus("sock", self.conf, "left", cur_pane="A")

        self.assertEqual(got, "B")
        self.assertEqual(
            self.calls,
            [
                ("pane.layout", {}),
                ("pane.focus", {"pane_id": "B"}),
            ],
        )

    def test_collapsed_zoomed_layout_falls_back(self):
        self.layouts = [self.layout(True, ("A", 0, 80))]

        got = herdr_common.mru_focus("sock", self.conf, "left", cur_pane="A")

        self.assertIsNone(got)
        self.assertEqual(self.calls, [("pane.layout", {})])

    def test_unzoom_enabled_restores_the_layout_before_searching(self):
        self.cfg["unzoom"] = True
        self.layouts = [
            self.layout(True, ("A", 0, 80)),
            self.layout(False, ("A", 0, 40), ("B", 40, 40)),
        ]

        got = herdr_common.mru_focus("sock", self.conf, "left", cur_pane="A")

        self.assertEqual(got, "B")
        self.assertEqual(
            self.calls,
            [
                ("pane.layout", {}),
                ("pane.zoom", {"pane_id": "A", "mode": "off"}),
                ("pane.layout", {}),
                ("pane.focus", {"pane_id": "B"}),
            ],
        )


if __name__ == "__main__":
    unittest.main()
