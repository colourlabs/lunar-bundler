local utils = require("src.utils")
local M = {}

local store = {
    name = "world",
    language = "lua",
}

function M.get(key)
    utils.log("fetching config key: " .. key)
    return store[key]
end

return M