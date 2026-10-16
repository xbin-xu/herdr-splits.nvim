---Herdr control-socket RPC client (NDJSON over a libuv pipe).
---
---All Herdr interaction from the Neovim side goes through here. The herdr
---CLI is no longer spawned at navigation time: on Windows a CLI spawn costs
---~20ms per call (and a python helper ~50ms), while a socket roundtrip is
---~0.5ms on the same machine.
---
---Transport: named pipe on Windows — herdr derives the pipe name by
---prefixing `\\.\pipe\` to the socket path (herdrdev/herdr#1531) — unix
---socket everywhere else; libuv's pipe handles both. One request per
---connection, matching the server (it closes after one reply).
local M = {}

---Reply deadline. A hung server must not hang the editor: the old CLI spawn
---had no such bound either, but this path is synchronous inside key
---handling, so it gets one.
M.timeout_ms = 2000

---Sanity cap on a reply; larger replies are treated as garbage.
local MAX_REPLY = 1024 * 1024

---The pipe/socket path to connect to, or nil outside a Herdr session.
---@return string|nil
local function pipe_path()
  local sock = vim.env.HERDR_SOCKET_PATH
  if not sock or sock == '' then
    return nil
  end
  if vim.fn.has('win32') == 1 and sock:sub(1, 2) ~= '\\\\' then
    return '\\\\.\\pipe\\' .. sock
  end
  return sock
end

---Send one request and return the decoded reply, or nil on any failure:
---no socket in the environment, connect/write/read error, timeout, empty
---or malformed reply. A reply carrying an `error` field is returned as-is
---(callers distinguish "server said no" from "unreachable").
---@param method string RPC method (see `herdr api schema --json`)
---@param params table|nil
---@return table|nil
function M.call(method, params)
  local path = pipe_path()
  if not path then
    return nil
  end
  local payload = vim.json.encode({
    id = 'herdr-splits',
    method = method,
    params = params or {},
  }) .. '\n'
  local pipe = vim.uv.new_pipe(false)
  if not pipe then
    return nil
  end

  local buf = ''
  local done = false
  local closed = false
  local function finish()
    done = true
    if not closed then
      closed = true
      pcall(function()
        pipe:close()
      end)
    end
  end

  pipe:connect(path, function(cerr)
    if cerr then
      finish()
      return
    end
    pipe:write(payload, function(werr)
      if werr then
        finish()
        return
      end
      pipe:read_start(function(_, data)
        -- data == nil (with or without an error) is EOF: the server closes
        -- after one reply; decode whatever arrived.
        if data then
          buf = buf .. data
          if buf:find('\n', 1, true) or #buf >= MAX_REPLY then
            finish()
          end
        else
          finish()
        end
      end)
    end)
  end)

  vim.wait(M.timeout_ms, function()
    return done
  end)
  if not done then
    finish()
    return nil
  end

  local line = buf:match('^[^\n]*') or ''
  if line == '' then
    return nil
  end
  local ok, decoded = pcall(vim.json.decode, line)
  if not ok or type(decoded) ~= 'table' then
    return nil
  end
  return decoded
end

return M
