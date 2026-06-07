use anyhow::Result;
use clap::Parser;
use lunar_bundler::config::Config;
use std::path::PathBuf;

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

    #[arg(long, default_value = "lunar_bundler.toml")]
    config: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = Config::load(&args.config)?;

    let entry = args
        .entry
        .or_else(|| config.bundle.as_ref()?.entry.clone())
        .ok_or_else(|| anyhow::anyhow!("no entry file specified"))?;

    let mut search_paths = args.search_paths;
    if let Some(paths) = config.paths.as_ref().and_then(|p| p.search.clone()) {
        search_paths.extend(paths);
    }

    if let Some(parent) = entry.parent() {
        search_paths.insert(0, parent.to_path_buf());
    }

    let bundle = lunar_bundler::bundle(lunar_bundler::BundleOptions {
        entry,
        search_paths,
        lua_version: args.lua_version,
        inject_top: args.inject_top,
        inject_bottom: args.inject_bottom,
    })?;

    let output = args
        .output
        .or_else(|| config.bundle.as_ref()?.output.clone());

    match output {
        Some(path) => std::fs::write(&path, &bundle)?,
        None => print!("{}", bundle),
    }

    Ok(())
}
