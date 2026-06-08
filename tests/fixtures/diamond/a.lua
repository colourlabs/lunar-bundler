local c = require("c")
local M = {}
function M.hello()
    c.greet("a")
end
return M