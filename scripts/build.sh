#!/bin/sh
# Build the Herdr-side helper and install it into scripts/bin/herdr-splits.
#
# `herdr plugin link` does not run the manifest's [[build]] commands, so local
# development needs this. The installed file deliberately has no extension on
# any platform: herdr resolves the relative command against the plugin root and
# Windows then runs that exact path without appending PATHEXT variants. On
# Windows `install` also writes an identical .exe mirror for shells.
#
# Plain POSIX sh (no bash-isms): this is also what `make build` runs.
# Usage: scripts/build.sh [--debug]
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
if [ "${1:-}" = "--debug" ]; then
	profile_args="--locked"
	profile="debug"
else
	profile_args="--release --locked"
	profile="release"
fi

if ! command -v cargo >/dev/null 2>&1; then
	echo "scripts/build.sh: cargo not found (install Rust from https://rustup.rs)" >&2
	exit 1
fi

echo "building ($profile)..."
# shellcheck disable=SC2086 # profile_args is a deliberate flag list
cargo build $profile_args --manifest-path "$root/rust/Cargo.toml"
# `--root` is explicit on purpose: with CARGO_TARGET_DIR set, the binary lives
# outside the checkout and could not find herdr-plugin.toml by walking up.
# shellcheck disable=SC2086
cargo run $profile_args --quiet --manifest-path "$root/rust/Cargo.toml" -- install --root "$root"

echo "installed $root/scripts/bin/herdr-splits"
