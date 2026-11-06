-- herdr-splits.nvim plugin bootstrap.
-- This file is loaded by Neovim when the plugin is found on the runtimepath.
-- It defers all work to `require('herdr-splits')`.

if vim.g.loaded_herdr_splits then
  return
end
vim.g.loaded_herdr_splits = true

-- Defer loading until user calls setup() or invokes a function.
-- The main module is available as require('herdr-splits').

-- Re-read herdr-splits.conf without restarting Neovim. Neovim reads the
-- instance policy once (in setup()), so an edited conf needs this command (or
-- a restart) before the new values take effect; the Herdr-side helper reads it
-- on every keypress. Chords are refreshed in memory, but terminal-mode maps are
-- installed per buffer — reopen terminals (or restart) for a changed chord.
vim.api.nvim_create_user_command('HerdrSplitsReload', function()
  local ok, report = pcall(function()
    return require('herdr-splits').reload()
  end)
  if not ok then
    vim.notify('herdr-splits: reload failed: ' .. tostring(report), vim.log.levels.ERROR)
    return
  end
  local parts = { 'conf: ' .. report.path }
  parts[#parts + 1] = report.policy_changed and 'policy reloaded' or 'policy unchanged'
  if report.chords_changed then
    parts[#parts + 1] = 'chords changed — reopen terminals or restart Neovim'
  end
  vim.notify('herdr-splits: ' .. table.concat(parts, '; '), vim.log.levels.INFO)
end, { desc = 'Re-read herdr-splits.conf (instance policy) without restarting' })
