---herdr-splits shared config file (the instance policy) + notation translator.
---Owns everything about `herdr-splits.conf`: path resolution, reading the
---managed values, translating Neovim key notation to Herdr chord notation,
---and writing the resolved config back (atomically). Requires nothing else
---from this plugin (avoiding a require-cycle with config.lua).
---@class HerdrSplitsConf
local M = {}

-- Neovim modifier prefix -> Herdr modifier name.
local MOD = { C = 'ctrl', M = 'alt', A = 'alt', S = 'shift', D = 'cmd' }
-- Herdr's documented specials: lowercase the terminal key when present.
local SPECIAL = {
  left = 'left', right = 'right', up = 'up', down = 'down',
  enter = 'enter', ['return'] = 'enter', tab = 'tab',
  esc = 'esc', escape = 'esc', space = 'space', backspace = 'backspace',
  del = 'delete', delete = 'delete', home = 'home',
  ['end'] = 'end', pageup = 'pageup', pagedown = 'pagedown',
}

-- Marker line stamped into every generated conf. Its presence means the file
-- is the plugin's own output: setup() is authoritative, so read_managed() does
-- NOT adopt it back (removing an opt reverts to the default on the next start).
local MARKER = '# herdr-splits-generated-v1'

local uv = vim.uv or vim.loop

-- Lazily-resolved cache of the shared config path, so setup() shells out to
-- `herdr plugin config-dir` at most once per process.
local resolved_path

---Resolve the herdr binary: a config.herdr_bin override (when the config
---module is loaded), then HERDR_BIN_PATH, then 'herdr'. pcall-required so
---conf.lua never hard-requires config.lua at load time (cycle-safe).
---@return string
local function resolve_herdr_bin()
  local ok, cfg = pcall(require, 'herdr-splits.config')
  if ok and cfg.herdr_bin then return cfg.herdr_bin end
  return vim.env.HERDR_BIN_PATH or 'herdr'
end

---Best-effort `herdr plugin config-dir herdr-splits` query. Returns the
---directory printed by herdr (trimmed), or nil on any failure / empty output.
---@return string|nil
local function query_config_dir()
  local ok, obj = pcall(vim.system, { resolve_herdr_bin(), 'plugin', 'config-dir', 'herdr-splits' }, { text = true })
  if not ok or not obj then return nil end
  local wok, res = pcall(obj.wait, obj)
  if not wok or not res or res.code ~= 0 then return nil end
  local out = (res.stdout or ''):gsub('^%s+', ''):gsub('%s+$', '')
  return out ~= '' and out or nil
end

---Path to the shared herdr-splits config file (same file the Herdr-side
---scripts read). Precedence: (1) HERDR_SPLITS_CONFIG; (2) HERDR_PLUGIN_CONFIG_DIR
---+ `/herdr-splits.conf`; (3) `herdr plugin config-dir herdr-splits` (via the
---herdr binary) + `/herdr-splits.conf`; (4) XDG_CONFIG_HOME / ~/.config
---fallback. Resolved once and cached, so setup() shells out at most once.
---@return string
function M.path()
  if resolved_path then return resolved_path end
  if vim.env.HERDR_SPLITS_CONFIG and vim.env.HERDR_SPLITS_CONFIG ~= '' then
    resolved_path = vim.env.HERDR_SPLITS_CONFIG
    return resolved_path
  end
  if vim.env.HERDR_PLUGIN_CONFIG_DIR and vim.env.HERDR_PLUGIN_CONFIG_DIR ~= '' then
    resolved_path = vim.env.HERDR_PLUGIN_CONFIG_DIR .. '/herdr-splits.conf'
    return resolved_path
  end
  local dir = query_config_dir()
  if dir then
    resolved_path = dir .. '/herdr-splits.conf'
    return resolved_path
  end
  local xdg = vim.env.XDG_CONFIG_HOME
  local base = (xdg and xdg:sub(1, 1) == '/') and xdg
      or ((vim.env.HOME or '~') .. '/.config')
  resolved_path = base .. '/herdr/plugins/config/herdr-splits/herdr-splits.conf'
  return resolved_path
end

---Translate a Neovim key notation (e.g. `<C-h>`, `<M-Left>`, `h`) into the
---Herdr chord notation the scripts and `herdr pane send-keys` expect
---(e.g. `ctrl+h`, `alt+left`, `h`). Returns nil for modifier-only / empty
---input so callers can fall back to a per-direction default.
---@param nvim_key string
---@return string|nil
function M.to_herdr(nvim_key)
  if type(nvim_key) ~= 'string' or nvim_key == '' then return nil end
  if not nvim_key:match('^<.+>$') then return nvim_key:lower() end -- plain "h" -> "h"
  local inner = nvim_key:sub(2, -2)
  local mods, key = {}, nil
  for part in inner:gmatch('[^-]+') do
    local m = MOD[part:upper()]
    if m then
      mods[#mods + 1] = m
    else
      key = SPECIAL[part:lower()] or part:lower()
    end
  end
  if not key then return nil end
  mods[#mods + 1] = key
  return table.concat(mods, '+')
end

-- Reverse tables for to_nvim. `enter` normalises to <CR> (Neovim's canonical
-- spelling; `<Return>`/`<Enter>` in the input both landed on `enter`).
local MOD_REV = { ctrl = 'C', alt = 'M', shift = 'S', cmd = 'D' }
local SPECIAL_REV = {
  left = 'Left', right = 'Right', up = 'Up', down = 'Down',
  enter = 'CR', tab = 'Tab', esc = 'Esc', space = 'Space',
  backspace = 'BS', delete = 'Del', home = 'Home', ['end'] = 'End',
  pageup = 'PageUp', pagedown = 'PageDown',
}
-- Canonical modifier order in Neovim notation.
local MOD_ORDER = { 'C', 'M', 'S', 'D' }

---Translate Herdr chord notation back into Neovim key notation (the inverse of
---to_herdr), so Neovim-side mappings can follow the resolved `nav_keys`.
---`enter` becomes `<CR>` — Neovim's canonical spelling.
---@param chord string|nil e.g. `ctrl+h`, `alt+left`
---@return string|nil e.g. `<C-h>`, `<M-Left>`
function M.to_nvim(chord)
  if type(chord) ~= 'string' or chord == '' then return nil end
  local parts = vim.split(chord, '+', { plain = true })
  local key = table.remove(parts)
  if not key or key == '' then return nil end
  local name = SPECIAL_REV[key] or key
  local seen = {}
  for _, mod in ipairs(parts) do
    local m = MOD_REV[mod]
    if m then seen[m] = true end
  end
  local mods = {}
  for _, m in ipairs(MOD_ORDER) do
    if seen[m] then mods[#mods + 1] = m end
  end
  if #mods == 0 then return name end
  return '<' .. table.concat(mods, '-') .. '-' .. name .. '>'
end

---Whether `text` carries the generated MARKER. Present means the file is
---setup()'s own output (a projection of the managed opts); absent means the
---file belongs to the user, and setup() must never overwrite it.
---@param text string
---@return boolean
function M.is_generated(text)
  for line in text:gmatch('[^\n]+') do
    if line:match('^%s*' .. vim.pesc(MARKER) .. '%s*$') then return true end
  end
  return false
end

---Parse the shared config file for the managed keys only. `#` starts a
---trailing comment, blank lines are skipped, and the LAST occurrence of a key
---wins (matching the Herdr-side scripts' `tail -n 1`). Returns `({}, false)`
---for a missing file.
---
---`ignore_generated` picks the authority:
---* `false`/`nil` — the conf file is authoritative (`overwrite_conf` off, the
---  default). Every value is honoured, including a file `setup()` generated in
---  an earlier session, so taking ownership by deleting the marker line and an
---  upgrade from the setup-authoritative era both keep working.
---* `true` — `setup()` is authoritative (`overwrite_conf` on). A file carrying
---  the MARKER is our own projection and is NOT read back, so removing an opt
---  reverts to its default on the next start.
---@param ignore_generated boolean|nil
---@return table values, boolean had_managed_values
function M.read_managed(ignore_generated)
  local f = io.open(M.path(), 'r')
  if not f then return {}, false end
  local out = { nav_keys = {}, resize_keys = {} }
  for line in f:lines() do
    -- setup()-authoritative mode: our own output is not read back.
    if ignore_generated and line:match('^%s*' .. vim.pesc(MARKER) .. '%s*$') then
      f:close()
      return {}, false
    end
    local k, v = line:match('^%s*([%w_]+)%s*=%s*([^%s#]+)')
    if k and v then
      local dir = k:match('^nav_key_(%a+)$')
      if dir then
        out.nav_keys[dir] = v
      else
        dir = k:match('^resize_key_(%a+)$')
        if dir then
          out.resize_keys[dir] = v
        elseif k == 'unzoom_on_nav' then
          -- Case-insensitive, like the Rust/Python/Vim readers.
          out.unzoom_on_nav = (v:lower() ~= 'false')
        elseif k == 'nav_at_edge' then
          out.nav_at_edge = (v == 'stop' and 'stop' or 'wrap')
        elseif k == 'most_recently_focused' then
          -- Case-insensitive: only `true` enables it, anything else disables.
          out.most_recently_focused = (v:lower() == 'true')
        end
      end
    end
  end
  f:close()
  local adopted = false
  for _, t in pairs({ out.nav_keys, out.resize_keys }) do
    for _ in pairs(t) do adopted = true; break end
  end
  if out.unzoom_on_nav ~= nil or out.nav_at_edge ~= nil or out.most_recently_focused ~= nil then
    adopted = true
  end
  return out, adopted
end

---Write the full managed key set to the shared config with a generated-file
---header (including the MARKER), atomically via a unique same-dir temp +
---os.rename so a script never observes a partial write and two concurrent
---Neovim instances don't share a temp inode. Skips the write entirely when the
---content is unchanged (preserves mtime, reduces multi-instance contention).
---
---This is the *only* writer, and it only runs for `setup({ overwrite_conf =
---true })`: with the default (`false`) no code path touches the file, which is
---the real guarantee that a hand-written conf survives. When it does run on a
---headerless file it takes the file over (stamping the MARKER); setup() warns
---that it adopted those values first.
---Returns (true, nil) on success or (false, err) on failure; callers wrap in
---pcall so a write failure never crashes startup.
---@param resolved table resolved managed config (nav_keys, resize_keys, unzoom_on_nav, nav_at_edge, most_recently_focused)
---@return boolean ok, string|nil err
function M.write(resolved)
  local path = M.path()
  local lines = {
    MARKER,
    '# herdr-splits plugin config — GENERATED by herdr-splits.nvim setup().',
    '# Written because setup({ overwrite_conf = true }): this file is a projection',
    '# of the Lua options and is regenerated on every setup() — remove an opt in',
    '# setup() to revert it to the default.',
    '#',
    '# To make this file yours: delete the marker line above (and set',
    '# overwrite_conf = false, the default). setup() then reads it as the instance',
    '# policy and never overwrites it. `herdr-splits init` writes a fresh scaffold.',
    'nav_key_left='     .. resolved.nav_keys.left,
    'nav_key_down='     .. resolved.nav_keys.down,
    'nav_key_up='       .. resolved.nav_keys.up,
    'nav_key_right='    .. resolved.nav_keys.right,
    'resize_key_left='  .. resolved.resize_keys.left,
    'resize_key_down='  .. resolved.resize_keys.down,
    'resize_key_up='    .. resolved.resize_keys.up,
    'resize_key_right=' .. resolved.resize_keys.right,
    'unzoom_on_nav='    .. (resolved.unzoom_on_nav == false and 'false' or 'true'),
    'nav_at_edge='      .. (resolved.nav_at_edge == 'stop' and 'stop' or 'wrap'),
    'most_recently_focused=' .. (resolved.most_recently_focused == true and 'true' or 'false'),
  }
  local content = table.concat(lines, '\n') .. '\n'

  local existing = io.open(path, 'r')
  if existing then
    local prev = existing:read('*a')
    existing:close()
    -- Skip when unchanged: only when the existing bytes match exactly.
    if prev == content then return true end
  end

  vim.fn.mkdir(vim.fn.fnamemodify(path, ':h'), 'p')
  -- Unique same-dir temp so os.rename stays same-filesystem (atomic) and two
  -- Neovim instances don't collide on a shared temp inode.
  local tmp = path .. '.tmp.' .. vim.fn.getpid() .. '.' .. tostring(uv.hrtime())
  local f, err = io.open(tmp, 'w')
  if not f then
    pcall(os.remove, tmp)
    vim.notify('herdr-splits: failed to open conf temp file: ' .. tostring(err), vim.log.levels.WARN)
    return false, err
  end
  local wok, werr = f:write(content)
  -- file:close() flushes buffered output and can itself fail with a delayed
  -- write error (disk full, NFS, quota). Its result MUST be checked before
  -- os.rename: otherwise an incomplete temp replaces the live config and the
  -- atomic-write guarantee is lost. LuaJIT returns true on success or
  -- (nil, err) on a flush failure — same form as file:write, checked here too.
  local cok, cerr = f:close()
  if (not wok) or (not cok) then
    local why = (not wok) and werr or cerr
    pcall(os.remove, tmp)
    vim.notify('herdr-splits: failed to write conf temp file: ' .. tostring(why), vim.log.levels.WARN)
    return false, why
  end
  -- uv.fs_rename atomically replaces an existing destination on Windows
  -- (MoveFileExW + MOVEFILE_REPLACE_EXISTING); the CRT's os.rename fails
  -- with EEXIST whenever the destination exists, which broke every conf
  -- regeneration on Windows (stale conf + repeated startup warning).
  local ok, rerr = uv.fs_rename(tmp, path)
  if not ok then
    -- Fallback for destinations MoveFileEx cannot replace (e.g. held open
    -- without FILE_SHARE_DELETE): remove then rename. Briefly non-atomic,
    -- but strictly better than leaving the stale conf in place.
    pcall(os.remove, path)
    ok, rerr = os.rename(tmp, path)
  end
  if not ok then
    pcall(os.remove, tmp)
    vim.notify('herdr-splits: failed to write conf: ' .. tostring(rerr), vim.log.levels.WARN)
  end
  return ok, rerr
end

---Runnable self-check for the notation translator. Asserts on the documented
---mapping table; returns true on success. Intended for headless validation:
---`nvim --headless -c "lua assert(require('herdr-splits.conf')._selfcheck())" -c "qa"`.
---@return boolean
function M._selfcheck()
  local cases = {
    { '<C-h>', 'ctrl+h' }, { '<C-j>', 'ctrl+j' }, { '<C-k>', 'ctrl+k' }, { '<C-l>', 'ctrl+l' },
    { '<M-h>', 'alt+h' }, { '<M-Left>', 'alt+left' }, { '<S-Left>', 'shift+left' },
    { '<D-x>', 'cmd+x' }, { '<C-M-h>', 'ctrl+alt+h' }, { 'h', 'h' }, { '<Esc>', 'esc' },
  }
  for _, c in ipairs(cases) do
    local got = M.to_herdr(c[1])
    assert(got == c[2], ('to_herdr(%q)=%q want %q'):format(c[1], tostring(got), c[2]))
  end
  return true
end

return M
