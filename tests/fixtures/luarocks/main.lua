-- example app that uses a luarocks module for example
local argparse = require("argparse")

local parser = argparse("app", "a demo app bundled with lunar-bundler")
parser:argument("input", "input file")
parser:option("-o --output", "output file")
parser:option("-v --verbose", "enable verbose output")

local args = parser:parse()
print("input: " .. args.input)