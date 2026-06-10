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

# scaffold a new project
lunar-bundler init                  # default: Lua template
lunar-bundler init --template lua   # lua template
lunar-bundler init --template teal  # requires `tl`
lunar-bundler init my-project       # scaffold in a subdirectory
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
    "overrides": [{ "module": "json", "path": "vendor/json/init.lua" }]
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

## sandbox

lunar-bundler can scan bundled modules for use of dangerous globals (`os.execute`, `io.open`, `dofile`, etc). enable it in config:

```toml
[sandbox]
# "error" (default) fails the build, "warn" prints warnings
level = "error"
deny = ["os", "io", "debug", "package", "dofile", "loadfile", "load"]
```

when `level = "error"`, the build stops with all violations listed.
when `level = "warn"`, violations are printed as warnings and the build continues.

the checker uses the full-moon AST parser, so it won't flag names inside string literals or local variable declarations (`local os = {}` is fine). it catches:

| pattern            | example                                          |
| ------------------ | ------------------------------------------------ |
| direct calls       | `dofile("x")` / `loadfile("x")` / `load("code")` |
| table method calls | `os.execute("rm")` / `io.open("file")`           |
| value reads        | `print(os)` / `local x = io`                     |
| nested calls       | `os.execute(io.open("file"))`                    |

## compatibility checks

lunar-bundler can scan bundled modules for Lua features that aren't available in the target version. enable it in config:

```toml
[compat]
# "error" (default) fails the build, "warn" prints warnings
level = "warn"
# optionally skip specific checks
ignore = ["GotoUsed", "BitwiseOps"]
```

the checker detects:

| kind                  | detects                                                 | supported in |
| --------------------- | ------------------------------------------------------- | ------------ |
| `GotoUsed`            | `goto label` / `::label::`                              | 5.2+         |
| `ConstAttribute`      | `local x <const> = 1`                                   | 5.4+         |
| `ToBeClosedAttribute` | `local f <close> = io.open()`                           | 5.4+         |
| `IntegerDivision`     | `x // y`                                                | 5.3+         |
| `BitwiseOps`          | `x & y`, `x \| y`, `x ~ y`, `x << y`, `x >> y`          | 5.3+         |
| `BitwiseNot`          | `~x`                                                    | 5.3+         |
| `Utf8Library`         | `utf8.len()`, `require("utf8")`                         | 5.3+         |
| `TableMove`           | `table.move()`                                          | 5.3+         |
| `StringPack`          | `string.pack()`, `string.unpack()`, `string.packsize()` | 5.3+         |
| `MathTointeger`       | `math.tointeger()`                                      | 5.3+         |
| `MathType`            | `math.type()`                                           | 5.3+         |
| `FfiLibrary`          | `require("ffi")`                                        | LuaJIT only  |
| `BitLibrary`          | `require("bit")`                                        | LuaJIT only  |
| `JitLibrary`          | `require("jit")`                                        | LuaJIT only  |

the check runs against `bundle.lua_version` (passed via `--lua-version` CLI flag or `[bundle] lua_version` in config). for example, setting `--lua-version 51` will flag all Lua 5.2+ features.

## build modes

lunar-bundler supports `development` (default) and `production` build modes:

```bash
lunar-bundler src/main.lua -o bundle.lua --mode=production
```

or in config:

```toml
[bundle]
mode = "production"
```

## loaders

loaders transform files before bundling. useful for transpilers like moonscript or teal.

### built-in loaders (`@name`)

these loaders use temp files for cross-platform support (no stdin pipe reliance):

| loader        | transpiler | install                       |
| ------------- | ---------- | ----------------------------- |
| `@moonscript` | `moonc`    | `luarocks install moonscript` |
| `@teal`       | `tl`       | `luarocks install teal`       |

```toml
[resolve]
extensions = ["moon", "lua"]

[[loaders.rules]]
test = "*.moon"
use = ["@moonscript"]

[[loaders.rules]]
test = "*.tl"
use = ["@teal"]
```

### command loaders

you can also reference named shell commands from `[loaders.commands]`:

```toml
[loaders]
commands = { moonscript = "moonc -", teal = "tl gen -" }

[[loaders.rules]]
test = "*.moon"
use = ["moonscript"]

[[loaders.rules]]
test = "*.tl"
use = ["teal"]

# only run in production
[[loaders.rules]]
test = "*.lua"
use = ["strip-comments"]
mode = "production"
```

or inline with `run`:

```toml
[[loaders.rules]]
test = "*.moon"
run = "moonc -"
```

custom file extensions can be added to require() resolution:

```toml
[resolve]
extensions = ["moon", "tl", "lua"]
```

## moonscript guide

moonscript is a language that compiles to Lua. with lunar-bundler's loader system, you can import `.moon` files seamlessly from Lua using `require()`.

### 1. install moonscript

```bash
luarocks install moonscript
```

### 2. project structure

```
my-project/
├── src/
│   ├── main.lua           # entry point, requires("greeting")
│   └── greeting.moon      # Moonscript source
└── lunar_bundler.toml
```

### 3. configure the loader

```toml
# lunar_bundler.toml
[bundle]
entry = "src/main.lua"
output = "dist/bundle.lua"

[resolve]
extensions = ["moon", "lua"]

[[loaders.rules]]
test = "*.moon"
use = ["@moonscript"]
```

the `@moonscript` built-in loader uses temp files so it works on all platforms (no stdin pipe). the `extensions` key tells the resolver to try `.moon` files when resolving `require()` calls. without it, `require("greeting")` would only look for `greeting.lua`.

### 4. write your moonscript

```moonscript
-- src/greeting.moon
class Greeting
  new: (name) =>
    @name = name
  say: =>
    print "Hello, #{@name}!"

{ :Greeting }
```

### 5. require it from lua

```lua
-- src/main.lua
local Greeting = require("greeting")
local g = Greeting("world")
g:say()
```

### 6. bundle

```bash
lunar-bundler
```

the output bundle will contain the compiled Lua (not raw moonscript), so no moonscript runtime is needed.

### troubleshooting

- **`moonc` not found**: install moonscript via `luarocks install moonscript`.
- **`.moon` files not resolved**: make sure `extensions = ["moon", "lua"]` is set in `[resolve]`.

## scaffolding

the `init` subcommand scaffolds a new project:

```bash
lunar-bundler init                   # default: Lua template
lunar-bundler init --template lua    # plain Lua
lunar-bundler init --template teal   # Teal with @teal loader
lunar-bundler init --template moonscript # Moonscript with @moonscript loader
lunar-bundler init my-project        # scaffold in a subdirectory
```

each template generates a `lunar_bundler.toml`, entry point, and a sample module.

## source maps

in development mode, lunar-bundler emits a `.map` file alongside the bundle and injects a `sourceMappingURL` comment. errors will reference original source files instead of bundle line numbers.

source maps are automatically disabled in production mode.

## how it works

lunar-bundler starts at your entry file, walks all `require()` calls recursively, and emits a single Lua file with a small runtime shim that replaces `require()`. each module is wrapped in a closure so locals don't leak between modules. the entry point is inlined at the bottom.

modules that aren't part of your project (like `require("socket")`) are left alone and resolved at runtime as normal. mark them as externals in your config to suppress the unresolved module error.

dynamic requires (`require(some_variable)`) are warned about but not errors - they are left intact for runtime resolution.

## limitations

- native C modules cannot be bundled
- Lua 5.5 `global` declarations are stripped before parsing and not emitted in the bundle
- dynamic requires cannot be statically resolved and must be handled at runtime

## status

core bundling works. not yet published to crates.io due to `full-moon` dependency using a git fork for Lua 5.5 support.
