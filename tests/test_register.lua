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

T['publishes the editor token and clears it on suspend, resume and leave'] = function()
  local result = child.lua_func(function()
    vim.env.HERDR_ENV = '1'
    vim.env.HERDR_PANE_ID = 'pane-9'
    local calls = {}
    package.loaded['herdr-splits.rpc'] = {
      call = function(method, params)
        calls[#calls + 1] = { method = method, params = vim.deepcopy(params) }
        return { result = {} }
      end,
    }

    local register = require('herdr-splits.register')
    register.register()
    local publish = calls[1]

    vim.api.nvim_exec_autocmds('VimSuspend', {})
    local suspend_clears = calls[2].params.tokens.editor == vim.NIL

    vim.api.nvim_exec_autocmds('VimResume', {})
    local resume_reports = calls[3] and calls[3].params.tokens.editor == 'nvim'

    register.unregister()
    local unregister_clears = calls[4].params.tokens.editor == vim.NIL

    -- Outside a session: complete no-op (no reports, no autocmds armed).
    vim.env.HERDR_ENV = nil
    local before = #calls
    register.register()
    local outside_added = #calls - before

    return {
      publish = publish,
      suspend_clears = suspend_clears,
      resume_reports = resume_reports,
      unregister_clears = unregister_clears,
      outside_added = outside_added,
    }
  end)

  expect.equality(result.publish, {
    method = 'pane.report_metadata',
    params = {
      pane_id = 'pane-9',
      source = 'herdr-splits',
      tokens = { editor = 'nvim' },
      ttl_ms = 30000,
    },
  })
  expect.equality(result.suspend_clears, true)
  expect.equality(result.resume_reports, true)
  expect.equality(result.unregister_clears, true)
  expect.equality(result.outside_added, 0)
end

return T
