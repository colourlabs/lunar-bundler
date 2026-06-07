local utils = require("src.utils")
local M = {}

function M.greet(name)
    utils.log("greeting: " .. name)
    print("hello, " .. name .. "!")
end

return M