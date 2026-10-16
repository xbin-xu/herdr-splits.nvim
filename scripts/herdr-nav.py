#!/usr/bin/env python3
"""herdr-nav.py — herdr navigation helper (herdr-splits fast path).

Talks to the herdr control socket directly over RPC (no herdr CLI
subprocesses). Detection order: the pane registration token first (reported
by the editor-side plugin, see lua/herdr-splits/register.lua), then
foreground process names as the fallback (bare vim without the plugin).
Same usage and semantics as herdr-nav.sh, the only difference being speed.

Usage: herdr-nav.py <left|down|up|right>
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from herdr_common import (  # noqa: E402
    DIRECTION_OPPOSITE,
    current_pane_id,
    discover_socket_path,
    is_vim_pane,
    read_config,
    rpc_call,
)


def main() -> int:
    if len(sys.argv) != 2 or sys.argv[1] not in DIRECTION_OPPOSITE:
        print("usage: herdr-nav.py <left|down|up|right>", file=sys.stderr)
        return 2
    direction = sys.argv[1]
    opposite = DIRECTION_OPPOSITE[direction]
    cfg = read_config("nav", direction)
    sock_path = discover_socket_path()

    try:
        pane_id = current_pane_id(sock_path)
    except OSError as e:
        print(f"herdr-nav: cannot connect to herdr socket {sock_path}: {e}", file=sys.stderr)
        return 1
    if not pane_id:
        print("herdr-nav: could not determine current pane", file=sys.stderr)
        return 1

    # Vim pane: forward the chord; the Neovim plugin owns movement + unzoom.
    if is_vim_pane(sock_path, pane_id):
        rpc_call(sock_path, "pane.send_keys", {"pane_id": pane_id, "keys": [cfg["key"]]})
        return 0

    # Plain pane: unzoom if needed, then focus (wrapping at edges).
    try:
        resp = rpc_call(sock_path, "pane.edges", {"pane_id": pane_id})
    except OSError as e:
        print(f"herdr-nav: socket error on pane.edges: {e}", file=sys.stderr)
        return 1

    def edges_of(r):
        return r.get("result", {}).get("edges", {})

    edges = edges_of(resp)
    zoomed = edges.get("layout", {}).get("zoomed", False)  # zoomed lives in edges.layout
    edges_trusted = True
    if zoomed:
        if cfg["unzoom"]:
            rpc_call(sock_path, "pane.zoom", {"pane_id": pane_id, "mode": "off"})
            edges = edges_of(rpc_call(sock_path, "pane.edges", {"pane_id": pane_id}))
        else:
            edges_trusted = False  # zoomed pane reports itself at every edge

    if edges_trusted and edges.get(direction):
        if cfg["nav_at_edge"] == "stop":
            return 0
        rpc_call(sock_path, "pane.focus_direction", {"pane_id": pane_id, "direction": opposite})
    else:
        rpc_call(sock_path, "pane.focus_direction", {"pane_id": pane_id, "direction": direction})
    return 0


if __name__ == "__main__":
    sys.exit(main())
