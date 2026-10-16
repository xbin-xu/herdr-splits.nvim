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

T['roundtrips over a local pipe and fails soft on every error path'] = function()
  local result = child.lua_func(function()
    -- A real libuv server on a local socket/pipe: same transport the herdr
    -- server speaks, so the codec + timing get exercised for real.
    local is_win = vim.fn.has('win32') == 1
    local sock = is_win and ('herdr-splits-test-' .. vim.fn.getpid()) or (vim.fn.tempname() .. '.sock')

    local behaviors = {}
    local req = 0
    local srv = vim.uv.new_pipe(false)
    srv:bind(is_win and ('\\\\.\\pipe\\' .. sock) or sock)
    srv:listen(1, function()
      local conn = vim.uv.new_pipe(false)
      srv:accept(conn)
      local buf = ''
      conn:read_start(function(_, data)
        if not data then
          pcall(function()
            conn:close()
          end)
          return
        end
        buf = buf .. data
        if buf:find('\n', 1, true) then
          req = req + 1
          local behavior = behaviors[req]
          if behavior == 'ok' then
            conn:write('{"id":"x","result":{"hello":"world"}}\n')
          elseif behavior == 'split' then
            conn:write('{"id":"x","res')
            vim.defer_fn(function()
              conn:write('ult":{"split":true}}\n')
            end, 40)
          elseif behavior == 'garbage' then
            conn:write('not json\n')
          elseif behavior == 'close' then
            conn:close()
          end
          -- 'silent': hold the connection open, never reply
        end
      end)
    end)

    vim.env.HERDR_SOCKET_PATH = sock
    local rpc = require('herdr-splits.rpc')
    rpc.timeout_ms = 300

    behaviors[1] = 'ok'
    local normal = rpc.call('pane.current', {})
    behaviors[2] = 'split'
    local split = rpc.call('pane.current', {})
    behaviors[3] = 'garbage'
    local garbage = rpc.call('pane.current', {})
    behaviors[4] = 'close'
    local closed = rpc.call('pane.current', {})
    behaviors[5] = 'silent'
    local t0 = vim.uv.hrtime()
    local silent = rpc.call('pane.current', {})
    local silent_ms = (vim.uv.hrtime() - t0) / 1e6

    vim.env.HERDR_SOCKET_PATH = ''
    local no_env = rpc.call('pane.current', {})
    vim.env.HERDR_SOCKET_PATH = sock .. '-missing'
    local no_server = rpc.call('pane.current', {})

    srv:close()
    return {
      normal = normal and normal.result and normal.result.hello or '<nil>',
      split = split and split.result and split.result.split or '<nil>',
      garbage = garbage == nil and '<nil>' or 'decoded',
      closed = closed == nil and '<nil>' or 'decoded',
      silent = silent == nil and '<nil>' or 'decoded',
      silent_bounded = silent_ms >= 200 and silent_ms < 2000,
      no_env = no_env == nil and '<nil>' or 'decoded',
      no_server = no_server == nil and '<nil>' or 'decoded',
    }
  end)

  expect.equality(result, {
    normal = 'world',
    split = true,
    garbage = '<nil>',
    closed = '<nil>',
    silent = '<nil>',
    silent_bounded = true,
    no_env = '<nil>',
    no_server = '<nil>',
  })
end

T['returns an error reply as a table for the caller to inspect'] = function()
  local result = child.lua_func(function()
    local is_win = vim.fn.has('win32') == 1
    local sock = is_win and ('herdr-splits-test-err-' .. vim.fn.getpid()) or (vim.fn.fnameescape(vim.fn.tempname() .. '.sock'))
    local srv = vim.uv.new_pipe(false)
    srv:bind(is_win and ('\\\\.\\pipe\\' .. sock) or sock)
    srv:listen(1, function()
      local conn = vim.uv.new_pipe(false)
      srv:accept(conn)
      conn:read_start(function(_, data)
        if data then
          conn:write('{"id":"x","error":{"message":"boom"}}\n')
        end
      end)
    end)

    vim.env.HERDR_SOCKET_PATH = sock
    local rpc = require('herdr-splits.rpc')
    rpc.timeout_ms = 300
    local resp = rpc.call('pane.current', {})
    srv:close()
    return {
      is_table = type(resp) == 'table',
      error_message = resp and resp.error and resp.error.message or '<nil>',
    }
  end)

  expect.equality(result, { is_table = true, error_message = 'boom' })
end

return T
