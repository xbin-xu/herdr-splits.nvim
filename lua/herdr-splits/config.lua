---@class HerdrSplitsConfig
---@field default_amount number Resize amount as Herdr ratio (float, e.g. 0.03 = 3%)
---@field neovim_amount number Resize amount for native Neovim resizes (integer cells, default 3)
---@field at_edge 'wrap'|'stop'|'split'|function Neovim window-edge behavior (distinct from nav_at_edge below)
---@field ignored_buftypes string[] Buffer types ignored during resize
---@field ignored_filetypes string[] Filetypes ignored during resize
---@field move_cursor_same_row boolean Keep cursor on same screen row when moving left/right
---@field herdr_bin string|nil Path to herdr binary (auto-detected if nil)
---@field ignored_events string[] Autocmd events to ignore during resize operations
---@field auto_sync_herdr boolean|nil If true, auto-sync the Herdr-managed checkout to match this lazy commit (opt-in; default false)
---@field floating_zindex_max number Threshold below which a floating window's zindex classifies it as an embedded sidebar (default 50; Neovim's default float zindex)
---@field ignore_previewwindows boolean If true, vim.wo[winid].previewwindow windows (e.g. dadbod `.dbout`) are treated as sidebars (opt-in; default false)
--- Instance policy: the managed keys (nav_keys, resize_keys, unzoom_on_nav,
--- nav_at_edge, most_recently_focused) live in `herdr-splits.conf`, which the
--- Herdr-side helper, plain Vim and Neovim all read. Neovim only writes it when
--- `overwrite_conf` is on, and never overwrites a file it did not generate.
--- Pass chords in Neovim notation (e.g. `<C-h>`); they are stored in Herdr
--- notation.
---@field overwrite_conf boolean (default false) When true, setup() is the authority: it
---  writes the managed keys to the shared conf and ignores a previously generated file
---  when reading, so removing an opt reverts it. When false, the conf file is the
---  authority: setup() never writes and the managed opts are this editor's local
---  override only.
---@field nav_keys table<string,string> Forward chords per direction: {left,down,up,right} — honoured only with `overwrite_conf` (Herdr has to know the chord, and it reads it from the conf)
---@field resize_keys table<string,string> Resize forward chords per direction: {left,down,up,right} — honoured only with `overwrite_conf`
---@field unzoom_on_nav boolean If true, auto-unzoom when navigating away from a zoomed pane (conf default: false — the pane stays zoomed and the zoom follows the focus)
---@field nav_at_edge 'wrap'|'stop' Herdr pane-boundary wrap behavior (distinct from at_edge, which is the Neovim window-edge behavior)
---@field most_recently_focused boolean tmux-style target choice at both levels — the most recently focused window/pane among the adjacent candidates, including at wrapped edges (conf default: true)
---@field terminal_nav false|true|HerdrSplitsTerminalNav Terminal-mode navigation: re-map the nav chords buffer-locally in pty terminal buffers so the edge hands off to Herdr (opt-in, default false). `true` = all terminals, a table scopes it.
---@field nav_keys_nvim table<string,string> The resolved nav_keys in Neovim notation (derived, feeds terminal_nav's t-mode maps)

---@class HerdrSplitsTerminalNav
---@field filetypes string[]|nil Map only these terminal filetypes (omit = no filetype restriction)
---@field skip_filetypes string[]|nil Never map these terminal filetypes

-- Instance-policy defaults (Herdr notation). These mirror the Rust helper's
-- `Config::defaults` and the Python fallback, so a missing conf file means the
-- same thing to every reader. `setup()` overrides them from the conf, then from
-- its own opts.
local managed_defaults = {
  nav_keys = { left = 'ctrl+h', down = 'ctrl+j', up = 'ctrl+k', right = 'ctrl+l' },
  resize_keys = { left = 'alt+h', down = 'alt+j', up = 'alt+k', right = 'alt+l' },
  unzoom_on_nav = false,
  nav_at_edge = 'wrap',
  most_recently_focused = true,
}

local M = {
  default_amount = 0.03,
  neovim_amount = 3,
  at_edge = 'wrap',
  ignored_buftypes = { 'nofile', 'quickfix', 'prompt', 'help', 'terminal' },
  ignored_filetypes = {
    'NvimTree',
    -- sidebars
    'neo-tree',
    'snacks_dashboard',
    'snacks_explorer',
    'snacks_picker',
    -- DB / REPL / data sidebars
    'dadbod-ui',
    'dbout',
    -- outlines / symbols
    'aerial',
    'Outline',
    -- diagnostics / quick lists
    'Trouble',
    'quickfix',
  },
  move_cursor_same_row = false,
  herdr_bin = nil,
  ignored_events = { 'BufEnter', 'WinEnter' },
  auto_sync_herdr = false,
  floating_zindex_max = 50,
  ignore_previewwindows = false,
  -- Instance policy: deep-copied from managed_defaults (single source).
  -- Resolved from defaults <- herdr-splits.conf <- explicit setup() opts, in
  -- Herdr chord notation. Written back to the conf only with overwrite_conf.
  overwrite_conf = false,
  nav_keys = vim.deepcopy(managed_defaults.nav_keys),
  resize_keys = vim.deepcopy(managed_defaults.resize_keys),
  unzoom_on_nav = managed_defaults.unzoom_on_nav,
  nav_at_edge = managed_defaults.nav_at_edge,
  most_recently_focused = managed_defaults.most_recently_focused,
  -- Explicit managed opts from the last setup(), replayed by reload().
  _managed_opts = {},
  -- The last resolved managed set (what write() publishes).
  _resolved = vim.deepcopy(managed_defaults),
  -- Neovim-side t-mode maps (see terminal.lua). Derived from the resolved
  -- nav_keys below so a remap reaches terminal mode too.
  terminal_nav = false,
  nav_keys_nvim = {},
}

local conf = require('herdr-splits.conf')

---Resolve managed values from defaults <- conf values <- this editor's
---explicit opts and assign them to M. Shared by setup() and reload().
---@param existing table managed values parsed from the conf (may be empty)
---@return table resolved
local function apply_managed(existing)
  local resolved = vim.deepcopy(managed_defaults)
  resolved = vim.tbl_deep_extend('force', resolved, existing)
  resolved = vim.tbl_deep_extend('force', resolved, M._managed_opts or {})

  M._resolved = resolved
  M.nav_keys, M.resize_keys = resolved.nav_keys, resolved.resize_keys
  M.unzoom_on_nav, M.nav_at_edge = resolved.unzoom_on_nav, resolved.nav_at_edge
  M.most_recently_focused = resolved.most_recently_focused

  -- Neovim-notation chords for the Neovim-side t-mode maps (terminal_nav),
  -- derived from the resolved Herdr-notation keys above so both sides agree.
  M.nav_keys_nvim = {}
  for dir, chord in pairs(M.nav_keys) do
    M.nav_keys_nvim[dir] = conf.to_nvim(chord)
  end

  return resolved
end

---Apply user configuration on top of the instance policy.
---Idempotent in outcome: the managed set is re-resolved from
---defaults <- herdr-splits.conf <- opts on every call; non-managed opts merge
---into the existing config.
---
---Authority: with `overwrite_conf` off (the default) the conf file wins and
---setup() never writes it; with it on, setup() is the authority and rewrites
---the file. Either way a conf without the generated marker is never touched.
---@param opts table|nil
function M.setup(opts)
  opts = opts or {}

  -- Apply herdr_bin from opts before the first conf.path() resolution so the
  -- documented precedence (config.herdr_bin -> HERDR_BIN_PATH -> 'herdr') holds.
  if opts.herdr_bin ~= nil then M.herdr_bin = opts.herdr_bin end
  if opts.overwrite_conf ~= nil then M.overwrite_conf = opts.overwrite_conf and true or false end

  -- Managed opts: translate user chords nvim -> herdr, capture only what was passed.
  -- Chords reach the Herdr side exclusively through the conf, so they only mean
  -- something while setup() is the authority; in file-authority mode they would
  -- desync Neovim's terminal maps from the chord Herdr actually forwards.
  local user = {}
  local has_nav_keys = type(opts.nav_keys) == 'table' and next(opts.nav_keys) ~= nil
  local has_resize_keys = type(opts.resize_keys) == 'table' and next(opts.resize_keys) ~= nil
  if has_nav_keys or has_resize_keys then
    if M.overwrite_conf then
      if has_nav_keys then
        user.nav_keys = {}
        for dir, k in pairs(opts.nav_keys) do user.nav_keys[dir] = conf.to_herdr(k) end
      end
      if has_resize_keys then
        user.resize_keys = {}
        for dir, k in pairs(opts.resize_keys) do user.resize_keys[dir] = conf.to_herdr(k) end
      end
    else
      vim.notify(
        'herdr-splits: nav_keys/resize_keys only apply with setup({ overwrite_conf = true }) '
          .. '— Herdr reads the chords from ' .. conf.path()
          .. ' (edit it, or run `herdr-splits init` for a scaffold). Ignoring them.',
        vim.log.levels.WARN
      )
    end
  end
  if opts.unzoom_on_nav ~= nil then user.unzoom_on_nav = opts.unzoom_on_nav end
  if opts.nav_at_edge ~= nil then user.nav_at_edge = opts.nav_at_edge end
  if opts.most_recently_focused ~= nil then user.most_recently_focused = opts.most_recently_focused end

  M._managed_opts = user
  -- File-authority mode honours every value in the conf, including one an
  -- earlier setup() generated; setup-authority mode ignores its own output so
  -- removing an opt reverts to the default.
  local existing, found = conf.read_managed(M.overwrite_conf)
  apply_managed(existing)

  -- setup-authority mode only: a hand-written conf is adopted and then
  -- rewritten just below. Say where those values came from, once.
  if found and M.overwrite_conf then
    vim.notify(
      'herdr-splits: adopting values from a hand-written conf (' .. conf.path()
        .. '). setup({ overwrite_conf = true }) rewrites that file from the Lua opts',
      vim.log.levels.INFO
    )
  end

  -- Non-managed opts (default_amount, at_edge, ignored_filetypes, …) merge as before.
  local other = vim.deepcopy(opts)
  other.nav_keys, other.resize_keys = nil, nil
  other.unzoom_on_nav, other.nav_at_edge = nil, nil
  other.most_recently_focused = nil
  other.overwrite_conf = nil
  local merged = vim.tbl_deep_extend('force', M, other)
  for k, v in pairs(merged) do M[k] = v end

  -- Publish only when setup() is the authority (never fatal).
  if M.overwrite_conf then
    pcall(conf.write, M._resolved)
  end
end

---Re-read the conf and refresh the managed values without restarting Neovim.
---Chords are refreshed in memory too, but Neovim's terminal-mode maps are
---installed per buffer from `nav_keys_nvim`, so a changed chord only reaches
---Terminal mode once those buffers are reopened (or Neovim restarts).
---
---In `overwrite_conf` mode the file is the projection of this editor's opts, so
---a reload also rewrites it: otherwise the helper (which reads the file on every
---keypress) could keep a value this session no longer uses — a hand-written file
---adopted by an earlier `setup()` is the case that matters here.
---@return table report { path, policy_changed, chords_changed }
function M.reload()
  local previous = {
    nav_keys = vim.deepcopy(M.nav_keys),
    resize_keys = vim.deepcopy(M.resize_keys),
    unzoom_on_nav = M.unzoom_on_nav,
    nav_at_edge = M.nav_at_edge,
    most_recently_focused = M.most_recently_focused,
  }

  apply_managed(conf.read_managed(M.overwrite_conf))
  if M.overwrite_conf then
    pcall(conf.write, M._resolved)
  end

  return {
    path = conf.path(),
    policy_changed = previous.unzoom_on_nav ~= M.unzoom_on_nav
      or previous.nav_at_edge ~= M.nav_at_edge
      or previous.most_recently_focused ~= M.most_recently_focused,
    chords_changed = not vim.deep_equal(previous.nav_keys, M.nav_keys)
      or not vim.deep_equal(previous.resize_keys, M.resize_keys),
  }
end

return M
