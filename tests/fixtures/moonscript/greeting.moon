utils = require "utils"

class Greeting
  new: (name) =>
    @name = name
  hello: =>
    print "Hello " .. utils.capitalize(@name)

Greeting
