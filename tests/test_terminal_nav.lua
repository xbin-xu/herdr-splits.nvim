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
      child.lua([[
        vim.env.HERDR_ENV = '1'
        vim.env.HERDR_PANE_ID = 'pane-1'
        vim.env.HERDR_SPLITS_CONFIG = vim.fn.tempname() .. '.conf'
        -- Keep setup() off the control socket: these tests are about mappings.
        package.loaded['herdr-splits.rpc'] = { call = function() return { result = {} } end }
      ]])
    end,
    post_case = function()
      -- Kill the pty jobs before the child dies: on Windows every terminal is a
      -- ConPTY console, and a conhost whose parent disappears without the job
      -- being stopped survives as an orphan (nobody reads its output pipe any
      -- more, so it can even spin a core). Verified the hard way.
      pcall(function()
        child.lua([[
          for _, b in ipairs(vim.api.nvim_list_bufs()) do
            local job = vim.b[b].terminal_job_id
            if job then pcall(vim.fn.jobstop, job) end
          end
          vim.cmd('silent! %bwipeout!')
        ]])
      end)
      child.stop()
    end,
  },
})

---Open a terminal buffer (optionally with a filetype, i.e. what snacks does).
local function open_term(ft)
  return child.lua_func(function(filetype)
    local buf = vim.api.nvim_create_buf(false, true)
    if filetype then
      vim.bo[buf].filetype = filetype
    end
    vim.api.nvim_buf_call(buf, function()
      vim.fn.termopen(vim.o.shell)
    end)
    -- TermOpen attaches synchronously; also flush the scheduled re-attach.
    vim.wait(50, function() return false end)
    return buf
  end, ft)
end

---Buffer-local t-mode nav maps on `buf`, keyed by lhs.
local function t_maps(buf)
  return child.lua_func(function(b)
    local out = {}
    for _, m in ipairs(vim.api.nvim_buf_get_keymap(b, 't')) do
      if m.desc and m.desc:match('^Navigate ') then
        out[m.lhs:lower()] = { expr = m.expr, buffer = m.buffer }
      end
    end
    return out
  end, buf)
end

T['terminal_nav is off by default'] = function()
  child.lua("require('herdr-splits').setup({})")
  local buf = open_term('snacks_terminal')
  expect.equality(t_maps(buf), {})
end

T['filetypes allow-list scopes the maps'] = function()
  child.lua("require('herdr-splits').setup({ terminal_nav = { filetypes = { 'snacks_terminal' } } })")
  local snacks = open_term('snacks_terminal')
  local plain = open_term(nil)

  local snacks_maps = t_maps(snacks)
  expect.equality(vim.tbl_count(snacks_maps), 4)
  expect.equality(snacks_maps['<c-h>'], { expr = 1, buffer = snacks })
  expect.equality(snacks_maps['<c-j>'], { expr = 1, buffer = snacks })
  expect.equality(snacks_maps['<c-k>'], { expr = 1, buffer = snacks })
  expect.equality(snacks_maps['<c-l>'], { expr = 1, buffer = snacks })

  -- A plain :terminal keeps the chord (it belongs to the program inside).
  expect.equality(t_maps(plain), {})
end

T['skip_filetypes wins over filetypes'] = function()
  child.lua([[
    require('herdr-splits').setup({
      terminal_nav = { skip_filetypes = { 'toggleterm' } },
    })
  ]])
  expect.equality(t_maps(open_term('toggleterm')), {})
  expect.equality(vim.tbl_count(t_maps(open_term('run'))), 4)
end

T['maps follow the chords in the conf'] = function()
  -- Chords are instance policy: Herdr forwards them, so they live in the conf.
  child.lua([[vim.fn.writefile({
    'nav_key_left=alt+left',
    'nav_key_down=alt+down',
    'nav_key_up=alt+up',
    'nav_key_right=alt+right',
  }, vim.env.HERDR_SPLITS_CONFIG)]])
  child.lua([[
    require('herdr-splits').setup({
      terminal_nav = { filetypes = { 'snacks_terminal' } },
    })
  ]])
  local maps = t_maps(open_term('snacks_terminal'))
  expect.equality(vim.tbl_count(maps), 4)
  expect.equality(maps['<m-left>'] ~= nil, true)
  expect.equality(maps['<c-h>'], nil)
end

T['terminal_nav = true maps every pty terminal'] = function()
  child.lua("require('herdr-splits').setup({ terminal_nav = true })")
  expect.equality(vim.tbl_count(t_maps(open_term(nil))), 4)
  -- ... but never a non-terminal buffer.
  expect.equality(t_maps(child.lua_func(function()
    local buf = vim.api.nvim_create_buf(false, true)
    vim.bo[buf].filetype = 'snacks_terminal'
    return buf
  end)), {})
end

T['nav() keeps the chord in floats and moves in tiled windows'] = function()
  local res = child.lua_func(function()
    local plugin = require('herdr-splits')
    local calls = {}
    plugin.move_cursor_left = function()
      calls[#calls + 1] = 'left'
    end
    local term = require('herdr-splits.terminal')

    vim.cmd('split')
    local tiled = term.nav('left', '<C-h>')

    local win = vim.api.nvim_open_win(vim.api.nvim_create_buf(false, true), true, {
      relative = 'editor',
      row = 1,
      col = 1,
      width = 20,
      height = 5,
    })
    local kept = term.nav('left', '<C-h>')

    vim.wait(300, function() return #calls > 0 end)
    return {
      tiled = tiled,
      kept = kept,
      calls = calls,
      float = vim.api.nvim_win_get_config(win).relative,
    }
  end)
  expect.equality(res.tiled, nil)
  expect.equality(res.kept, '<C-h>')
  expect.equality(res.calls, { 'left' })
  expect.equality(res.float, 'editor')
end

T['setup() is idempotent'] = function()
  child.lua("require('herdr-splits').setup({ terminal_nav = { filetypes = { 'snacks_terminal' } } })")
  local buf = open_term('snacks_terminal')
  child.lua("require('herdr-splits').setup({ terminal_nav = { filetypes = { 'snacks_terminal' } } })")
  expect.equality(vim.tbl_count(t_maps(buf)), 4)
end

return T
