---Herdr interface: control-socket RPC (rpc.lua) plus the operations nav and
---resize need. Navigation no longer shells out to the herdr CLI — a spawn
---costs ~20ms per call on Windows versus ~0.5ms for an RPC roundtrip.
---
---The Herdr-side scripts (scripts/herdr-*.py) remain the entry point for
---keypresses that land in Herdr itself; this module is the Neovim-side
---counterpart.
---@class HerdrSplitsHerdr
local M = {}

local config = require('herdr-splits.config')
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

---Focus a Herdr pane in the given direction (Herdr's geometric rule).
---Calls `pane.focus_direction` over the control socket.
---@param direction '"left"'|'"right"'|'"up"'|'"down"'
---@return boolean true on success
function M.focus_pane(direction)
  if not M.is_in_session() then
    return false
  end
  local resp = rpc.call('pane.focus_direction', {
    pane_id = M.current_pane_id(),
    direction = direction,
  })
  return resp ~= nil and resp.error == nil
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
