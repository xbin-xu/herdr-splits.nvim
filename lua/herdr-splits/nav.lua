---Navigation logic: seamless movement between Neovim splits and Herdr panes.
---@class HerdrSplitsNav
local M = {}

local config = require('herdr-splits.config')
local geometry = require('herdr-splits.geometry')
local herdr = require('herdr-splits.herdr')
local win = require('herdr-splits.win')

---Treat the current window as a sidebar that should not be navigated through.
---Combines the configured ignore lists with the embedded-float heuristic.
---@return boolean
local function is_sidebar()
  return win.is_ignored_or_preview() or win.is_embedded_floating_window()
end

---Split a new Neovim window using the user's placement preferences.
---@param direction '"left"'|'"right"'|'"up"'|'"down"'
local function split_edge(direction)
  if direction == 'left' or direction == 'right' then
    vim.cmd('vsp')
  else
    vim.cmd('sp')
  end
end

---@param winid number
---@param callback function
---@return any
local function call_in_window(winid, callback)
  if winid == vim.api.nvim_get_current_win() then
    return callback()
  end
  local ok, result = pcall(vim.api.nvim_win_call, winid, callback)
  if ok then
    return result
  end
end

---@param winid number
---@param key string
---@param count number
---@return number|nil, number|nil, number|nil
local function layout_winnrs(winid, key, count)
  local result = call_in_window(winid, function()
    local current = vim.fn.winnr()
    return {
      target = vim.fn.winnr(count .. key),
      previous = count > 1 and vim.fn.winnr((count - 1) .. key) or current,
      current = current,
    }
  end)
  if result then
    return result.target, result.previous, result.current
  end
end

---tmux-style pane selection (`config.most_recently_focused`): among the
---windows adjacent in the requested direction, the most recently focused
---one wins — tmux's directional select-pane semantics (its `active_point`
---ordering, verified empirically against tmux 3.4). At the layout edge the
---search flips to the opposite boundary, mirroring tmux's edge wrap.
---(The `mru*` locals below are the internal shorthand for
---most_recently_focused.)
local mru = {}
local mru_counter = 0
local mru_group

local function mru_bump(win)
  mru_counter = mru_counter + 1
  mru[win] = mru_counter
end

---Create the WinEnter tracker on first use (MRU mode only). Every window
---transition Neovim performs fires WinEnter (wincmd, nvim_set_current_win,
---splits, closes, tab switches), so the autocmd is the single writer of
---recency and no call site bumps explicitly. Keeping the search correct:
---the current window is by definition the one entered last, i.e. always
---the most recent.
local function mru_ensure()
  if mru_group then
    return
  end
  mru_group = vim.api.nvim_create_augroup('herdr-splits-mru', { clear = true })
  vim.api.nvim_create_autocmd('WinEnter', {
    group = mru_group,
    callback = function()
      mru_bump(vim.api.nvim_get_current_win())
    end,
  })
end

---Tiled-window rectangle (top-left row/col plus size); floats are excluded
---(screen-relative position, not part of the tiling the search walks).
local function tiled_rect(w)
  if vim.api.nvim_win_get_config(w).relative ~= '' then
    return nil
  end
  local h = vim.api.nvim_win_get_height(w)
  local width = vim.api.nvim_win_get_width(w)
  if h == 0 or width == 0 then
    return nil
  end
  local pos = vim.api.nvim_win_get_position(w)
  return { row = pos[1], col = pos[2], h = h, w = width }
end

---Windows the tmux directional search considers from the current window in
---`direction` (see geometry.candidates for the rule itself). Floats and
---zero-sized windows are excluded — screen-relative position, not part of
---the tiling the search walks.
---@param direction '"left"'|'"right"'|'"up"'|'"down"'
---@param wrap boolean
---@return number[] winids in tabpage order
local function mru_candidates(direction, wrap)
  local rects = {}
  for _, w in ipairs(vim.api.nvim_tabpage_list_wins(0)) do
    local r = tiled_rect(w)
    if r then
      rects[#rects + 1] = { id = w, x = r.col, y = r.row, w = r.w, h = r.h }
    end
  end
  return geometry.candidates(rects, vim.api.nvim_get_current_win(), direction, wrap)
end

---Most recently focused among `cands`; ties keep the earliest in order,
---mirroring tmux's strict `active_point >` comparison.
---@param cands number[]
---@return number|nil
local function mru_pick(cands)
  local best, best_p
  for _, w in ipairs(cands) do
    local p = mru[w] or 0
    if not best_p or p > best_p then
      best, best_p = w, p
    end
  end
  return best
end

---Move cursor between Neovim splits, falling through to Herdr at edges.
---This is the core navigation function.
---@param direction '"left"'|'"right"'|'"up"'|'"down"'
---@param opts table|nil { same_row: boolean|nil, at_edge: string|function|nil }
function M.move_cursor(direction, opts)
  local same_row = config.move_cursor_same_row
  local at_edge_behavior = config.at_edge

  if type(opts) == 'table' then
    if opts.same_row ~= nil then
      same_row = opts.same_row
    end
    if opts.at_edge ~= nil then
      at_edge_behavior = opts.at_edge
    end
  end

  local embedded = win.is_embedded_floating_window()
  if win.is_floating() and not embedded then
    herdr.focus_pane(direction)
    return
  end

  local dir_key = win.dir_keys[direction]
  local offset = vim.fn.winline() + vim.api.nvim_win_get_position(0)[1]
  local prev_win = vim.api.nvim_get_current_win()
  local geometry_win = prev_win
  if embedded then
    geometry_win = win.resolve_embedded_parent(prev_win)
    if not geometry_win then
      return
    end
  end

  local count = vim.v.count1
  local target_winnr, previous_winnr, current_winnr = layout_winnrs(geometry_win, dir_key, count)
  if not target_winnr then
    return
  end
  local will_wrap
  if count > 1 then
    will_wrap = target_winnr == previous_winnr
  else
    will_wrap = target_winnr == current_winnr
  end

  local function restore_same_row()
    if (direction == 'left' or direction == 'right') and same_row then
      local row = offset - vim.api.nvim_win_get_position(0)[1]
      if row > 0 then
        vim.cmd('normal! ' .. row .. 'H')
      end
    end
  end

  local function move_local(key, move_count)
    if not embedded then
      local before = vim.api.nvim_get_current_win()
      vim.cmd(move_count .. 'wincmd ' .. key)
      return vim.api.nvim_get_current_win() ~= before
    end

    local target = call_in_window(geometry_win, function()
      return vim.fn.win_getid(vim.fn.winnr(move_count .. key))
    end)
    if not target or target == geometry_win then
      return false
    end
    local ok = pcall(vim.api.nvim_set_current_win, target)
    return ok and vim.api.nvim_get_current_win() == target
  end

  ---Jump to the window on the opposite side.
  ---With `most_recently_focused`, pick among the opposite-edge candidates
  ---the tmux way (the most recently focused one; tmux flips its directional
  ---search across the layout edge). Otherwise: keep stepping in the reverse
  ---direction until the walk stops — the window it stops at sits on the far
  ---edge. (One reverse step would only reach the adjacent window, which
  ---coincides with the opposite edge solely when the row/column holds two
  ---windows; with three or more, or in asymmetric layouts, it lands short.)
  local function wrap_local()
    if config.most_recently_focused and not embedded then
      local target = mru_pick(mru_candidates(direction, true))
      if target then
        local ok = pcall(vim.api.nvim_set_current_win, target)
        if ok and vim.api.nvim_get_current_win() == target then
          return true
        end
      end
    end
    local key = win.dir_keys_reverse[direction]
    local guard = vim.fn.winnr('$')
    while guard > 0 and move_local(key, 1) do
      guard = guard - 1
    end
    return true
  end

  ---Wrap-across-boundary: cross into the sibling Herdr pane on the
  ---opposite side (we are at the Herdr edge in `direction`, wrap is on, and
  ---a pane exists behind us). With `most_recently_focused` the target is
  ---the tmux flip — the wrap search in `direction` picks among the
  ---candidates at the opposite boundary, matching wrap_local; anything the
  ---search cannot decide falls back to the geometric reverse focus, which
  ---is Herdr's own rule (and what the Herdr-side scripts use for plain
  ---panes).
  local function cross_wrap_pane()
    if herdr.focus_pane_wrap(direction) then
      return
    end
    herdr.focus_pane(win.reverse_direction[direction])
  end

  -- Command-line window (q:, q/, q?): Neovim forbids all window commands
  -- (E11). Never wincmd; at a Neovim screen edge, delegate to Herdr
  -- (subprocess-safe, does not close the cmdwin); otherwise silent no-op.
  -- Mirrors smart-splits.nvim PR #464.
  if win.is_command_line_window() then
    if will_wrap and herdr.is_in_session() then
      local at_herdr_edge = herdr.current_pane_at_edge(direction)
      if at_herdr_edge == false then
        herdr.focus_pane(direction)
      elseif at_herdr_edge == true and herdr.nav_at_edge() ~= 'stop' then
        cross_wrap_pane()
      end
    end
    return
  end

  -- tmux-style selection (opt-in): among the adjacent windows in this
  -- direction, go to the most recently focused one instead of Neovim's
  -- native pick. Recency needs no bump here: the landing set_current_win
  -- fires WinEnter, and the departing window is already the most recent
  -- (it was entered last). With no candidates (layout edge) the flow below
  -- still decides: Herdr-cross first, then the wrap behaviour.
  if config.most_recently_focused and count == 1 and not embedded then
    mru_ensure()
    local target = mru_pick(mru_candidates(direction, false))
    if target then
      local ok = pcall(vim.api.nvim_set_current_win, target)
      if ok and vim.api.nvim_get_current_win() == target then
        restore_same_row()
        return
      end
    end
  end

  if move_local(dir_key, count) then
    restore_same_row()
    return
  end

  -- We're at a Neovim edge. Try to cross into Herdr.
  if not herdr.is_in_session() then
    if will_wrap and count == 1 then
      local sidebar = is_sidebar()
      if type(at_edge_behavior) == 'function' then
        at_edge_behavior({
          direction = direction,
          split = function() split_edge(direction) end,
          is_sidebar = sidebar,
          wrap = wrap_local,
        })
      elseif at_edge_behavior == 'stop' then
        return
      elseif at_edge_behavior == 'split' then
        if not sidebar then
          split_edge(direction)
        end
      else -- 'wrap' (default)
        wrap_local()
      end
    end
    return
  end

  -- Check zoom state: unzoom first, then retry Neovim navigation.
  -- Must happen BEFORE the at_herdr_edge check — when zoomed, the pane fills
  -- the screen so herdr reports it as being at every edge, making
  -- at_herdr_edge useless until we unzoom.
  if herdr.unzoom_enabled() and herdr.current_pane_is_zoomed() then
    herdr.unzoom()
    if embedded then
      geometry_win = win.resolve_embedded_parent(prev_win)
      if not geometry_win then
        return
      end
    end
    if move_local(dir_key, 1) then
      restore_same_row()
      return
    end
    -- Still at edge after unzoom; fall through to Herdr edge check.
  end

  -- Check if we're at the Herdr edge too
  local at_herdr_edge = herdr.current_pane_at_edge(direction)
  if at_herdr_edge == nil then
    if will_wrap and count == 1 then
      wrap_local()
    end
    return
  end

  if not at_herdr_edge then
    -- There's a Herdr pane in this direction. Cross the boundary.
    local moved = herdr.focus_pane(direction)
    if not moved and will_wrap and count == 1 then
      wrap_local()
    end
    return
  end

  -- At both Neovim AND Herdr edges (no herdr pane to cross into).
  -- Apply at_edge behavior. (Any needed unzoom already happened above.)
  if type(at_edge_behavior) == 'function' then
    at_edge_behavior({
      direction = direction,
      split = function() split_edge(direction) end,
      is_sidebar = is_sidebar(),
      wrap = wrap_local,
    })
  elseif at_edge_behavior == 'stop' then
    return
  elseif at_edge_behavior == 'split' then
    if not is_sidebar() then
      split_edge(direction)
    end
  else -- 'wrap' (default)
    if will_wrap and count == 1 then
      -- Wrap to the opposite side. If a Herdr pane exists there AND nav_at_edge
      -- allows wrap-across-boundary (the default), cross into it; otherwise
      -- wrap within Neovim. nav_at_edge=stop keeps the wrap inside Neovim.
      if herdr.nav_at_edge() ~= 'stop'
          and herdr.current_pane_at_edge(win.reverse_direction[direction]) == false
      then
        cross_wrap_pane()
      else
        wrap_local()
      end
    end
  end
end

return M
