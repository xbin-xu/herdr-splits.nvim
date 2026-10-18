---:checkhealth herdr-splits integration.
---Discovered automatically by Neovim's built-in `:checkhealth` via the
---runtimefile lookup in `lua/<plugin>/health.lua`.
---Requires Neovim ≥ 0.10 for `vim.health`.

local M = {}

function M.check()
  local config = require('herdr-splits.config')
  local herdr = require('herdr-splits.herdr')
  local win = require('herdr-splits.win')

  vim.health.start('herdr-splits')

  if herdr.is_in_session() then
    vim.health.ok(
      string.format(
        'Running inside a Herdr session (HERDR_ENV=1, HERDR_PANE_ID=%s)',
        tostring(vim.env.HERDR_PANE_ID)
      )
    )
  else
    vim.health.warn(
      'Not inside a Herdr session (HERDR_ENV='
        .. tostring(vim.env.HERDR_ENV)
        .. ') — nav/resize will not cross pane boundaries.'
    )
  end

  -- The Herdr-side actions run scripts/bin/herdr-splits (built from rust/).
  -- Without it the manifest's commands fail, which looks like "keys stopped
  -- working" rather than a missing build.
  local paths = require('herdr-splits.paths')
  local helper = paths.root() .. '/scripts/bin/herdr-splits'
  if vim.fn.executable(helper) == 1 then
    local version = vim.fn.system({ helper, '--version' })
    vim.health.ok(
      string.format('Herdr-side helper present: %s', (vim.trim(version or ''))))
  else
    vim.health.error(
      'Herdr-side helper missing at '
        .. helper
        .. ' — build it with `cargo build --release --manifest-path rust/Cargo.toml`'
        .. ' then `cargo run --release --manifest-path rust/Cargo.toml -- install`'
        .. ' (or reinstall the plugin so its build commands run)')
  end

  -- One pane.get roundtrip proves the RPC transport and shows whether the
  -- registration token went out — the two failure modes of detection.
  if herdr.is_in_session() then
    local rpc = require('herdr-splits.rpc')
    local resp = rpc.call('pane.get', { pane_id = vim.env.HERDR_PANE_ID })
    if not resp or resp.error then
      vim.health.error('Herdr RPC unreachable (check HERDR_SOCKET_PATH) — nav/resize will not cross panes')
    else
      local pane = resp.result and resp.result.pane or {}
      local tokens = pane.tokens or {}
      if tokens.editor == 'nvim' then
        vim.health.ok('Pane registration token present (editor=nvim)')
      else
        vim.health.warn('No editor=nvim token — Herdr-side detection falls back to process names')
      end
    end
  end

  vim.health.info(string.format('ignored_buftypes: %s', vim.inspect(config.ignored_buftypes)))
  vim.health.info(string.format('ignored_filetypes: %s', vim.inspect(config.ignored_filetypes)))
  vim.health.info(string.format('floating_zindex_max: %s', tostring(config.floating_zindex_max)))
  vim.health.info(string.format('ignore_previewwindows: %s', tostring(config.ignore_previewwindows)))

  if win.is_embedded_floating_window() then
    vim.health.warn('Current window is an embedded floating window (zindex < 50) — treat as sidebar.')
  end
  if win.is_ignored_win() then
    vim.health.info('Current window matches an ignored filetype/buftype.')
  end
  if win.is_floating() and not win.is_embedded_floating_window() then
    vim.health.info('Current window is a true floating popup — nav/resize will forward to Herdr.')
  end
end

return M
