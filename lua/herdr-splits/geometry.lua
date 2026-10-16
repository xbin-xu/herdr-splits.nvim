---Directional candidate search over tiled rectangles.
---
---The tmux `select-pane` target rule, shared by Neovim-window navigation
---(nav.lua) and Herdr-pane navigation (herdr.lua): among the rectangles
---adjacent in a direction, the nearest layer (minimal positive gap) with
---perpendicular overlap; `wrap` flips the search to the opposite layout
---boundary. "Nearest layer" absorbs one-cell split separators without
---hardcoding their width.
---
---Pure — no vim/window/state access, callers adapt their own geometry into
---rects. Mirrors scripts/herdr_common.py `_pane_candidates`; the two are
---kept honest by tests/test_geometry.lua fixtures and a differential fuzz
---(both languages were fuzz-verified equivalent).
---@class HerdrSplitsGeometry
local M = {}

---@class HerdrSplitsRect
---@field id any
---@field x number left column
---@field y number top row
---@field w number width in cells
---@field h number height in cells

---Ids of the rectangles the tmux directional search considers from `cur`.
---@param rects HerdrSplitsRect[] in a stable caller-defined (tabpage /
---       layout) order — results preserve it
---@param cur any id of the current rectangle (must be present in `rects`)
---@param direction '"left"'|'"right"'|'"up"'|'"down"'
---@param wrap boolean flip to the opposite layout boundary
---@return any[] candidate ids
function M.candidates(rects, cur, direction, wrap)
  local cur_rect
  for _, r in ipairs(rects) do
    if r.id == cur then
      cur_rect = r
      break
    end
  end
  if not cur_rect then
    return {}
  end

  local horiz = direction == 'left' or direction == 'right'
  local fwd = direction == 'right' or direction == 'down'

  ---Project a rect: [s, e] inclusive along the movement axis, [ps, pe]
  ---across it.
  ---@return number s, number e, number ps, number pe
  local function project(r)
    if horiz then
      return r.x, r.x + r.w - 1, r.y, r.y + r.h - 1
    end
    return r.y, r.y + r.h - 1, r.x, r.x + r.w - 1
  end
  ---The edge facing the movement (tmux's edge comparison).
  local function near(s, e)
    return fwd and s or e
  end
  local cs, ce, cps, cpe = project(cur_rect)

  local out = {}
  if wrap then
    -- Opposite boundary: right/down wrap to the smallest start coordinate,
    -- left/up wrap to the largest stop coordinate.
    local boundary = fwd and math.huge or -math.huge
    for _, r in ipairs(rects) do
      local s, e = project(r)
      if fwd then
        boundary = math.min(boundary, near(s, e))
      else
        boundary = math.max(boundary, near(s, e))
      end
    end
    for _, r in ipairs(rects) do
      if r.id ~= cur then
        local s, e, ps, pe = project(r)
        if near(s, e) == boundary and ps <= cpe and pe >= cps then
          out[#out + 1] = r.id
        end
      end
    end
    return out
  end

  -- Nearest layer beyond the current rectangle: smallest positive gap.
  local best_gap
  for _, r in ipairs(rects) do
    if r.id ~= cur then
      local s, e, ps, pe = project(r)
      local gap = fwd and (s - ce) or (cs - e)
      if gap >= 1 and ps <= cpe and pe >= cps then
        if not best_gap or gap < best_gap then
          best_gap, out = gap, { r.id }
        elseif gap == best_gap then
          out[#out + 1] = r.id
        end
      end
    end
  end
  return out
end

return M
