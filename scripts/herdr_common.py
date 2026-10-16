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
