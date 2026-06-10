local M = {}
function M.capitalize(s)
  return s:sub(1,1):upper() .. s:sub(2)
end
return M
