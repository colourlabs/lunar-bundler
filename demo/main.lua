-- example app that requires from many sources

local greeter = require("src.greeter")
local config = require("src.config")

greeter.greet(config.get("name"))
greeter.greet(config.get("language"))
