#!/usr/bin/env bash
# Herdr navigation helper — used by herdr keybinds for seamless two-way nav.
# Semantics mirror scripts/herdr-nav.py (the python scripts are the fast
# path via direct socket RPC; this is the no-python fallback via the herdr
# CLI). Detection: HERDR_PANE_ID env → pane registration token → Vim title
# marker → process names. The pane-level MRU mode (`most_recently_focused`)
# is python-only:
# the herdr CLI can only focus by direction, while MRU must focus a
# specific pane chosen by the search (done over the socket). This fallback
# keeps Herdr's geometric focus rule.
#
# When a navigation key is pressed in Herdr:
# 1. If the focused pane is running Neovim: forward the key chord into that
#    pane. The Neovim plugin owns in-split movement, edge crossing, and
#    unzoom — do NOT unzoom here, or moving between Neovim splits would
#    incorrectly unzoom the pane.
# 2. Otherwise (a plain Herdr pane): unzoom if needed, then move Herdr pane
#    focus. At a layout edge, focus wraps around to the opposite side,
#    unless `nav_at_edge=stop`.
#
# Usage: herdr-nav.sh <left|down|up|right>

set -euo pipefail

dir="${1:?usage: herdr-nav.sh <left|down|up|right>}"
herdr="${HERDR_BIN_PATH:-herdr}"

case "$dir" in
  left)  key="ctrl+h"; config_key="nav_key_left"; opp="right" ;;
  down)  key="ctrl+j"; config_key="nav_key_down"; opp="up" ;;
  up)    key="ctrl+k"; config_key="nav_key_up"; opp="down" ;;
  right) key="ctrl+l"; config_key="nav_key_right"; opp="left" ;;
  *) echo "herdr-nav.sh: unknown direction: $dir" >&2; exit 2 ;;
esac

# shellcheck source=scripts/herdr-common.sh
. "$(dirname "$0")/herdr-common.sh"

# Resolve the forward chord, `unzoom_on_nav` and `nav_at_edge` from the
# shared config file (defaults: ctrl+hjkl, unzoom enabled, wrap).
configured_key=$(herdr_splits_conf_value "$config_key")
if [ -n "$configured_key" ]; then key="$configured_key"; fi
unzoom=1
if [ "$(herdr_splits_conf_value unzoom_on_nav)" = "false" ]; then unzoom=0; fi
nav_at_edge=$(herdr_splits_conf_value nav_at_edge)

# herdr injects HERDR_PANE_ID into plugin actions (and panes); fall back to
# a pane.current query only when it is missing (manual invocation).
pane_id="${HERDR_PANE_ID:-}"
if [ -z "$pane_id" ]; then
  pane_id=$("$herdr" pane current --current 2>/dev/null \
    | grep -o '"pane_id"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 \
    | sed 's/.*"\([^"]*\)"$/\1/' || true)
fi

# Vim/Neovim pane: forward the chord; the editor plugin decides movement,
# unzoom, and edge crossing.
if herdr_splits_is_vim "$herdr" "$pane_id"; then
  exec "$herdr" pane send-keys "$pane_id" "$key"
fi

# --- Non-vim Herdr pane: unzoom if needed, then focus (wrapping at edges) ---

# `pane edges` reports edge flags plus the zoomed state (inside `layout`).
edges_out=$("$herdr" pane edges --current 2>/dev/null || true)

# A zoomed pane fills the tab and reports itself at every edge, so the edge
# flags are useless for the wrap decision. Unzoom (if enabled) so we can
# trust them. If unzoom is disabled we leave zoom untouched and must NOT use
# the (all-true) edge flags — wrap is skipped below in that case.
edges_trusted=1
if printf '%s' "$edges_out" | grep -q '"zoomed"[[:space:]]*:[[:space:]]*true'; then
  if [ "$unzoom" -eq 1 ]; then
    "$herdr" pane zoom --off --current 2>/dev/null || true
    # Layout changed; re-read edges so the wrap check is accurate.
    edges_out=$("$herdr" pane edges --current 2>/dev/null || true)
  else
    edges_trusted=0
  fi
fi

# Move to the neighbor in the requested direction. When already at the
# requested edge (no neighbor there): wrap to the opposite side, or — when
# nav_at_edge=stop — do nothing. Skip the edge check when the flags are
# unreliable (still zoomed, unzoom disabled).
if [ "$edges_trusted" -eq 1 ] && printf '%s' "$edges_out" | grep -q "\"$dir\"[[:space:]]*:[[:space:]]*true"; then
  if [ "$nav_at_edge" = "stop" ]; then
    exit 0
  fi
  exec "$herdr" pane focus --direction "$opp" --current
else
  exec "$herdr" pane focus --direction "$dir" --current
fi
