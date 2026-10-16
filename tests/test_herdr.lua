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

return T
