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
	# cargo is often installed but missing from PATH: rustup puts its proxies in
	# ~/.cargo/bin and leaves editing PATH to the installer, while a process
	# started before that edit (the herdr server, a desktop-launched Neovim)
	# still has the old environment. Look in the standard places before failing.
	for candidate in \
		"${CARGO:-}" \
		"${CARGO_HOME:-$HOME/.cargo}/bin/cargo" \
		"${CARGO_HOME:-$HOME/.cargo}/bin/cargo.exe" \
		"$HOME/.cargo/bin/cargo.exe"
	do
		if [ -n "$candidate" ] && [ -x "$candidate" ]; then
			cargo=$candidate
			break
		fi
	done
	# Last resort: a toolchain directory. Not a rustup proxy, but a working
	# cargo, which is all this script needs.
	if [ -z "${cargo:-}" ]; then
		for candidate in "$HOME"/.rustup/toolchains/*/bin/cargo "$HOME"/.rustup/toolchains/*/bin/cargo.exe
		do
			if [ -x "$candidate" ]; then
				cargo=$candidate
				break
			fi
		done
	fi
	if [ -z "${cargo:-}" ]; then
		echo "scripts/build.sh: cargo not found." >&2
		echo "  Install Rust from https://rustup.rs, or run this from a shell where" >&2
		echo "  'cargo --version' works, or set CARGO=/path/to/cargo." >&2
		exit 1
	fi
else
	cargo=cargo
fi

echo "building ($profile)..."
# shellcheck disable=SC2086 # profile_args is a deliberate flag list
"$cargo" build $profile_args --manifest-path "$root/rust/Cargo.toml"
# `--root` is explicit on purpose: with CARGO_TARGET_DIR set, the binary lives
# outside the checkout and could not find herdr-plugin.toml by walking up.
# shellcheck disable=SC2086
"$cargo" run $profile_args --quiet --manifest-path "$root/rust/Cargo.toml" -- install --root "$root"

echo "installed $root/scripts/bin/herdr-splits"
