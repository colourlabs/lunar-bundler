use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "lunar-bundler")]
#[command(about = "a lua bundler written in rust")]
struct Args {
    entry: PathBuf,

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
}

fn main() -> Result<()> {
    let args = Args::parse();

    if !args.entry.exists() {
        anyhow::bail!("entry file does not exist: {}", args.entry.display());
    }

    // build search paths: always include the entry file's directory
    let mut search_paths = args.search_paths.clone();
    if let Some(parent) = args.entry.parent() {
        search_paths.insert(0, parent.to_path_buf());
    }

    let bundle = lunar_bundler::bundle(lunar_bundler::BundleOptions {
        entry: args.entry,
        search_paths,
        lua_version: args.lua_version,
        inject_top: args.inject_top,
        inject_bottom: args.inject_bottom,
    })?;

    match args.output {
        Some(path) => std::fs::write(&path, &bundle)?,
        None => print!("{}", bundle),
    }

    Ok(())
}