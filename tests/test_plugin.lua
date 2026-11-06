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

T['public module loads its expected API'] = function()
  local result = child.lua_func(function()
    local plugin = require('herdr-splits')
    local names = {
      'setup',
      'reload',
      'setup_terminal_nav',
      'sync_herdr',
      'resize_left',
      'resize_down',
      'resize_up',
      'resize_right',
      'move_cursor_left',
      'move_cursor_down',
      'move_cursor_up',
      'move_cursor_right',
      'add_ignored_filetype',
      'add_ignored_buftype',
    }
    local types = {}
    for _, name in ipairs(names) do
      types[name] = type(plugin[name])
    end
    return types
  end)

  expect.equality(result, {
    setup = 'function',
    -- Documented in the Lua-API section and used by `:HerdrSplitsReload`.
    reload = 'function',
    setup_terminal_nav = 'function',
    sync_herdr = 'function',
    resize_left = 'function',
    resize_down = 'function',
    resize_up = 'function',
    resize_right = 'function',
    move_cursor_left = 'function',
    move_cursor_down = 'function',
    move_cursor_up = 'function',
    move_cursor_right = 'function',
    add_ignored_filetype = 'function',
    add_ignored_buftype = 'function',
  })
end

T['the reload command uses the public API and reports the conf path'] = function()
  local result = child.lua_func(function()
    vim.env.HERDR_SPLITS_CONFIG = vim.fn.tempname() .. '.conf'
    local notices = {}
    vim.notify = function(msg)
      notices[#notices + 1] = tostring(msg)
    end
    -- The plugin bootstrap defines the command; source it like a real start.
    vim.cmd('runtime! plugin/herdr-splits.lua')
    local defined = vim.fn.exists(':HerdrSplitsReload')
    vim.cmd('HerdrSplitsReload')
    return { defined = defined, notices = notices }
  end)

  expect.equality(result.defined, 2)
  expect.equality(#result.notices, 1)
  expect.equality(result.notices[1]:find('conf:', 1, true) ~= nil, true)
end

return T
