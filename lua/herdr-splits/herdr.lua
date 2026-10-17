---Herdr interface: control-socket RPC (rpc.lua) plus the operations nav and
---resize need. Navigation no longer shells out to the herdr CLI — a spawn
---costs ~20ms per call on Windows versus ~0.5ms for an RPC roundtrip — and
---the pane-level most_recently_focused selection runs here, sharing the
---mru-panes recency file with the Herdr-side scripts.
---
---The Herdr-side scripts (scripts/herdr-*.py) remain the entry point for
---keypresses that land in Herdr itself; this module is the Neovim-side
---counterpart.
---@class HerdrSplitsHerdr
local M = {}

local config = require('herdr-splits.config')
local conf = require('herdr-splits.conf')
local geometry = require('herdr-splits.geometry')
local rpc = require('herdr-splits.rpc')

---Resolve the herdr binary path. Still used for setup-time work (conf-dir
---discovery, plugin sync); navigation itself goes over the socket.
---@return string
function M.herdr_bin()
  return config.herdr_bin or vim.env.HERDR_BIN_PATH or 'herdr'
end

---Check if Neovim is running inside a Herdr session.
---@return boolean
function M.is_in_session()
  return vim.env.HERDR_ENV == '1' and vim.env.HERDR_PANE_ID ~= nil and #vim.env.HERDR_PANE_ID > 0
end

---Get the current Herdr pane ID.
---@return string|nil
function M.current_pane_id()
  if not M.is_in_session() then
    return nil
  end
  return vim.env.HERDR_PANE_ID
end

---Check if the current Herdr pane is at the layout boundary in the given direction.
---@param direction '"left"'|'"right"'|'"up"'|'"down"'
---@return boolean|nil true if at edge, false if neighbor exists, nil on error
function M.current_pane_at_edge(direction)
  if not M.is_in_session() then
    return nil
  end
  local resp = rpc.call('pane.edges', { pane_id = M.current_pane_id() })
  local edges = resp and resp.error == nil and resp.result and resp.result.edges
  if type(edges) ~= 'table' then
    return nil
  end
  local at_edge = edges[direction]
  if type(at_edge) ~= 'boolean' then
    return nil
  end
  return at_edge
end

---Check if the current Herdr pane is zoomed.
---@return boolean|nil true if zoomed, false if not, nil on error
function M.current_pane_is_zoomed()
  if not M.is_in_session() then
    return nil
  end
  -- pane.layout with no pane_id reports the current tab's layout.
  local resp = rpc.call('pane.layout', {})
  local layout = resp and resp.error == nil and resp.result and resp.result.layout
  if type(layout) ~= 'table' then
    return nil
  end
  local zoomed = layout.zoomed
  if type(zoomed) ~= 'boolean' then
    return nil
  end
  return zoomed
end

---Pane rectangles of the current tab, as geometry.candidates entries, in
---the server's own order; nil when the layout is unavailable.
---@return boolean|nil zoomed, table|nil rects
local function pane_rects()
  local resp = rpc.call('pane.layout', {})
  local layout = resp and resp.error == nil and resp.result and resp.result.layout
  if type(layout) ~= 'table' or type(layout.panes) ~= 'table' then
    return nil
  end
  local rects = {}
  for _, p in ipairs(layout.panes) do
    local r = p.rect or {}
    rects[#rects + 1] = {
      id = p.pane_id,
      x = r.x or 0,
      y = r.y or 0,
      w = r.width or 0,
      h = r.height or 0,
    }
  end
  return layout.zoomed == true, rects
end

---Pane recency state, shared with the Herdr-side scripts: one pane id per
---line, most-recent-first, in a file next to the shared conf (the scripts'
---`_mru_path`). Each keypress is a fresh process on the script side, so the
---history must live on disk.
---@return string
local function mru_path()
  return vim.fn.fnamemodify(conf.path(), ':h') .. '/mru-panes'
end

---@return string[] most-recent-first (raw file order; callers filter)
local function load_pane_mru()
  local f = io.open(mru_path(), 'r')
  if not f then
    return {}
  end
  local order = {}
  for line in f:lines() do
    local p = vim.trim(line)
    if p ~= '' then
      order[#order + 1] = p
    end
  end
  f:close()
  return order
end

---Best-effort atomic write; MRU state is an optimization, never fatal.
---@param order string[]
local function save_pane_mru(order)
  local path = mru_path()
  local tmp = path .. '.tmp.' .. vim.fn.getpid()
  local f = io.open(tmp, 'w')
  if not f then
    return
  end
  local wok = f:write(table.concat(order, '\n') .. (#order > 0 and '\n' or ''))
  local cok = f:close()
  if not wok or not cok then
    pcall(os.remove, tmp)
    return
  end
  local uv = vim.uv or vim.loop
  local renamed = uv.fs_rename(tmp, path)
  if not renamed then
    -- MoveFileEx-failure fallback, as in conf.write: briefly non-atomic.
    pcall(os.remove, path)
    pcall(os.rename, tmp, path)
  end
end

---True when the layout still describes a real tiling. Herdr keeps
---reporting the pre-zoom pane rectangles while a tab is zoomed, so the
---search works unchanged; older servers collapsed a zoomed layout to the
---zoomed pane's full-area rectangle, which cannot be searched.
---@param rects HerdrSplitsRect[]
---@return boolean
local function layout_is_tiled(rects)
  if #rects < 2 then
    return false
  end
  local seen = {}
  for _, r in ipairs(rects) do
    seen[string.format('%d,%d,%d,%d', r.x, r.y, r.w, r.h)] = true
  end
  local distinct = 0
  for _ in pairs(seen) do
    distinct = distinct + 1
    if distinct > 1 then
      return true
    end
  end
  return false
end

---tmux-style pane selection: focus the most recently focused pane among the
---candidates adjacent in `direction`, using the same rule and the same
---fallbacks as scripts/herdr_common.py `mru_focus` (whose caller in
---herdr-nav.py falls through to `pane.focus_direction` when the search
---decides nothing). Returns true only when a pane was focused; every other
---outcome falls back to the geometric focus in M.focus_pane.
---@param direction '"left"'|'"right"'|'"up"'|'"down"'
---@return boolean
local function focus_pane_mru(direction)
  local cur = M.current_pane_id()
  if not cur then
    return false
  end

  local zoomed, rects = pane_rects()
  if not rects then
    return false
  end
  if zoomed then
    if M.unzoom_enabled() then
      rpc.call('pane.zoom', { pane_id = cur, mode = 'off' })
      zoomed, rects = pane_rects()
      if not rects then
        return false
      end
    elseif not layout_is_tiled(rects) then
      -- A collapsed zoomed layout (older Herdr) has no searchable panes;
      -- the caller falls back to Herdr's geometric focus.
      return false
    end
  end

  local cands = geometry.candidates(rects, cur, direction, false)
  if #cands == 0 and M.nav_at_edge() ~= 'stop' then
    cands = geometry.candidates(rects, cur, direction, true)
  end
  if #cands == 0 then
    return false
  end

  local is_cand, is_live = {}, {}
  for _, p in ipairs(cands) do
    is_cand[p] = true
  end
  for _, r in ipairs(rects) do
    is_live[r.id] = true
  end
  -- Live panes only; the current pane counts as the most recent focus.
  local order = { cur }
  for _, p in ipairs(load_pane_mru()) do
    if p ~= cur and is_live[p] then
      order[#order + 1] = p
    end
  end
  local pick
  for _, p in ipairs(order) do
    if is_cand[p] then
      pick = p
      break
    end
  end
  if not pick then
    pick = cands[1] -- no history yet: server order, deterministic
  end
  local saved = { pick }
  for _, p in ipairs(order) do
    if p ~= pick then
      saved[#saved + 1] = p
    end
  end
  save_pane_mru(saved)

  local resp = rpc.call('pane.focus', { pane_id = pick })
  return resp ~= nil and resp.error == nil
end

---Focus a Herdr pane in the given direction.
---With `most_recently_focused` the target is picked tmux-style among the
---adjacent panes (shared mru-panes state with the Herdr-side scripts);
---anything the search cannot decide falls through to Herdr's own geometric
---focus_direction, mirroring the scripts' behaviour.
---@param direction '"left"'|'"right"'|'"up"'|'"down"'
---@return boolean true on success
function M.focus_pane(direction)
  if not M.is_in_session() then
    return false
  end

  if config.most_recently_focused and focus_pane_mru(direction) then
    return true
  end

  local resp = rpc.call('pane.focus_direction', {
    pane_id = M.current_pane_id(),
    direction = direction,
  })
  return resp ~= nil and resp.error == nil
end

---Focus the pane the wrap search would target in `direction`: the
---candidates at the OPPOSITE layout boundary (tmux's edge flip), the same
---rule wrap_local applies to Neovim windows. Used when Neovim wraps at its
---edge into a sibling Herdr pane while `most_recently_focused` is on.
---Returns false when the option is off, outside a session, or the search
---decides nothing — callers then fall back to the geometric reverse focus.
---@param direction '"left"'|'"right"'|'"up"'|'"down"'
---@return boolean true when a pane was focused
function M.focus_pane_wrap(direction)
  if not M.is_in_session() or not config.most_recently_focused then
    return false
  end
  return focus_pane_mru(direction)
end

---Unzoom the current Herdr pane (turn off zoom).
---@return boolean true on success
function M.unzoom()
  if not M.is_in_session() then
    return false
  end
  local resp = rpc.call('pane.zoom', { pane_id = M.current_pane_id(), mode = 'off' })
  return resp ~= nil and resp.error == nil
end

---Check whether auto-unzoom is enabled.
---Reads `config.unzoom_on_nav` (resolved from setup(); default: enabled).
---@return boolean
function M.unzoom_enabled()
  return config.unzoom_on_nav ~= false
end

---Edge-wrap behavior across the Herdr pane boundary, read from the resolved
---in-memory config (the same value written to the shared conf that the
---Herdr-side scripts read). `stop` suppresses wrap-across-boundary on both
---the plain-pane side (handled by the Herdr script) and the Neovim
---edge-wrap side (handled here). Anything else defaults to `wrap`.
---@return '"wrap"'|'"stop"'
function M.nav_at_edge()
  return config.nav_at_edge == 'stop' and 'stop' or 'wrap'
end

---Resize the current Herdr pane in the given direction.
---@param direction '"left"'|'"right"'|'"up"'|'"down"'
---@param amount number Float ratio (e.g., 0.03 = 3% of terminal dimension)
---@return boolean true on success
function M.resize_pane(direction, amount)
  if not M.is_in_session() then
    return false
  end
  local resp = rpc.call('pane.resize', {
    pane_id = M.current_pane_id(),
    direction = direction,
    amount = amount,
  })
  return resp ~= nil and resp.error == nil
end

return M
