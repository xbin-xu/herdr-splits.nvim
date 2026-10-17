#!/usr/bin/env python3
"""Unit tests for the plain-pane fallback in herdr-nav.py (no server).

These pin the zoom/edge decision for panes that do not run an editor.
Herdr keeps reporting the pre-zoom tiling and accurate edge flags while a
tab is zoomed, so the plain-pane path must still wrap at the edge with
auto-unzoom disabled. A collapsed zoomed layout (older servers) keeps the
requested-direction fallback.
"""

import importlib.util
import sys
import unittest
from pathlib import Path
from unittest import mock

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
spec = importlib.util.spec_from_file_location("herdr_nav", SCRIPTS / "herdr-nav.py")
herdr_nav = importlib.util.module_from_spec(spec)
spec.loader.exec_module(herdr_nav)


def edges_response(zoomed, panes):
    return {
        "result": {
            "edges": {
                "left": True,
                "right": False,
                "up": True,
                "down": True,
                "pane_id": "A",
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
        },
    }


class PlainPaneZoomTest(unittest.TestCase):
    def setUp(self):
        self.calls = []
        self.edges = edges_response(True, [("A", 0, 40), ("B", 40, 40)])
        self.cfg = {
            "key": "ctrl+h",
            "unzoom": False,
            "nav_at_edge": "wrap",
            "most_recently_focused": False,
        }
        self.argv = ["herdr-nav.py", "left"]
        for patcher in (
            mock.patch.object(herdr_nav, "resolve_config_path", return_value=Path("conf")),
            mock.patch.object(herdr_nav, "discover_socket_path", return_value="sock"),
            mock.patch.object(herdr_nav, "current_pane_id", return_value="A"),
            mock.patch.object(herdr_nav, "is_vim_pane", return_value=False),
            mock.patch.object(herdr_nav, "mru_focus", return_value=None),
            mock.patch.object(herdr_nav, "read_config", side_effect=lambda *a: self.cfg),
            mock.patch.object(herdr_nav, "rpc_call", side_effect=self._rpc),
            mock.patch.object(sys, "argv", self.argv),
        ):
            patcher.start()
            self.addCleanup(patcher.stop)

    def _rpc(self, sock_path, method, params, req_id="req"):
        self.calls.append((method, params))
        if method == "pane.edges":
            return self.edges
        if method == "pane.focus_direction":
            return {"result": {}}
        raise AssertionError(f"unexpected RPC: {method}")

    def test_zoomed_pane_wraps_at_the_edge(self):
        exit_code = herdr_nav.main()

        self.assertEqual(exit_code, 0)
        self.assertEqual(
            self.calls,
            [
                ("pane.edges", {"pane_id": "A"}),
                ("pane.focus_direction", {"pane_id": "A", "direction": "right"}),
            ],
        )

    def test_collapsed_zoomed_layout_keeps_requested_direction(self):
        self.edges = edges_response(True, [("A", 0, 80)])

        exit_code = herdr_nav.main()

        self.assertEqual(exit_code, 0)
        self.assertEqual(
            self.calls,
            [
                ("pane.edges", {"pane_id": "A"}),
                ("pane.focus_direction", {"pane_id": "A", "direction": "left"}),
            ],
        )


if __name__ == "__main__":
    unittest.main()
