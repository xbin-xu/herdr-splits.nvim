---Paths inside this plugin's own checkout, resolved from this file's
---location so they hold for any install method (lazy.nvim clone, dev link,
---Herdr-managed copy).
---@class HerdrSplitsPaths
local M = {}

---Root of the plugin checkout: the directory holding `lua/`, `scripts/`
---and `herdr-plugin.toml`.
---@return string
function M.root()
  local source = debug.getinfo(1, 'S').source -- "@<abs>/lua/herdr-splits/paths.lua"
  return vim.fn.fnamemodify(source:sub(2), ':p:h:h:h') -- up 3: file<-herdr-splits<-lua<-root
end

return M
