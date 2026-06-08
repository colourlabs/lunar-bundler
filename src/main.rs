use anyhow::Result;
use clap::Parser;
use owo_colors::OwoColorize;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use lunar_bundler::config::Config;

#[derive(Parser, Debug)]
#[command(name = "lunar-bundler")]
#[command(about = "a lua bundler written in rust")]
struct Args {
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
}

fn main() -> Result<()> {
    let args: Args = Args::parse();

    let filter = if args.verbose {
        EnvFilter::new("lunar_bundler=debug")
    } else {
        EnvFilter::from_default_env()
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    let config_path = args.config.unwrap_or_else(|| {
        let toml = PathBuf::from("lunar_bundler.toml");
        let jsonc = PathBuf::from("lunar_bunlder.jsonc");
        if toml.exists() {
            toml
        } else if jsonc.exists() {
            jsonc
        } else {
            toml
        }
    });

    let config = Config::load(&config_path)?;

    let entry = args
        .entry
        .or_else(|| config.bundle.as_ref()?.entry.clone())
        .ok_or_else(|| anyhow::anyhow!("no entry file specified, consider checking the CLI args or lunar_bundler.jsonc/toml configuration"))?;

    let mut search_paths = args.search_paths;
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

    tracing::info!("starting bundle");
    println!(
        "{} bundling {} with config '{}'",
        "lunar_bundler".truecolor(30, 80, 180).bold(),
        entry.display().truecolor(30, 80, 180),
        config_path.display().truecolor(30, 80, 180),
    );

    let luarocks = args.luarocks
        || config
            .bundle
            .as_ref()
            .and_then(|b| b.luarocks)
            .unwrap_or(false);

    let result = lunar_bundler::bundle(lunar_bundler::BundleOptions {
        entry,
        search_paths,
        lua_version: args.lua_version,
        inject_top: args.inject_top,
        inject_bottom: args.inject_bottom,
        externals,
        overrides,
        luarocks,
    })?;

    println!(
        "{} bundled {} modules in {}",
        "lunar_bundler".truecolor(30, 80, 180).bold(),
        result.module_count.truecolor(30, 80, 180).bold(),
        format!("{}b", result.output.len()).truecolor(30, 80, 180),
    );

    let output = args
        .output
        .or_else(|| config.bundle.as_ref()?.output.clone());

    match output {
        Some(path) => {
            std::fs::write(&path, &result.output)?;
            println!(
                "{} written to {}",
                "lunar_bundler".truecolor(30, 80, 180).bold(),
                path.display().truecolor(30, 80, 180),
            );
        }
        None => print!("{}", result.output),
    }

    Ok(())
}
