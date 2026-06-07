pub mod bundler;
pub mod emitter;
pub mod error;
pub mod graph;
pub mod preprocessor;
pub mod resolver;
pub mod config;
pub mod scanner;

pub struct BundleOptions {
    pub entry: std::path::PathBuf,
    pub search_paths: Vec<std::path::PathBuf>,
    pub lua_version: String,
    pub inject_top: Option<std::path::PathBuf>,
    pub inject_bottom: Option<std::path::PathBuf>,
}

pub fn bundle(opts: BundleOptions) -> anyhow::Result<String> {
    // read injection files if provided
    let inject_top = opts.inject_top
        .map(|p| std::fs::read_to_string(p))
        .transpose()?;

    let inject_bottom = opts.inject_bottom
        .map(|p| std::fs::read_to_string(p))
        .transpose()?;

    bundler::bundle(bundler::BundlerOptions {
        entry: opts.entry,
        search_paths: opts.search_paths,
        inject_top,
        inject_bottom,
    })
}