#!/usr/bin/env python3
"""Unit tests for pane_editor detection (no Herdr server required).

The transport is mocked; these assert the decision order and the signals that
make a pane "an editor": nvim's registration token, the Vim plugin's title
marker, then foreground process names for plugin-less editors.
"""

import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import herdr_common  # noqa: E402


class PaneEditorTest(unittest.TestCase):
    def setUp(self):
        self.pane = {}
        self.procs = []
        self.calls = []
        self._patch = mock.patch.object(herdr_common, "rpc_call", side_effect=self._rpc)
        self._patch.start()
        self.addCleanup(self._patch.stop)

    def _rpc(self, sock_path, method, params, req_id="req"):
        self.calls.append(method)
        if method == "pane.get":
            return {"result": {"pane": self.pane}}
        if method == "pane.process_info":
            return {"result": {"process_info": {"foreground_processes": self.procs}}}
        raise AssertionError(f"unexpected RPC: {method}")

    def check(self, pane, procs, want):
        self.pane = pane
        self.procs = procs
        self.calls.clear()
        got = herdr_common.pane_editor("sock", "w1:p1")
        self.assertEqual(got, want)
        return list(self.calls)

    def test_token_identifies_nvim_without_further_queries(self):
        calls = self.check({"tokens": {"editor": "nvim"}}, [], "nvim")
        self.assertEqual(calls, ["pane.get"])

    def test_title_marker_identifies_vim_without_further_queries(self):
        calls = self.check(
            {"terminal_title_stripped": "foo.txt - VIM [herdr-splits]"}, [], "vim"
        )
        self.assertEqual(calls, ["pane.get"])

    def test_marker_anywhere_in_title(self):
        self.check({"terminal_title_stripped": "x [herdr-splits] y"}, [], "vim")

    def test_process_names_are_the_last_fallback(self):
        calls = self.check({}, [{"name": "nvim.exe"}], "nvim")
        self.assertEqual(calls, ["pane.get", "pane.process_info"])
        self.check({}, [{"name": "vim.exe"}], "vim")
        self.check({}, [{"name": "gvim.exe"}], "vim")
        self.check({}, [{"name": "bash.exe"}], None)

    def test_empty_pane_id_short_circuits(self):
        self.assertEqual(herdr_common.pane_editor("sock", ""), None)
        self.assertEqual(self.calls, [])

    def test_pane_get_error_falls_through_to_process_names(self):
        with mock.patch.object(
            herdr_common,
            "rpc_call",
            side_effect=[
                {"error": {"code": "pane_not_found"}},
                {"result": {"process_info": {"foreground_processes": [{"name": "vim"}]}}},
            ],
        ):
            self.assertEqual(herdr_common.pane_editor("sock", "w1:p1"), "vim")

    def test_is_vim_pane_wraps_kind(self):
        self.pane = {"terminal_title_stripped": "[herdr-splits]"}
        self.assertTrue(herdr_common.is_vim_pane("sock", "w1:p1"))
        self.pane = {}
        self.assertFalse(herdr_common.is_vim_pane("sock", "w1:p1"))


if __name__ == "__main__":
    unittest.main()
