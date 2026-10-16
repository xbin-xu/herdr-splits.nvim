local T = MiniTest.new_set()
local expect = MiniTest.expect
local geometry = require('herdr-splits.geometry')

local function rect(id, x, y, w, h)
  return { id = id, x = x, y = y, w = w, h = h }
end

T['picks the nearest layer, preserving input order on ties'] = function()
  -- T layout: a full-height left; b top-right; c bottom-right.
  local rects = { rect('a', 0, 0, 10, 20), rect('b', 10, 0, 10, 10), rect('c', 10, 10, 10, 10) }
  expect.equality(geometry.candidates(rects, 'a', 'right', false), { 'b', 'c' })
  expect.equality(geometry.candidates(rects, 'c', 'left', false), { 'a' })
  expect.equality(geometry.candidates(rects, 'b', 'down', false), { 'c' })
  expect.equality(geometry.candidates(rects, 'b', 'up', false), {})
end

T['nearest layer wins over farther windows'] = function()
  -- a | b (touching) ...... d (far). One positive gap layer only.
  local rects = {
    rect('a', 0, 0, 10, 20),
    rect('b', 10, 0, 10, 10),
    rect('d', 30, 0, 10, 10),
  }
  expect.equality(geometry.candidates(rects, 'a', 'right', false), { 'b' })
  expect.equality(geometry.candidates(rects, 'b', 'right', false), { 'd' })
end

T['requires perpendicular overlap'] = function()
  -- b overlaps a's rows; c sits fully below a's rows.
  local rects = { rect('a', 0, 0, 10, 10), rect('b', 10, 0, 10, 10), rect('c', 10, 10, 10, 10) }
  expect.equality(geometry.candidates(rects, 'a', 'right', false), { 'b' })
end

T['wrap flips to the opposite layout boundary'] = function()
  -- Horizontal: a at the left boundary; b/c at the right boundary (x 10-19).
  local rects = {
    rect('a', 0, 0, 10, 20),
    rect('b', 10, 0, 10, 10),
    rect('c', 10, 10, 10, 10),
    rect('d', 0, 20, 10, 10), -- left boundary, but no perpendicular overlap with b/c
  }
  expect.equality(geometry.candidates(rects, 'a', 'left', true), { 'b', 'c' })
  expect.equality(geometry.candidates(rects, 'b', 'right', true), { 'a' })
  -- c already sits on the right boundary: right-wrap looks at the left
  -- boundary and takes only the row-overlapping window (a, not d).
  expect.equality(geometry.candidates(rects, 'c', 'right', true), { 'a' })

  -- Vertical stack: up wraps to the bottom, down wraps to the top.
  local stack = { rect('top', 0, 0, 10, 10), rect('mid', 0, 10, 10, 10), rect('bot', 0, 20, 10, 10) }
  expect.equality(geometry.candidates(stack, 'top', 'up', true), { 'bot' })
  expect.equality(geometry.candidates(stack, 'bot', 'down', true), { 'top' })
end

T['without wrap there are no candidates at the boundary'] = function()
  local rects = { rect('a', 0, 0, 10, 20), rect('b', 10, 0, 10, 10) }
  expect.equality(geometry.candidates(rects, 'a', 'left', false), {})
  expect.equality(geometry.candidates(rects, 'b', 'right', false), {})
end

T['returns nothing when the current rect is absent'] = function()
  local rects = { rect('a', 0, 0, 10, 10), rect('b', 10, 0, 10, 10) }
  expect.equality(geometry.candidates(rects, 'ghost', 'right', false), {})
  expect.equality(geometry.candidates(rects, 'ghost', 'right', true), {})
  expect.equality(geometry.candidates({}, 'a', 'right', false), {})
end

return T
