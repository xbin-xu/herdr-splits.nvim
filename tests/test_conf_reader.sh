#!/usr/bin/env bash
# Reader parity for the shared conf on the bash fallback side — no herdr, no pty.
#
#   bash tests/test_conf_reader.sh
#
# The bash reader (`scripts/herdr-nav.sh`) is the fifth participant, and the one
# that used to escape the symmetry checks: it defaulted to `unzoom=1` and only
# recognised `unzoom_on_nav=false`, so after the defaults flipped it unzoomed
# where every other reader kept the zoom. These cases pin the same semantics the
# Rust `conf_value`, Lua `read_managed`, Python `read_config` and Vim
# `s:conf_value` implement: comments, last value wins, empty means unset,
# booleans in either direction and in any case.

set -uo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
# shellcheck source=scripts/herdr-common.sh
. "$root/scripts/herdr-common.sh"

conf=$(mktemp "${TMPDIR:-/tmp}/herdr-splits-conf.XXXXXX")
trap 'rm -f "$conf"' EXIT
export HERDR_SPLITS_CONFIG="$conf"

failures=0

check() {
  local label=$1 want=$2 got=$3
  if [ "$want" != "$got" ]; then
    echo "FAIL: $label: got '$got', want '$want'" >&2
    failures=$((failures + 1))
  fi
}

# `configured_unzoom` mirrors what herdr-nav.sh computes: 1 = unzoom, 0 = keep.
configured_unzoom() {
  local value
  value=$(herdr_splits_conf_value unzoom_on_nav | tr '[:upper:]' '[:lower:]')
  if [ "$value" = "true" ]; then echo 1; else echo 0; fi
}

configured_edge() {
  local value
  # Mirrors the script exactly: the enum is compared verbatim and anything that
  # is not `stop` wraps — including an absent key, since the shipped default is
  # `wrap`. Same rule as Rust (`value != "stop"`), Python and Lua.
  value=$(herdr_splits_conf_value nav_at_edge)
  if [ "$value" = "stop" ]; then echo stop; else echo wrap; fi
}

# --- missing file: the shipped defaults -----------------------------------
: >"$conf"
check "missing unzoom_on_nav -> keep the zoom (default)" 0 "$(configured_unzoom)"
check "missing nav_at_edge -> wrap (default)" wrap "$(configured_edge)"

# --- both directions, and case-insensitive --------------------------------
printf 'unzoom_on_nav=false\n' >"$conf"
check "unzoom_on_nav=false" 0 "$(configured_unzoom)"
printf 'unzoom_on_nav=true\n' >"$conf"
check "unzoom_on_nav=true" 1 "$(configured_unzoom)"
printf 'unzoom_on_nav=FALSE\n' >"$conf"
check "unzoom_on_nav=FALSE" 0 "$(configured_unzoom)"
printf 'unzoom_on_nav=True\n' >"$conf"
check "unzoom_on_nav=True" 1 "$(configured_unzoom)"

printf 'nav_at_edge=wrap\n' >"$conf"
check "nav_at_edge=wrap" wrap "$(configured_edge)"
# The enum is case-sensitive in every reader (Rust `value != "stop"`, Python
# `== "stop"`, Lua `== 'stop'`): only an exact `stop` stops.
printf 'nav_at_edge=STOP\n' >"$conf"
check "nav_at_edge=STOP is not stop -> wrap" wrap "$(configured_edge)"

# --- comments and last value wins ----------------------------------------
printf '# unzoom_on_nav=true\nunzoom_on_nav=true\nunzoom_on_nav=false\n' >"$conf"
check "last value wins" 0 "$(configured_unzoom)"
printf '# nav_at_edge=wrap\n' >"$conf"
check "commented key ignored -> default" wrap "$(configured_edge)"

# --- a generated file parses the same ------------------------------------
printf '# herdr-splits-generated-v1\nunzoom_on_nav=true\nnav_at_edge=wrap\n' >"$conf"
check "generated file unzoom" 1 "$(configured_unzoom)"
check "generated file edge" wrap "$(configured_edge)"

if [ "$failures" -ne 0 ]; then
  echo "test_conf_reader.sh: $failures failure(s)" >&2
  exit 1
fi
echo "bash conf reader: OK"
