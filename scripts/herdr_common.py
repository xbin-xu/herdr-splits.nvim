"""Shared runtime for herdr-splits fast-path scripts (herdr-nav.py /
herdr-resize.py).

Talks to the Herdr server over its local control socket (NDJSON RPC, one
request per connection) instead of spawning `herdr` CLI subprocesses: on
Windows every process spawn costs ~50ms, so a bash script that shells out to
the CLI 4-6 times per keypress lands at 350-500ms. With direct RPC the only
remaining per-key cost is this interpreter's startup (~50ms, ~45ms under
`python -S`). Method names and params follow `herdr api schema --json`
(protocol 22).

Stdlib only — safe to run under `python -S`.
"""

import json
import os
import re
import socket
import sys
from pathlib import Path

# Full-match vim family process names; Windows reports e.g. "nvim.exe".
VIM_PROCESS_RE = re.compile(r"^(g?(view|l?n?vim?x?)(diff)?)(\.exe)?$", re.IGNORECASE)

DIRECTION_OPPOSITE = {"left": "right", "right": "left", "up": "down", "down": "up"}


# ---------------------------------------------------------------------------
# Socket / pipe discovery and transport
# ---------------------------------------------------------------------------


def discover_socket_path() -> str:
    """Resolve the herdr control socket/pipe path.

    1. HERDR_SOCKET_PATH / HERDR_SOCKET (injected into panes and plugin
       actions; authoritative when present)
    2. Windows: %APPDATA%\\herdr\\herdr.sock (native Windows herdr home)
    3. XDG_CONFIG_HOME / ~/.config/herdr/herdr.sock (unix layout)
    """
    explicit = os.environ.get("HERDR_SOCKET_PATH") or os.environ.get("HERDR_SOCKET")
    if explicit:
        return explicit
    if sys.platform == "win32":
        appdata = os.environ.get("APPDATA")
        if appdata:
            p = Path(appdata) / "herdr" / "herdr.sock"
            if p.exists():
                return str(p)
    xdg = os.environ.get("XDG_CONFIG_HOME")
    base = Path(xdg) / "herdr" if xdg else Path.home() / ".config" / "herdr"
    return str(base / "herdr.sock")


class _UnixSocketTransport:
    def __init__(self, sock_path: str):
        self._sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self._sock.settimeout(5.0)
        self._sock.connect(sock_path)

    def sendall(self, data: bytes) -> None:
        self._sock.sendall(data)

    def recv(self, n: int) -> bytes:
        return self._sock.recv(n)

    def close(self) -> None:
        try:
            self._sock.close()
        except OSError:
            pass


class _WindowsPipeTransport:
    """Named pipe transport. herdr derives the pipe name from the socket
    path by prefixing \\\\.\\pipe\\ (herdrdev/herdr#1531)."""

    def __init__(self, pipe_name: str):
        if not pipe_name.startswith("\\\\.\\pipe\\"):
            pipe_name = "\\\\.\\pipe\\" + pipe_name
        self._fh = open(pipe_name, "r+b", buffering=0)

    def sendall(self, data: bytes) -> None:
        self._fh.write(data)

    def recv(self, n: int) -> bytes:
        return self._fh.read(n)

    def close(self) -> None:
        try:
            self._fh.close()
        except OSError:
            pass


def connect_transport(sock_path: str):
    if sys.platform == "win32":
        return _WindowsPipeTransport(sock_path)
    return _UnixSocketTransport(sock_path)


def rpc_call(sock_path: str, method: str, params: dict, req_id: str = "req") -> dict:
    """One request per connection (herdr closes after one reply)."""
    transport = connect_transport(sock_path)
    try:
        payload = json.dumps({"id": req_id, "method": method, "params": params}) + "\n"
        transport.sendall(payload.encode("utf-8"))
        buf = b""
        while b"\n" not in buf:
            chunk = transport.recv(65536)
            if not chunk:
                break
            buf += chunk
        line = buf.split(b"\n", 1)[0].decode("utf-8")
        return json.loads(line)
    finally:
        transport.close()


# ---------------------------------------------------------------------------
# Shared config file (same resolution order as the Neovim side / .sh scripts)
# ---------------------------------------------------------------------------


def resolve_config_path() -> Path:
    explicit = os.environ.get("HERDR_SPLITS_CONFIG")
    if explicit:
        return Path(explicit)
    plugin_dir = os.environ.get("HERDR_PLUGIN_CONFIG_DIR")
    if plugin_dir:
        return Path(plugin_dir) / "herdr-splits.conf"
    if sys.platform == "win32":
        appdata = os.environ.get("APPDATA")
        if appdata:
            return Path(appdata) / "herdr" / "plugins" / "config" / "herdr-splits" / "herdr-splits.conf"
    return Path.home() / ".config" / "herdr" / "plugins" / "config" / "herdr-splits" / "herdr-splits.conf"


NAV_DEFAULT_KEYS = {"left": "ctrl+h", "down": "ctrl+j", "up": "ctrl+k", "right": "ctrl+l"}
RESIZE_DEFAULT_KEYS = {"left": "alt+h", "down": "alt+j", "up": "alt+k", "right": "alt+l"}


def read_config(kind: str, direction: str) -> dict:
    """Read the shared herdr-splits.conf. kind: 'nav' or 'resize'."""
    cfg = {
        "key": (NAV_DEFAULT_KEYS if kind == "nav" else RESIZE_DEFAULT_KEYS)[direction],
        "unzoom": True,
        "nav_at_edge": "wrap",
        "most_recently_focused": False,
    }
    config_path = resolve_config_path()
    try:
        text = config_path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return cfg

    config_key = f"{'nav' if kind == 'nav' else 'resize'}_key_{direction}"
    m = re.findall(rf"^\s*{config_key}\s*=\s*([^\s#]+)", text, re.MULTILINE)
    if m:
        cfg["key"] = m[-1]
    if re.search(r"^\s*unzoom_on_nav\s*=\s*false", text, re.MULTILINE):
        cfg["unzoom"] = False
    if re.search(r"^\s*nav_at_edge\s*=\s*stop", text, re.MULTILINE):
        cfg["nav_at_edge"] = "stop"
    if re.search(r"^\s*most_recently_focused\s*=\s*true", text, re.MULTILINE):
        cfg["most_recently_focused"] = True
    return cfg


# ---------------------------------------------------------------------------
# Pane editor detection (registration token)
# ---------------------------------------------------------------------------


def pane_has_editor(sock_path: str, pane_id: str) -> bool:
    """True when the pane carries the editor registration token.

    The Neovim plugin reports tokens{editor=nvim} via pane.report_metadata
    with a TTL (lua/herdr-splits/register.lua): a stale registration expires
    server-side and a pane that no longer runs Neovim stops matching on its
    own — no files, no pid-liveness probing."""
    if not pane_id:
        return False
    resp = rpc_call(sock_path, "pane.get", {"pane_id": pane_id})
    tokens = resp.get("result", {}).get("pane", {}).get("tokens") or {}
    return tokens.get("editor") == "nvim"


def current_pane_id(sock_path: str) -> str:
    """herdr injects HERDR_PANE_ID into plugin actions (and panes); fall back
    to a pane.current RPC only when it is missing (manual invocation)."""
    env_pane = os.environ.get("HERDR_PANE_ID", "")
    if env_pane:
        return env_pane
    resp = rpc_call(sock_path, "pane.current", {})
    pane = resp.get("result", {}).get("pane", {})
    return pane.get("pane_id", "")


def is_vim_pane(sock_path: str, pane_id: str) -> bool:
    """Registration token first (the editor side knows what it is; process
    names are a guess, and on Windows they cannot see through shims / proxy
    shells at all — no tcgetpgrp equivalent on ConPTY), then foreground
    process names for plugin-less vim/gvim panes."""
    if pane_has_editor(sock_path, pane_id):
        return True
    resp = rpc_call(sock_path, "pane.process_info", {"pane_id": pane_id})
    if "error" in resp:
        return False
    procs = resp.get("result", {}).get("process_info", {}).get("foreground_processes", [])
    return any(VIM_PROCESS_RE.match(p.get("name", "")) for p in procs)


# ---------------------------------------------------------------------------
# Pane selection for most_recently_focused (mru* = internal shorthand;
# tmux select-pane semantics)
# ---------------------------------------------------------------------------


def _pane_layout(sock_path: str):
    """Current tab's layout as (zoomed, {pane_id: (x, y, w, h)}).
    When zoomed, herdr reports only the zoomed pane (full area)."""
    resp = rpc_call(sock_path, "pane.layout", {})
    layout = resp.get("result", {}).get("layout", {})
    rects = {}
    for p in layout.get("panes", []):
        r = p.get("rect", {})
        rects[p["pane_id"]] = (r.get("x", 0), r.get("y", 0), r.get("width", 0), r.get("height", 0))
    return bool(layout.get("zoomed")), rects


def _pane_candidates(rects, cur, direction, wrap):
    """tmux directional search over pane rects: the nearest layer of panes
    beyond the current one with perpendicular overlap; with `wrap`, the
    opposite layout boundary (tmux flips its search across the edge).
    Both passes share one projection of each rect onto the movement axis
    ([s, e] inclusive, plus the perpendicular span), which collapses the
    four-direction matrix to the `horiz`/`fwd` pair. Mirrors nav.lua
    mru_candidates."""
    if cur not in rects:
        return []
    horiz = direction in ("left", "right")
    fwd = direction in ("right", "down")

    def project(r):
        """[s, e] inclusive along the movement axis, [ps, pe] across it."""
        x, y, w, h = r
        if horiz:
            return x, x + w - 1, y, y + h - 1
        return y, y + h - 1, x, x + w - 1

    cs, ce, cps, cpe = project(rects[cur])

    def near(s, e):
        """The edge facing the movement (tmux's edge comparison)."""
        return s if fwd else e

    if wrap:
        # Opposite boundary: right/down wrap to the smallest start
        # coordinate, left/up wrap to the largest stop coordinate.
        boundary = float("inf") if fwd else float("-inf")
        for r in rects.values():
            s, e = project(r)[:2]
            boundary = (min(boundary, near(s, e)) if fwd
                        else max(boundary, near(s, e)))
        out = []
        for pid, r in rects.items():
            if pid == cur:
                continue
            s, e, ps, pe = project(r)
            if near(s, e) == boundary and ps <= cpe and pe >= cps:
                out.append(pid)
        return out

    # Nearest layer beyond the current window: smallest positive gap.
    best, out = None, []
    for pid, r in rects.items():
        if pid == cur:
            continue
        s, e, ps, pe = project(r)
        gap = (s - ce) if fwd else (cs - e)
        if gap >= 1 and ps <= cpe and pe >= cps:
            if best is None or gap < best:
                best, out = gap, [pid]
            elif gap == best:
                out.append(pid)
    return out


def _mru_path(config_path) -> Path:
    """Pane MRU state: a small file next to the shared conf, one pane id per
    line, most-recent-first. Each keypress is a fresh process, so cross-
    invocation history must live on disk (herdr exposes no recency API)."""
    return Path(config_path).parent / "mru-panes"


def _load_pane_mru(path: Path):
    try:
        text = path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return []
    return [line.strip() for line in text.splitlines() if line.strip()]


def _save_pane_mru(path: Path, order) -> None:
    """Best-effort atomic write; MRU state is an optimization, never fatal."""
    tmp = Path(str(path) + ".tmp." + str(os.getpid()))
    try:
        tmp.write_text("\n".join(order) + ("\n" if order else ""), encoding="utf-8")
        os.replace(tmp, path)
    except OSError:
        try:
            tmp.unlink()
        except OSError:
            pass


def mru_focus(sock_path: str, config_path, direction: str, cur_pane: str = None):
    """Focus the most recently focused pane among the candidates adjacent in
    `direction` (tmux select-pane semantics). Returns the focused pane id,
    or None when nothing was found — callers then fall back to Herdr's own
    focus_direction / edge handling.

    Recency is recorded from the plugin's own moves plus the pane hosting
    each keypress (the current pane is bumped on every invocation). Focus
    changes made purely outside the plugin — mouse clicks, Herdr's own
    bindings — become visible at the next keypress inside that pane, so the
    MRU order may lag those by one interaction.

    Zoom: with auto-unzoom enabled the pane is unzoomed first and the search
    runs on the restored layout; with it disabled the layout is unusable and
    the caller falls back to the legacy path.
    """
    cur = cur_pane or os.environ.get("HERDR_PANE_ID", "")
    if not cur:
        return None
    cfg = read_config("nav", direction)

    zoomed, rects = _pane_layout(sock_path)
    if zoomed:
        if not cfg.get("unzoom"):
            return None
        rpc_call(sock_path, "pane.zoom", {"pane_id": cur, "mode": "off"})
        _, rects = _pane_layout(sock_path)

    cands = _pane_candidates(rects, cur, direction, wrap=False)
    if not cands:
        if cfg.get("nav_at_edge") == "stop":
            return None
        cands = _pane_candidates(rects, cur, direction, wrap=True)
    if not cands:
        return None

    path = _mru_path(config_path)
    # Live panes only; the current pane counts as the most recent focus.
    order = [cur] + [p for p in _load_pane_mru(path) if p != cur and p in rects]
    pick = next((p for p in order if p in cands), None)
    if pick is None:
        pick = cands[0]  # no history yet: deterministic (herdr's own order)
    _save_pane_mru(path, [pick] + [p for p in order if p != pick])
    rpc_call(sock_path, "pane.focus", {"pane_id": pick})
    return pick
