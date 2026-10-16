---Pane registration — the reliable editor-detection path.
---
---`herdr pane process-info` cannot see Neovim when it is not a direct child
---of the pane shell (scoop shims, the MSYS posix-spawn proxy bash, wrappers
---like yazi/cmd.exe), and Windows has no tcgetpgrp equivalent for herdr to
---ask the kernel instead. So the editor registers itself with the server:
---setup() publishes a pane token (`editor=nvim`) via pane.report_metadata
---with a TTL, and the Herdr-side scripts read it back with pane.get.
---
---TTL + heartbeat replace the old marker-file + pid-liveness machinery:
---a crashed nvim's registration expires server-side (no stale files, no
---OpenProcess probing, no pid-reuse false positives), and a clean exit
---clears it immediately.
---
---Lifecycle: VimLeavePre / VimSuspend clear the token; VimResume
---re-publishes it, so chords are not forwarded into a suspended nvim sitting
---behind a shell. A nested nvim in the same pane simply wins the token
---until it exits (last heartbeat wins — no compare-and-delete to get wrong).
---Do NOT lazy-load the plugin that calls setup(): a lazily-registered token
---means detection silently misses this pane.
local M = {}

local rpc = require('herdr-splits.rpc')

-- Long enough to survive a stalled heartbeat, short enough that a crashed
-- nvim stops matching quickly.
local TTL_MS = 30000
local HEARTBEAT_MS = 10000
local TOKENS = { editor = 'nvim' }

local heartbeat

---Publish the token for the current pane. False outside a Herdr session or
---when the server is unreachable / rejects the report.
---@return boolean
local function report()
  local pane_id = vim.env.HERDR_PANE_ID
  if vim.env.HERDR_ENV ~= '1' or not pane_id or pane_id == '' then
    return false
  end
  local resp = rpc.call('pane.report_metadata', {
    pane_id = pane_id,
    source = 'herdr-splits',
    tokens = TOKENS,
    ttl_ms = TTL_MS,
  })
  return resp ~= nil and resp.error == nil
end

---Drop the token ahead of the TTL (JSON null deletes the key).
local function clear()
  local pane_id = vim.env.HERDR_PANE_ID
  if not pane_id or pane_id == '' then
    return
  end
  rpc.call('pane.report_metadata', {
    pane_id = pane_id,
    source = 'herdr-splits',
    tokens = { editor = vim.NIL },
  })
end

local function start_heartbeat()
  if heartbeat or not vim.uv then
    return
  end
  heartbeat = vim.uv.new_timer()
  if not heartbeat then
    return
  end
  -- vim.schedule: rpc.call blocks on vim.wait, which fast-context (libuv
  -- callback) code may not call directly.
  heartbeat:start(HEARTBEAT_MS, HEARTBEAT_MS, vim.schedule_wrap(function()
    report()
  end))
end

local function stop_heartbeat()
  if not heartbeat then
    return
  end
  pcall(function()
    heartbeat:stop()
  end)
  pcall(function()
    heartbeat:close()
  end)
  heartbeat = nil
end

---Remove this pane's registration and stop refreshing it. Safe to call
---outside a session; never throws.
function M.unregister()
  stop_heartbeat()
  pcall(clear)
end

---Register the current pane as running (this) Neovim, and arm the lifecycle
---autocmds + heartbeat. Best-effort: no-op outside a Herdr session, never
---throws.
function M.register()
  local ok, err = pcall(function()
    if not report() then
      return
    end
    local group = vim.api.nvim_create_augroup('herdr-splits-register', { clear = true })
    vim.api.nvim_create_autocmd({ 'VimLeavePre', 'VimSuspend' }, {
      group = group,
      callback = function()
        M.unregister()
      end,
    })
    vim.api.nvim_create_autocmd('VimResume', {
      group = group,
      callback = function()
        if report() then
          start_heartbeat()
        end
      end,
    })
    start_heartbeat()
  end)
  if not ok then
    vim.notify('herdr-splits: pane registration failed: ' .. tostring(err), vim.log.levels.DEBUG)
  end
end

return M
