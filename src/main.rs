use anyhow::Result;
use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use lunar_bundler::config::Config;
use lunar_bundler::loader::{command_loader, moonscript_loader, teal_loader};
use lunar_bundler::{BuildMode, Loader};

#[derive(Parser, Debug)]
#[command(name = "lunar-bundler")]
#[command(about = "a lua bundler written in rust")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// entry point lua file
    entry: Option<PathBuf>,

    /// output file (defaults to stdout if not specified)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// additional search paths for require() resolution
    #[arg(short = 'p', long = "path")]
    search_paths: Vec<PathBuf>,

    /// lua version (51, 52, 53, 54, 55)
    #[arg(long, default_value = "55")]
    lua_version: String,

    /// inject a file at the top of the bundle
    #[arg(long)]
    inject_top: Option<PathBuf>,

    /// inject a file at the bottom of the bundle
    #[arg(long)]
    inject_bottom: Option<PathBuf>,

    #[arg(long)]
    config: Option<PathBuf>,

    #[arg(short, long)]
    verbose: bool,

    #[arg(long)]
    luarocks: bool,

    /// build mode: development (default) or production
    #[arg(long, default_value = "development")]
    mode: String,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Scaffold a new project
    Init {
        /// target directory (defaults to current directory)
        path: Option<PathBuf>,

        /// project template (moonscript, teal, lua)
        #[arg(long, default_value = "lua")]
        template: String,
    },
}

fn scaffold(path: &std::path::Path, template: &str) -> Result<()> {
    match template {
        "moonscript" => scaffold_moonscript(path),
        "teal" => scaffold_teal(path),
        "lua" => scaffold_lua(path),
        _ => anyhow::bail!(
            "unknown template '{}'. available: moonscript, teal, lua",
            template
        ),
    }
}

fn scaffold_moonscript(path: &std::path::Path) -> Result<()> {
    let src = path.join("src");
    std::fs::create_dir_all(&src)?;

    std::fs::write(
        path.join("lunar_bundler.toml"),
        r#"[bundle]
entry = "src/main.lua"
output = "bundle.lua"

[resolve]
extensions = ["moon", "lua"]

[loaders]
commands = { moonscript = "moonc -" }

[[loaders.rules]]
test = "*.moon"
use = ["@moonscript"]
"#,
    )?;

    std::fs::write(
        src.join("main.lua"),
        r#"-- entry point
local Greeting = require("greeting")

local g = Greeting("world")
g:say()
"#,
    )?;

    std::fs::write(
        src.join("greeting.moon"),
        r#"class Greeting
  new: (name) =>
    @name = name
  say: =>
    print "Hello, #{@name}!"

{ :Greeting }
"#,
    )?;

    println!("scaffolded Moonscript project in {}", path.display());
    println!("  run `lunar-bundler` to bundle");
    Ok(())
}

fn scaffold_teal(path: &std::path::Path) -> Result<()> {
    let src = path.join("src");
    std::fs::create_dir_all(&src)?;

    std::fs::write(
        path.join("lunar_bundler.toml"),
        r#"[bundle]
entry = "src/main.lua"
output = "bundle.lua"

[resolve]
extensions = ["tl", "lua"]

[loaders]
commands = { teal = "tl gen -" }

[[loaders.rules]]
test = "*.tl"
use = ["teal"]
"#,
    )?;

    std::fs::write(
        src.join("main.lua"),
        r#"-- entry point
local greet = require("greeting")

greet.hello("world")
"#,
    )?;

    std::fs::write(
        src.join("greeting.tl"),
        r#"local function hello(name: string)
    print("Hello, " .. name)
end

return { hello = hello }
"#,
    )?;

    println!("scaffolded Teal project in {}", path.display());
    println!("  run `lunar-bundler` to bundle");
    Ok(())
}

fn scaffold_lua(path: &std::path::Path) -> Result<()> {
    let src = path.join("src");
    std::fs::create_dir_all(&src)?;

    std::fs::write(
        path.join("lunar_bundler.toml"),
        r#"[bundle]
entry = "src/main.lua"
output = "bundle.lua"

[paths]
search = ["."]
"#,
    )?;

    std::fs::write(
        src.join("main.lua"),
        r#"-- entry point
local greet = require("greeting")

greet.hello("world")
"#,
    )?;

    std::fs::write(
        src.join("greeting.lua"),
        r#"local function hello(name)
    print("Hello, " .. name)
end

return { hello = hello }
"#,
    )?;

    println!("scaffolded Lua project in {}", path.display());
    println!("  run `lunar-bundler` to bundle");
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Init { path, template }) => {
            let dir = path.unwrap_or_else(|| std::env::current_dir().unwrap());
            return scaffold(&dir, &template);
        }
        None => {}
    }

    let filter = if cli.verbose {
        EnvFilter::new("lunar_bundler=debug")
    } else {
        EnvFilter::from_default_env()
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    let config_path = cli.config.unwrap_or_else(|| {
        let toml = PathBuf::from("lunar_bundler.toml");
        let jsonc = PathBuf::from("lunar_bundler.jsonc");
        if toml.exists() {
            toml
        } else if jsonc.exists() {
            jsonc
        } else {
            toml
        }
    });

    let config = Config::load(&config_path)?;

    let entry = cli
        .entry
        .or_else(|| config.bundle.as_ref()?.entry.clone())
        .ok_or_else(|| anyhow::anyhow!("no entry file specified, consider checking the CLI args or lunar_bundler.jsonc/toml configuration"))?;

    let mut search_paths = cli.search_paths;
    if let Some(paths) = config.paths.as_ref().and_then(|p| p.search.clone()) {
        search_paths.extend(paths);
    }

    if let Some(parent) = entry.parent() {
        search_paths.insert(0, parent.to_path_buf());
    }

    let externals = config
        .resolve
        .as_ref()
        .and_then(|r| r.externals.clone())
        .unwrap_or_default();

    let overrides = config
        .resolve
        .as_ref()
        .and_then(|r| r.overrides.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|o| (o.module, o.path))
        .collect();

    let resolve_extensions = config
        .resolve
        .as_ref()
        .and_then(|r| r.extensions.clone())
        .unwrap_or_default();

    let luarocks = cli.luarocks
        || config
            .bundle
            .as_ref()
            .and_then(|b| b.luarocks)
            .unwrap_or(false);

    let mode = BuildMode::from_mode_str(&cli.mode);

    // Resolve config loaders into direct Loader closures
    let mut loaders: Vec<(String, Vec<Loader>)> = vec![];

    if let Some(loaders_config) = &config.loaders {
        let commands = loaders_config.commands.clone().unwrap_or_default();

        // Inline command-based rules (run: "command")
        if let Some(rules) = &loaders_config.rules {
            for rule in rules {
                // Check mode filter
                if let Some(rule_mode) = &rule.mode
                    && BuildMode::from_mode_str(rule_mode) != mode
                {
                    continue;
                }

                let mut loader_chain: Vec<Loader> = Vec::new();

                // Named loaders from commands map
                if let Some(names) = &rule.use_ {
                    for name in names {
                        if name.starts_with('@') {
                            // Built-in loader
                            match name.as_str() {
                                "@moonscript" => loader_chain.push(moonscript_loader()),
                                "@teal" => loader_chain.push(teal_loader()),
                                _ => tracing::warn!(
                                    "unknown built-in loader '{}' in rule '{}', skipping",
                                    name,
                                    rule.test
                                ),
                            }
                        } else if let Some(cmd) = commands.get(name) {
                            loader_chain.push(command_loader(cmd));
                        } else {
                            tracing::warn!(
                                "loader '{}' referenced in rule '{}' not found in [loaders.commands], skipping",
                                name,
                                rule.test
                            );
                        }
                    }
                }

                // Inline command
                if let Some(cmd) = &rule.run {
                    loader_chain.push(command_loader(cmd));
                }

                if !loader_chain.is_empty() {
                    loaders.push((rule.test.clone(), loader_chain));
                }
            }
        }
    }

    tracing::info!("starting bundle");
    println!(
        "{} bundling {} with config '{}'",
        "lunar_bundler".truecolor(30, 80, 180).bold(),
        entry.display().truecolor(30, 80, 180),
        config_path.display().truecolor(30, 80, 180),
    );

    let sandbox_level = config
        .sandbox
        .as_ref()
        .map(|sb| {
            sb.level.as_deref().unwrap_or("error").parse().unwrap_or_default()
        })
        .unwrap_or(lunar_bundler::sandbox::SandboxLevel::Off);

    let sandbox_deny = config
        .sandbox
        .as_ref()
        .and_then(|sb| sb.deny.clone())
        .unwrap_or_default();

    let compat_level = config
        .compat
        .as_ref()
        .map(|c| match c.level.as_deref().unwrap_or("error") {
            "warn" => lunar_bundler::compat::CompatLevel::Warn,
            "off" => lunar_bundler::compat::CompatLevel::Off,
            _ => lunar_bundler::compat::CompatLevel::Error,
        })
        .unwrap_or(lunar_bundler::compat::CompatLevel::Off);

    let compat_ignore = config
        .compat
        .as_ref()
        .and_then(|c| c.ignore.clone())
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s.parse::<lunar_bundler::compat::CompatIssueKind>().ok())
        .collect::<Vec<_>>();

    let result = lunar_bundler::bundle(lunar_bundler::BundleOptions {
        entry,
        search_paths,
        lua_version: cli.lua_version,
        inject_top: cli.inject_top,
        inject_bottom: cli.inject_bottom,
        externals,
        overrides,
        luarocks,
        resolve_extensions,
        mode: mode.clone(),
        loaders,
        sandbox_level,
        sandbox_deny,
        compat_level,
        compat_ignore,
    })?;

    println!(
        "{} bundled {} modules in {}",
        "lunar_bundler".truecolor(30, 80, 180).bold(),
        result.module_count.truecolor(30, 80, 180).bold(),
        format!("{}b", result.output.len()).truecolor(30, 80, 180),
    );

    let output = cli
        .output
        .or_else(|| config.bundle.as_ref()?.output.clone());

    match output {
        Some(path) => {
            std::fs::write(&path, &result.output)?;

            if mode == lunar_bundler::BuildMode::Development {
                let map_path = format!("{}.map", path.display());
                std::fs::write(&map_path, &result.sourcemap)?;
                println!(
                    "{} written to {}",
                    "sourcemap".truecolor(30, 80, 180).bold(),
                    map_path.truecolor(30, 80, 180),
                );
            }

            println!(
                "{} written to {}",
                "bundle".truecolor(30, 80, 180).bold(),
                path.display().truecolor(30, 80, 180),
            );
        }
        None => print!("{}", result.output),
    }

    Ok(())
}
