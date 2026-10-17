ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
NVIM ?= nvim
VIM ?= vim
PYTHON ?= python

.PHONY: deps test test-vim test-py check-shell check

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

check: check-shell test test-vim test-py
