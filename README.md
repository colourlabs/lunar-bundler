# lunar-bundler

A probably fast Lua bundler written in Rust. Resolves `require()` calls and bundles your Lua project into a single file. Supports Lua 5.1 through 5.5, (not tested on Luau)

## installation

```bash
cargo install --path .
```

## usage

```bash
# bundle to stdout
lunar-bundler src/main.lua

# bundle to a file
lunar-bundler src/main.lua -o bundle.lua

# with additional search paths
lunar-bundler src/main.lua -o bundle.lua -p ./lib -p ./vendor

# inject code at the top or bottom of the bundle
lunar-bundler src/main.lua -o bundle.lua --inject-top header.lua --inject-bottom footer.lua
```

## how it works

lunar-bundler starts at your entry file, walks all `require()` calls recursively, and emits a single Lua file with a small runtime shim that replaces `require()`. Each module is wrapped in a closure so locals don't leak between modules. the entry point is inlined at the bottom.

modules that aren't part of your project (like `require("socket")`) are left alone and resolved at runtime as normal.

## limitations (as of right now)

- dynamic requires (`require(some_variable)`) are not resolved and will produce an error
- native C modules cannot be bundled (this probably won't be supported [lunar](https://github.com/colourlabs/lunar) doesn't support C modules due to security concerns)
- lua 5.5 `global` declarations are stripped before parsing and not emitted in the bundle

## status

core bundling works. not yet published to crates.io or has LuaRocks module resolving yet.