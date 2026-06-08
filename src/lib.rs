//! # lunar-bundler
//!
//! A Lua bundler that resolves `require()` calls and bundles your project
//! into a single file. Supports Lua 5.1 through 5.5.
//!
//! ## example
//!
//! ```no_run
//! use lunar_bundler::{BundleOptions, bundle};
//! use std::path::PathBuf;
//!
//! let result = bundle(BundleOptions {
//!     entry: PathBuf::from("src/main.lua"),
//!     search_paths: vec![PathBuf::from("src")],
//!     lua_version: "54".to_string(),
//!     ..Default::default()
//! }).unwrap();
//!
//! println!("bundled {} modules", result.module_count);
//! println!("{}", result.output);
//! ```

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
/// Options for the bundler.
pub struct BundleOptions {
    /// entry point lua file
    pub entry: std::path::PathBuf,
    /// additional search paths for require() resolution
    pub search_paths: Vec<std::path::PathBuf>,
    /// lua version to target: "51" -> Lua 5.1, "52" -> Lua 5.2, "53" -> Lua 5.3, "54" -> Lua 5.4, "55" -> Lua 5.5
    pub lua_version: String,
    /// file to inject at the top of the bundle
    pub inject_top: Option<std::path::PathBuf>,
    /// file to inject at the bottom of the bundle
    pub inject_bottom: Option<std::path::PathBuf>,
    /// modules to treat as external, left as require() calls at runtime.
    /// supports wildcards: "lunar/*" matches "lunar/router", "lunar/middleware", etc
    pub externals: Vec<String>,
    /// override where specific modules resolve to: ("json", "vendor/json/init.lua")
    pub overrides: Vec<(String, std::path::PathBuf)>,
    /// enable luarocks path discovery
    pub luarocks: bool,
}

/// Result of a successful bundle operation.
pub struct BundleResult {
    /// the bundled lua source
    pub output: String,
    /// number of modules bundled (excludes entry point)
    pub module_count: usize,
    /// the resolved entry point path
    pub entry: std::path::PathBuf,
}

/// Bundle a Lua project into a single file.
///
/// Walks all `require()` calls recursively from the entry point,
/// resolves them to files on disk, topologically sorts the dependency
/// graph, and emits a single Lua file with a runtime shim.
///
/// ## errors
///
/// returns an error if:
/// - the entry file does not exist
/// - a required module cannot be resolved and is not marked as external
/// - a circular dependency is detected
/// - a file cannot be parsed
pub fn bundle(opts: BundleOptions) -> anyhow::Result<BundleResult> {
    let inject_top = opts.inject_top.map(std::fs::read_to_string).transpose()?;

    let inject_bottom = opts
        .inject_bottom
        .map(std::fs::read_to_string)
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
