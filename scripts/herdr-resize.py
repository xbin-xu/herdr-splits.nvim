#!/usr/bin/env python3
"""herdr-resize.py — herdr resize helper (herdr-splits fast path).

Same detection as herdr-nav.py (registration token first, process names as
fallback): vim panes get the chord forwarded (Neovim decides between
resizing a split and handing off to herdr), plain panes are resized via
`pane.resize` directly. Zoom state is left untouched (matching smart-splits'
disable_multiplexer_nav_when_zoomed default). Same semantics as
herdr-resize.sh, the only difference being speed.

Usage: herdr-resize.py <left|down|up|right> [amount]
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from herdr_common import (  # noqa: E402
    current_pane_id,
    discover_socket_path,
    is_vim_pane,
    read_config,
    rpc_call,
)


def main() -> int:
    if len(sys.argv) not in (2, 3) or sys.argv[1] not in ("left", "down", "up", "right"):
        print("usage: herdr-resize.py <left|down|up|right> [amount]", file=sys.stderr)
        return 2
    direction = sys.argv[1]
    amount = float(sys.argv[2]) if len(sys.argv) == 3 else 0.03

    cfg = read_config("resize", direction)
    sock_path = discover_socket_path()

    try:
        pane_id = current_pane_id(sock_path)
    except OSError as e:
        print(f"herdr-resize: cannot connect to herdr socket {sock_path}: {e}", file=sys.stderr)
        return 1
    if not pane_id:
        print("herdr-resize: could not determine current pane", file=sys.stderr)
        return 1

    if is_vim_pane(sock_path, pane_id):
        rpc_call(sock_path, "pane.send_keys", {"pane_id": pane_id, "keys": [cfg["key"]]})
    else:
        rpc_call(sock_path, "pane.resize", {"pane_id": pane_id, "direction": direction, "amount": amount})
    return 0


if __name__ == "__main__":
    sys.exit(main())
