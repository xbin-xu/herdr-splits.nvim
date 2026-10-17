#!/usr/bin/env bash
# Herdr resize helper — used by herdr keybinds for seamless two-way resize.
# Semantics mirror scripts/herdr-resize.py (python = fast path via direct
# socket RPC; this is the no-python fallback via the herdr CLI). Detection
# order: HERDR_PANE_ID env → pane registration token → nvim process names.
#
# When a resize key (default alt+h/j/k/l) is pressed in Herdr:
# 1. If the focused pane is running Neovim: forward the key chord into that
#    pane (Neovim's plugin handles it).
# 2. Otherwise (including plain Vim, which has no resize mappings): resize
#    the Herdr pane directly.
#
# Zoom state is left untouched (matches smart-splits.nvim's
# disable_multiplexer_nav_when_zoomed default).
#
# Usage: herdr-resize.sh <left|down|up|right> [amount]

set -euo pipefail

dir="${1:?usage: herdr-resize.sh <left|down|up|right> [amount]}"
amount="${2:-0.03}"
herdr="${HERDR_BIN_PATH:-herdr}"

case "$dir" in
  left)  key="alt+h"; config_key="resize_key_left" ;;
  down)  key="alt+j"; config_key="resize_key_down" ;;
  up)    key="alt+k"; config_key="resize_key_up" ;;
  right) key="alt+l"; config_key="resize_key_right" ;;
  *) echo "herdr-resize.sh: unknown direction: $dir" >&2; exit 2 ;;
esac

# shellcheck source=scripts/herdr-common.sh
. "$(dirname "$0")/herdr-common.sh"

# Resolve the forward chord from the shared config (unset keeps alt+hjkl).
configured_key=$(herdr_splits_conf_value "$config_key")
if [ -n "$configured_key" ]; then key="$configured_key"; fi

# herdr injects HERDR_PANE_ID into plugin actions; fall back to pane.current.
pane_id="${HERDR_PANE_ID:-}"
if [ -z "$pane_id" ]; then
  pane_id=$("$herdr" pane current --current 2>/dev/null \
    | grep -o '"pane_id"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 \
    | sed 's/.*"\([^"]*\)"$/\1/' || true)
fi

if herdr_splits_is_nvim "$herdr" "$pane_id"; then
  exec "$herdr" pane send-keys "$pane_id" "$key"
fi

exec "$herdr" pane resize --direction "$dir" --amount "$amount" --current
