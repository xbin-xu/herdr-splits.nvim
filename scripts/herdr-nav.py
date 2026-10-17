#!/usr/bin/env python3
"""herdr-nav.py — herdr navigation helper (herdr-splits fast path).

Talks to the herdr control socket directly over RPC (no herdr CLI
subprocesses). Detection order: the pane registration token first (reported
by the Neovim plugin, see lua/herdr-splits/register.lua), then the Vim
plugin's terminal-title marker (plain Vim has no RPC transport for a token),
then foreground process names for plugin-less editors. Same usage and
semantics as herdr-nav.sh, the only difference being speed (the `.sh`
fallback does not support pane-level most_recently_focused).

--pane-only: called when Neovim crosses out at its edge; performs pane-level
selection only and never forwards chords into panes (that would loop back
into Neovim itself).

Usage: herdr-nav.py [--pane-only] <left|down|up|right>
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from herdr_common import (  # noqa: E402
    DIRECTION_OPPOSITE,
    current_pane_id,
    discover_socket_path,
    is_vim_pane,
    layout_is_tiled,
    mru_focus,
    read_config,
    resolve_config_path,
    rpc_call,
)


def try_mru(sock_path: str, config_path: Path, cfg: dict, direction: str, pane_id: str) -> bool:
    """tmux-style pane move when most_recently_focused is on. False hands
    the caller back to Herdr's own focus_direction / edge handling (option
    off, collapsed zoomed layout, socket error)."""
    if not cfg["most_recently_focused"]:
        return False
    try:
        return mru_focus(sock_path, config_path, direction, cur_pane=pane_id) is not None
    except OSError:
        return False


def main() -> int:
    args = sys.argv[1:]
    pane_only = "--pane-only" in args
    args = [a for a in args if a != "--pane-only"]
    if len(args) != 1 or args[0] not in DIRECTION_OPPOSITE:
        print("usage: herdr-nav.py [--pane-only] <left|down|up|right>", file=sys.stderr)
        return 2
    direction = args[0]
    opposite = DIRECTION_OPPOSITE[direction]
    cfg = read_config("nav", direction)
    config_path = resolve_config_path()
    sock_path = discover_socket_path()

    try:
        pane_id = current_pane_id(sock_path)
    except OSError as e:
        print(f"herdr-nav: cannot connect to herdr socket {sock_path}: {e}", file=sys.stderr)
        return 1
    if not pane_id:
        print("herdr-nav: could not determine current pane", file=sys.stderr)
        return 1

    # --pane-only (Neovim crossing out at its edge): pane-level selection
    # only — never forward chords into panes, or the call would loop back
    # into Neovim. Kept for plugin versions that predate the Lua RPC client
    # (current Neovim-side code performs this selection in-process).
    if pane_only:
        if try_mru(sock_path, config_path, cfg, direction, pane_id):
            return 0
        rpc_call(sock_path, "pane.focus_direction", {"pane_id": pane_id, "direction": direction})
        return 0

    # Vim pane: forward the chord; the Neovim plugin owns movement + unzoom.
    if is_vim_pane(sock_path, pane_id):
        rpc_call(sock_path, "pane.send_keys", {"pane_id": pane_id, "keys": [cfg["key"]]})
        return 0

    # Plain pane: tmux-style MRU first when opted in; the legacy geometry
    # below is the fall-through when the search found nothing (edges,
    # collapsed zoomed layout, single pane).
    if try_mru(sock_path, config_path, cfg, direction, pane_id):
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
            # Current Herdr keeps the pre-zoom tiling and accurate edge
            # flags while zoomed. Only a collapsed layout (older servers:
            # the zoomed pane alone, every edge reported reached) cannot
            # drive a wrap.
            panes = edges.get("layout", {}).get("panes", [])
            rects = {
                p.get("pane_id"): tuple(
                    p.get("rect", {}).get(k, 0) for k in ("x", "y", "width", "height")
                )
                for p in panes
            }
            edges_trusted = layout_is_tiled(rects)

    if edges_trusted and edges.get(direction):
        if cfg["nav_at_edge"] == "stop":
            return 0
        rpc_call(sock_path, "pane.focus_direction", {"pane_id": pane_id, "direction": opposite})
    else:
        rpc_call(sock_path, "pane.focus_direction", {"pane_id": pane_id, "direction": direction})
    return 0


if __name__ == "__main__":
    sys.exit(main())
