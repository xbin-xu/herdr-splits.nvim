---Terminal-mode navigation — hand <C-hjkl> to herdr-splits from inside a pty
---terminal buffer.
---
---Why buffer-local: terminal UIs and editor distros install their own
---*buffer-local* t-mode maps (LazyVim/snacks map `nav_h/j/k/l` in
---plugins/util.lua, toggleterm and friends do the same), and Neovim resolves
---buffer-local before global — a global t map would never fire. LazyVim's maps
---only run `wincmd`, so at the Neovim edge nothing happens and the chord never
---reaches Herdr: the cursor is stuck inside the terminal split. We therefore
---re-map at the buffer level, one tick after `TermOpen` (which also fires after
---such plugins have mapped their window, so ours is the map that wins).
---
---Plain Vim has had the equivalent since plugin/herdr-splits.vim (tnoremap);
---this closes the Neovim-side gap.
---
---Opt-in: `setup({ terminal_nav = ... })`.
local config = require('herdr-splits.config')

local M = {}

---wincmd key per direction — the fallback when herdr-splits itself fails.
local WINC = { left = 'h', down = 'j', up = 'k', right = 'l' }

---Right-hand side for one t-mode chord.
---
---Floats keep the chord: it belongs to the program inside (htop, fzf, ssh, ...),
---which is what LazyVim does with its own `self:is_floating()` branch.
---@param direction '"left"'|'"right"'|'"up"'|'"down"'
---@param lhs string the chord as bound; fed back verbatim for floats
---@return string|nil chord to feed back, nil when the move was scheduled
function M.nav(direction, lhs)
  if vim.api.nvim_win_get_config(0).relative ~= '' then
    return lhs
  end
  vim.schedule(function()
    local ok = pcall(function()
      require('herdr-splits')['move_cursor_' .. direction]()
    end)
    if not ok then
      vim.cmd.wincmd(WINC[direction])
    end
  end)
  return nil
end

---Is `buf` in scope for the configured `terminal_nav` spec?
---@param buf integer
---@param spec true|table
---@return boolean
function M.in_scope(buf, spec)
  if not vim.api.nvim_buf_is_valid(buf) or vim.bo[buf].buftype ~= 'terminal' then
    return false
  end
  if spec == true then
    return true
  end
  if type(spec) ~= 'table' then
    return false
  end
  local ft = vim.bo[buf].filetype
  for _, skip in ipairs(spec.skip_filetypes or {}) do
    if ft == skip then
      return false
    end
  end
  if type(spec.filetypes) ~= 'table' then
    return true
  end
  for _, want in ipairs(spec.filetypes) do
    if ft == want then
      return true
    end
  end
  return false
end

---Map the four chords on `buf` (buffer-local, expr, following nav_keys).
---@param buf integer
function M.attach(buf)
  for direction, winc_key in pairs(WINC) do
    local lhs = config.nav_keys_nvim[direction]
    if lhs then
      vim.keymap.set('t', lhs, function()
        return M.nav(direction, lhs)
      end, { buffer = buf, expr = true, nowait = true, desc = 'Navigate ' .. direction })
    end
  end
end

---Install the TermOpen autocmd and map already-open terminal buffers (a session
---restore rebuilds those without firing TermOpen). Idempotent.
---@param spec true|table|nil false/nil disables
function M.setup(spec)
  if not spec then
    return
  end
  local group = vim.api.nvim_create_augroup('herdr-splits-terminal-nav', { clear = true })
  vim.api.nvim_create_autocmd('TermOpen', {
    group = group,
    callback = function(ev)
      if not M.in_scope(ev.buf, spec) then
        return
      end
      -- Synchronously for determinism (a caller that inspects the maps right
      -- after `termopen()` sees them) ...
      M.attach(ev.buf)
      -- ... and once more a tick later, so we also win over anything else that
      -- maps on TermOpen itself.
      vim.schedule(function()
        if M.in_scope(ev.buf, spec) then
          M.attach(ev.buf)
        end
      end)
    end,
  })
  for _, buf in ipairs(vim.api.nvim_list_bufs()) do
    if vim.api.nvim_buf_is_loaded(buf) and M.in_scope(buf, spec) then
      M.attach(buf)
    end
  end
end

return M
