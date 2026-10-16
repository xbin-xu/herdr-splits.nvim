#!/usr/bin/env bash
# herdr-common.sh — shared helpers for herdr-nav.sh / herdr-resize.sh.
#
# Bash port of scripts/herdr_common.py: same detection order and semantics
# (HERDR_PANE_ID first, pane-registration token before process-name
# guessing). The python scripts are the fast path (direct socket RPC, no
# CLI subprocess spawns); these remain the no-python fallback and speak to
# herdr through its CLI. Sourced, not executed.

# Resolve the shared config file path (same precedence as the lua/python
# sides: HERDR_SPLITS_CONFIG, then HERDR_PLUGIN_CONFIG_DIR, then default).
herdr_splits_conf() {
  printf '%s' "${HERDR_SPLITS_CONFIG:-${HERDR_PLUGIN_CONFIG_DIR:-$HOME/.config/herdr/plugins/config/herdr-splits}/herdr-splits.conf}"
}

# Print the last value of <key> from the shared conf (empty when unset).
# Always exits 0 — an unset key is not an error (callers run under set -e).
herdr_splits_conf_value() {
  { sed -n -E "s/^[[:space:]]*$1[[:space:]]*=[[:space:]]*([^[:space:]#]+).*$/\\1/p" "$(herdr_splits_conf)" 2>/dev/null || true; } | tail -n 1
  return 0
}

# herdr_splits_is_vim <herdr_bin> <pane_id>
# Registration token first (the editor reports editor=nvim via
# pane.report_metadata with a TTL, so stale registrations expire on the
# server — the editor side knows what it is, and on Windows process names
# cannot see through shims/proxy shells at all), then foreground process
# names for plugin-less vim/gvim panes.
herdr_splits_is_vim() {
  [ -n "$2" ] || return 1
  if "$1" pane get "$2" 2>/dev/null \
      | grep -qE '"tokens"[^}]*"editor"[[:space:]]*:[[:space:]]*"nvim"'; then
    return 0
  fi
  local info
  info=$("$1" pane process-info --pane "$2" 2>/dev/null) || return 1
  printf '%s' "$info" | grep -qiE '"name"[[:space:]]*:[[:space:]]*"(g?(view|l?n?vim?x?)(diff)?)(\.exe)?"'
}
