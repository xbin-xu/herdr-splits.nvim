ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
NVIM ?= nvim
VIM ?= vim
PYTHON ?= python
CARGO ?= cargo

.PHONY: deps test test-vim test-py check-shell build test-rust fmt clippy check

deps:
	@if git -C "$(ROOT)" ls-files --error-unmatch deps/mini.test >/dev/null 2>&1; then \
		git -C "$(ROOT)" submodule update --init --depth 1 --checkout deps/mini.test; \
	else \
		test -f "$(ROOT)/deps/mini.test/lua/mini/test.lua"; \
	fi

test: deps
	cd "$(ROOT)" && "$(NVIM)" --headless --noplugin -i NONE \
		-u "$(ROOT)/scripts/minimal_init.lua" -c "lua MiniTest.run()"

test-vim:
	"$(VIM)" -u NONE -N --not-a-term -S "$(ROOT)/tests/vim/test_title.vim" </dev/null
	@echo "Vim title tests passed"

test-py:
	"$(PYTHON)" -m unittest discover -s "$(ROOT)/tests" -p 'test_*.py'

check-shell:
	bash -n "$(ROOT)/scripts/herdr-common.sh"
	bash -n "$(ROOT)/scripts/herdr-nav.sh"
	bash -n "$(ROOT)/scripts/herdr-resize.sh"
	sh -n "$(ROOT)/scripts/build.sh"

# Build the Herdr-side helper and install it where the manifest expects it.
# `herdr plugin link` does not run the manifest's [[build]] commands, so this is
# the local-development entry point. Release profile is what the actions run.
#
# Delegates to scripts/build.sh so the `--root` handling lives in exactly one
# place: without it, `install` walks up from `current_exe()` and fails whenever
# CARGO_TARGET_DIR puts the binary outside the checkout.
build:
	sh "$(ROOT)/scripts/build.sh"

test-rust:
	@command -v "$(CARGO)" >/dev/null 2>&1 || { \
		echo "cargo not found: skipping Rust tests (install Rust to run them)"; \
		exit 0; \
	}
	"$(CARGO)" test --locked --manifest-path "$(ROOT)/rust/Cargo.toml"

fmt:
	"$(CARGO)" fmt --manifest-path "$(ROOT)/rust/Cargo.toml"

clippy:
	"$(CARGO)" clippy --all-targets --locked --manifest-path "$(ROOT)/rust/Cargo.toml" -- -D warnings

check: check-shell test test-vim test-py test-rust
