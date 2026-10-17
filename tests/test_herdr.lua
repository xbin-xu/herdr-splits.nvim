local T
local expect = MiniTest.expect
local child = MiniTest.new_child_neovim()

local source = debug.getinfo(1, 'S').source
local root = vim.fn.fnamemodify(source:sub(2), ':p:h:h')

T = MiniTest.new_set({
  hooks = {
    pre_case = function()
      child.restart({ '--noplugin', '-u', 'NONE', '-i', 'NONE' })
      child.cmd('set runtimepath^=' .. vim.fn.fnameescape(root))
    end,
    post_case = function()
      child.stop()
    end,
  },
})

T['resolves the binary and requires a complete session environment'] = function()
  local result = child.lua_func(function()
    local config = require('herdr-splits.config')
    local herdr = require('herdr-splits.herdr')

    vim.env.HERDR_BIN_PATH = '/env/herdr'
    config.herdr_bin = nil
    local from_env = herdr.herdr_bin()
    config.herdr_bin = '/config/herdr'
    local from_config = herdr.herdr_bin()
    config.herdr_bin = nil
    vim.env.HERDR_BIN_PATH = nil
    local fallback = herdr.herdr_bin()

    vim.env.HERDR_ENV = nil
    vim.env.HERDR_PANE_ID = nil
    local absent = { herdr.is_in_session(), herdr.current_pane_id() or '<nil>' }
    vim.env.HERDR_ENV = '1'
    vim.env.HERDR_PANE_ID = ''
    local empty = { herdr.is_in_session(), herdr.current_pane_id() or '<nil>' }
    vim.env.HERDR_PANE_ID = 'pane-7'
    local present = { herdr.is_in_session(), herdr.current_pane_id() }

    return {
      binaries = { from_env, from_config, fallback },
      absent = absent,
      empty = empty,
      present = present,
    }
  end)

  expect.equality(result, {
    binaries = { '/env/herdr', '/config/herdr', 'herdr' },
    absent = { false, '<nil>' },
    empty = { false, '<nil>' },
    present = { true, 'pane-7' },
  })
end

T['parses edge responses and rejects invalid replies'] = function()
  local result = child.lua_func(function()
    vim.env.HERDR_ENV = '1'
    vim.env.HERDR_PANE_ID = 'pane-7'

    local NIL_REPLY = {} -- sentinel: a literal nil would punch a hole in the queue
    local queue = {
      { result = { edges = { left = true } } },
      { result = { edges = { left = false } } },
      NIL_REPLY, -- transport failure (rpc.call returns nil)
      { error = { message = 'boom' } },
      { result = {} },
      { result = { edges = {} } },
      { result = { edges = { left = vim.NIL } } },
      { result = { edges = { left = 'yes' } } },
    }
    local calls = {}
    package.loaded['herdr-splits.rpc'] = {
      call = function(method, params)
        calls[#calls + 1] = { method = method, params = vim.deepcopy(params) }
        local item = table.remove(queue, 1)
        if item == NIL_REPLY then
          return nil
        end
        return item
      end,
    }

    package.loaded['herdr-splits.herdr'] = nil
    local herdr = require('herdr-splits.herdr')
    local values = {}
    for i = 1, 8 do
      local value = herdr.current_pane_at_edge('left')
      values[i] = value == nil and '<nil>' or value
    end
    return { values = values, calls = calls }
  end)

  expect.equality(result.values, {
    true, false, '<nil>', '<nil>', '<nil>', '<nil>', '<nil>', '<nil>',
  })
  expect.equality(#result.calls, 8)
  for _, call in ipairs(result.calls) do
    expect.equality(call, { method = 'pane.edges', params = { pane_id = 'pane-7' } })
  end
end

T['parses zoom responses and rejects invalid replies'] = function()
  local result = child.lua_func(function()
    vim.env.HERDR_ENV = '1'
    vim.env.HERDR_PANE_ID = 'pane-7'
    local NIL_REPLY = {}
    local queue = {
      { result = { layout = { zoomed = true } } },
      { result = { layout = { zoomed = false } } },
      NIL_REPLY,
      { error = { message = 'boom' } },
      { result = {} },
      { result = { layout = {} } },
      { result = { layout = { zoomed = vim.NIL } } },
      { result = { layout = { zoomed = 1 } } },
    }
    local calls = {}
    package.loaded['herdr-splits.rpc'] = {
      call = function(method, params)
        calls[#calls + 1] = { method = method, params = vim.deepcopy(params) }
        local item = table.remove(queue, 1)
        if item == NIL_REPLY then
          return nil
        end
        return item
      end,
    }

    package.loaded['herdr-splits.herdr'] = nil
    local herdr = require('herdr-splits.herdr')
    local values = {}
    for i = 1, 8 do
      local value = herdr.current_pane_is_zoomed()
      values[i] = value == nil and '<nil>' or value
    end
    return { values = values, calls = calls }
  end)

  expect.equality(result.values, {
    true, false, '<nil>', '<nil>', '<nil>', '<nil>', '<nil>', '<nil>',
  })
  expect.equality(#result.calls, 8)
  for _, call in ipairs(result.calls) do
    expect.equality(call, { method = 'pane.layout', params = {} })
  end
end

T['focus, unzoom and resize map to RPC calls and report errors'] = function()
  local result = child.lua_func(function()
    vim.env.HERDR_ENV = '1'
    vim.env.HERDR_PANE_ID = 'pane-7'
    local queue = {
      { result = {} }, -- focus right: ok
      { error = { message = 'no neighbor' } }, -- focus left: error reply
      { result = {} }, -- unzoom: ok
      { error = { message = 'failed' } }, -- unzoom: error reply
      { result = {} }, -- resize: ok
      { error = { message = 'failed' } }, -- resize: error reply
    }
    local calls = {}
    package.loaded['herdr-splits.rpc'] = {
      call = function(method, params)
        calls[#calls + 1] = { method = method, params = vim.deepcopy(params) }
        return table.remove(queue, 1)
      end,
    }

    package.loaded['herdr-splits.herdr'] = nil
    local herdr = require('herdr-splits.herdr')
    return {
      values = {
        herdr.focus_pane('right'),
        herdr.focus_pane('left'),
        herdr.unzoom(),
        herdr.unzoom(),
        herdr.resize_pane('down', 0.03),
        herdr.resize_pane('left', 1 / 3),
      },
      calls = calls,
    }
  end)

  expect.equality(result.values, { true, false, true, false, true, false })
  expect.equality(result.calls, {
    { method = 'pane.focus_direction', params = { pane_id = 'pane-7', direction = 'right' } },
    { method = 'pane.focus_direction', params = { pane_id = 'pane-7', direction = 'left' } },
    { method = 'pane.zoom', params = { pane_id = 'pane-7', mode = 'off' } },
    { method = 'pane.zoom', params = { pane_id = 'pane-7', mode = 'off' } },
    { method = 'pane.resize', params = { pane_id = 'pane-7', direction = 'down', amount = 0.03 } },
    { method = 'pane.resize', params = { pane_id = 'pane-7', direction = 'left', amount = 1 / 3 } },
  })
end

T['most_recently_focused picks the most recent adjacent pane via the shared mru file'] = function()
  local result = child.lua_func(function()
    vim.env.HERDR_ENV = '1'
    vim.env.HERDR_PANE_ID = 'p1'
    local tmpdir = vim.fn.tempname()
    vim.fn.mkdir(tmpdir, 'p')
    vim.env.HERDR_SPLITS_CONFIG = tmpdir .. '/herdr-splits.conf'
    local mru = tmpdir .. '/mru-panes'

    -- T layout: p1 full height left; p2 top right; p3 bottom right.
    local function tl_layout(zoomed)
      return {
        result = {
          layout = {
            zoomed = zoomed or false,
            panes = {
              { pane_id = 'p1', rect = { x = 0, y = 0, width = 40, height = 20 } },
              { pane_id = 'p2', rect = { x = 40, y = 0, width = 40, height = 10 } },
              { pane_id = 'p3', rect = { x = 40, y = 10, width = 40, height = 10 } },
            },
          },
        },
      }
    end

    local function read_mru()
      local f = io.open(mru, 'r')
      if not f then
        return '<missing>'
      end
      local out = f:read('*a')
      f:close()
      return out
    end

    -- No history: deterministic first candidate (server order) wins.
    local queue_a = { tl_layout(), { result = {} } }
    local calls_a = {}
    package.loaded['herdr-splits.rpc'] = {
      call = function(method, params)
        calls_a[#calls_a + 1] = { method = method, params = vim.deepcopy(params) }
        return table.remove(queue_a, 1)
      end,
    }
    require('herdr-splits.config').most_recently_focused = true
    package.loaded['herdr-splits.herdr'] = nil
    local herdr = require('herdr-splits.herdr')

    local first_ok = herdr.focus_pane('right')
    local after_first = read_mru()

    -- History p3 > p2: the most recent candidate (p3) wins.
    local f = io.open(mru, 'w')
    f:write('p3\np2\n')
    f:close()
    local queue_b = { tl_layout(), { result = {} } }
    local calls_b = {}
    package.loaded['herdr-splits.rpc'].call = function(method, params)
      calls_b[#calls_b + 1] = { method = method, params = vim.deepcopy(params) }
      return table.remove(queue_b, 1)
    end

    local second_ok = herdr.focus_pane('right')
    local after_second = read_mru()

    -- Zoomed with auto-unzoom disabled: geometric fallback, no pick.
    require('herdr-splits.config').unzoom_on_nav = false
    local queue_c = { { result = { layout = { zoomed = true, panes = {} } } }, { result = {} } }
    local calls_c = {}
    package.loaded['herdr-splits.rpc'].call = function(method, params)
      calls_c[#calls_c + 1] = { method = method, params = vim.deepcopy(params) }
      return table.remove(queue_c, 1)
    end
    local third_ok = herdr.focus_pane('right')

    -- Zoomed with auto-unzoom enabled: unzooms, re-reads, then picks.
    require('herdr-splits.config').unzoom_on_nav = true
    local queue_d = {
      { result = { layout = { zoomed = true, panes = { { pane_id = 'p1', rect = { x = 0, y = 0, width = 80, height = 20 } } } } } },
      { result = {} }, -- pane.zoom off
      tl_layout(),
      { result = {} }, -- pane.focus
    }
    local calls_d = {}
    package.loaded['herdr-splits.rpc'].call = function(method, params)
      calls_d[#calls_d + 1] = { method = method, params = vim.deepcopy(params) }
      return table.remove(queue_d, 1)
    end
    local fourth_ok = herdr.focus_pane('right')

    vim.fn.delete(tmpdir, 'rf')
    return {
      first = { first_ok, after_first, calls_a },
      second = { second_ok, after_second, calls_b },
      third = { third_ok, calls_c },
      fourth = { fourth_ok, calls_d },
    }
  end)

  expect.equality(result.first[1], true)
  expect.equality(result.first[2], 'p2\np1\n')
  expect.equality(result.first[3], {
    { method = 'pane.layout', params = {} },
    { method = 'pane.focus', params = { pane_id = 'p2' } },
  })
  expect.equality(result.second[1], true)
  expect.equality(result.second[2], 'p3\np1\np2\n')
  expect.equality(result.second[3], {
    { method = 'pane.layout', params = {} },
    { method = 'pane.focus', params = { pane_id = 'p3' } },
  })
  -- Zoomed, unzoom disabled: layout read, then geometric fallback.
  expect.equality(result.third, {
    true,
    {
      { method = 'pane.layout', params = {} },
      { method = 'pane.focus_direction', params = { pane_id = 'p1', direction = 'right' } },
    },
  })
  -- Zoomed, unzoom enabled: unzoom, re-read, pick, bump the mru file.
  expect.equality(result.fourth, {
    true,
    {
      { method = 'pane.layout', params = {} },
      { method = 'pane.zoom', params = { pane_id = 'p1', mode = 'off' } },
      { method = 'pane.layout', params = {} },
      { method = 'pane.focus', params = { pane_id = 'p3' } },
    },
  })
end

T['wrap-cross flips to the opposite-boundary panes, not the reverse-nearest'] = function()
  local result = child.lua_func(function()
    vim.env.HERDR_ENV = '1'
    vim.env.HERDR_PANE_ID = 'A'
    local tmpdir = vim.fn.tempname()
    vim.fn.mkdir(tmpdir, 'p')
    vim.env.HERDR_SPLITS_CONFIG = tmpdir .. '/conf'
    local f = io.open(tmpdir .. '/mru-panes', 'w')
    f:write('D\nB\n') -- D more recent than B
    f:close()

    -- A full height left; B top right; C/D split the bottom right.
    local layout = {
      result = {
        layout = {
          zoomed = false,
          panes = {
            { pane_id = 'A', rect = { x = 0, y = 0, width = 40, height = 20 } },
            { pane_id = 'B', rect = { x = 40, y = 0, width = 40, height = 10 } },
            { pane_id = 'C', rect = { x = 40, y = 10, width = 20, height = 10 } },
            { pane_id = 'D', rect = { x = 60, y = 10, width = 20, height = 10 } },
          },
        },
      },
    }
    local calls = {}
    package.loaded['herdr-splits.rpc'] = {
      call = function(method, params)
        calls[#calls + 1] = { method = method, params = vim.deepcopy(params) }
        if method == 'pane.layout' then
          return layout
        end
        return { result = {} }
      end,
    }
    require('herdr-splits.config').most_recently_focused = true
    package.loaded['herdr-splits.herdr'] = nil
    local herdr = require('herdr-splits.herdr')

    local ok = herdr.focus_pane_wrap('left')
    vim.fn.delete(tmpdir, 'rf')
    return { ok = ok, calls = calls }
  end)

  -- Wrapping left from A must target the right-boundary panes {B, D} (the
  -- tmux flip) — not the reverse-nearest {B, C} — and the most recent (D)
  -- wins.
  expect.equality(result.ok, true)
  expect.equality(result.calls, {
    { method = 'pane.layout', params = {} },
    { method = 'pane.focus', params = { pane_id = 'D' } },
  })
end

T['searches the pre-zoom layout when auto-unzoom is disabled'] = function()
  local result = child.lua_func(function()
    vim.env.HERDR_ENV = '1'
    vim.env.HERDR_PANE_ID = 'A'
    local tmpdir = vim.fn.tempname()
    vim.fn.mkdir(tmpdir, 'p')
    vim.env.HERDR_SPLITS_CONFIG = tmpdir .. '/conf'

    local zoomed_layout = {
      result = {
        layout = {
          zoomed = true,
          panes = {
            { pane_id = 'A', rect = { x = 0, y = 0, width = 40, height = 20 } },
            { pane_id = 'B', rect = { x = 40, y = 0, width = 40, height = 20 } },
          },
        },
      },
    }
    local calls = {}
    package.loaded['herdr-splits.rpc'] = {
      call = function(method, params)
        calls[#calls + 1] = { method = method, params = vim.deepcopy(params) }
        if method == 'pane.layout' then
          return zoomed_layout
        end
        return { result = {} }
      end,
    }
    local config = require('herdr-splits.config')
    config.most_recently_focused = true
    config.unzoom_on_nav = false
    config.nav_at_edge = 'wrap'
    package.loaded['herdr-splits.herdr'] = nil
    local herdr = require('herdr-splits.herdr')

    -- C-h at the left Herdr edge wraps to B. Herdr keeps reporting the
    -- pre-zoom tiling, so the search needs no unzoom.
    local ok = herdr.focus_pane_wrap('left')
    vim.fn.delete(tmpdir, 'rf')
    return { ok = ok, calls = calls }
  end)

  expect.equality(result, {
    ok = true,
    calls = {
      { method = 'pane.layout', params = {} },
      { method = 'pane.focus', params = { pane_id = 'B' } },
    },
  })
end

return T
