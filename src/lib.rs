pub mod bundler;
pub mod config;
pub mod emitter;
pub mod error;
pub mod graph;
pub mod luarocks;
pub mod preprocessor;
pub mod resolver;
pub mod scanner;

#[derive(Default)]
pub struct BundleOptions {
    pub entry: std::path::PathBuf,
    pub search_paths: Vec<std::path::PathBuf>,
    pub lua_version: String,
    pub inject_top: Option<std::path::PathBuf>,
    pub inject_bottom: Option<std::path::PathBuf>,
    pub externals: Vec<String>,
    pub overrides: Vec<(String, std::path::PathBuf)>,
    pub luarocks: bool,
}

pub struct BundleResult {
    pub output: String,
    pub module_count: usize,
    pub entry: std::path::PathBuf,
}

pub fn bundle(opts: BundleOptions) -> anyhow::Result<BundleResult> {
    let inject_top = opts
        .inject_top
        .map(|p| std::fs::read_to_string(p))
        .transpose()?;

    let inject_bottom = opts
        .inject_bottom
        .map(|p| std::fs::read_to_string(p))
        .transpose()?;

    let mut search_paths = opts.search_paths;

    if opts.luarocks {
        let lr_paths = luarocks::discover_paths(&opts.lua_version);
        if lr_paths.is_empty() {
            tracing::warn!("luarocks enabled but no paths found, is luarocks installed?");
        } else {
            tracing::debug!("discovered {} luarocks paths", lr_paths.len());
            search_paths.extend(lr_paths);
        }
    }

    let result = bundler::bundle(bundler::BundlerOptions {
        entry: opts.entry.clone(),
        search_paths,
        inject_top,
        inject_bottom,
        externals: opts.externals,
        overrides: opts.overrides,
    })?;

    Ok(BundleResult {
        module_count: result.module_count,
        output: result.output,
        entry: opts.entry,
    })
}
