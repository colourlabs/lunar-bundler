# lunar-bundler

A Lua bundler written in Rust. Resolves `require()` calls and bundles your Lua project into a single file. Supports Lua 5.1 (LuaJIT included) through 5.5 (not tested on Luau). Works anywhere and any project.

## installation

```bash
cargo install --path .
```

## demo

```bash
cd demo
cargo run --manifest-path ../Cargo.toml
lua bundle.lua
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

# resolve pure-lua luarocks modules
lunar-bundler src/main.lua -o bundle.lua --luarocks

# use a custom config file
lunar-bundler --config my-config.toml
```

## configuration

lunar-bundler looks for `lunar_bundler.toml` or `lunar_bundler.jsonc` in the current directory automatically. you can also specify a config file with `--config`.

### lunar_bundler.toml

```toml
[bundle]
entry = "src/main.lua"
output = "dist/bundle.lua"
lua_version = "54"
luarocks = false

[paths]
search = [
    "src",
    "lib",
    "vendor",
]

[inject]
top = "header.lua"
bottom = "footer.lua"

[resolve]
# mark modules as external so they are left as require() calls at runtime
externals = [
    "socket",
    "lunar/*",  # wildcard, matches lunar/router, lunar/middleware, etc
]

# override where specific modules resolve to
overrides = [
    { module = "json", path = "vendor/json/init.lua" },
    { module = "socket", path = "shims/socket.lua" },
]
```

### lunar_bundler.jsonc

```jsonc
{
    "bundle": {
        "entry": "src/main.lua",
        "output": "dist/bundle.lua",
        "lua_version": "54", // corresponds to Lua 5.4
        "luarocks": false
    },
    // search paths for require() resolution
    "paths": {
        "search": ["src", "lib", "vendor"]
    },
    "inject": {
        "top": "header.lua",
        "bottom": "footer.lua"
    },
    "resolve": {
        // modules left as require() calls at runtime
        "externals": ["socket", "lunar/*"],
        "overrides": [
            { "module": "json", "path": "vendor/json/init.lua" }
        ]
    }
}
```

## luarocks

lunar-bundler can resolve and bundle pure-Lua luarocks packages. enable it with `--luarocks` or in your config:

```toml
[bundle]
luarocks = true
```

lunar-bundler discovers luarocks paths by running `luarocks path --lr-path` and falls back to well-known install locations (`~/.luarocks`, `/usr/local/share/lua`, etc). luarocks must be installed for this to work.

native C modules (like `luasocket`, `luafilesystem`) cannot be bundled and are automatically treated as externals with a warning.

## how it works

lunar-bundler starts at your entry file, walks all `require()` calls recursively, and emits a single Lua file with a small runtime shim that replaces `require()`. each module is wrapped in a closure so locals don't leak between modules. the entry point is inlined at the bottom.

modules that aren't part of your project (like `require("socket")`) are left alone and resolved at runtime as normal. mark them as externals in your config to suppress the unresolved module error.

dynamic requires (`require(some_variable)`) are warned about but not errors - they are left intact for runtime resolution.

## limitations

- native C modules cannot be bundled (mainly due to [lunar](https://github.com/colourlabs/lunar) not supporting C modules due to security concerns)
- Lua 5.5 `global` declarations are stripped before parsing and not emitted in the bundle
- dynamic requires cannot be statically resolved and must be handled at runtime

## status

core bundling works. not yet published to crates.io due to `full-moon` dependency using a git fork for Lua 5.5 support.