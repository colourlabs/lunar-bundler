//! Core bundling pipeline that wires together resolution, graph building, and emission.
//!
//! This module is the internal orchestrator - the public API is exposed through
//! [`crate::bundle`] in `lib.rs` which handles file I/O and config before
//! delegating here.
//!
//! ## pipeline
//!
//! ```text
//! BundlerOptions
//!     → Resolver (search paths + externals + overrides)
//!     → build_graph (recursive require() walking + topo sort)
//!     → Emitter (write runtime shim + module wrappers + entry point)
//!     → BundleOutput
//! ```

use anyhow::Result;
use std::path::PathBuf;

use crate::emitter::Emitter;
use crate::graph::build_graph;
use crate::resolver::Resolver;

/// internal options passed to the bundler pipeline.
/// see [`crate::BundleOptions`] for the public equivalent which
/// takes file paths for injections instead of strings.
pub struct BundlerOptions {
    /// entry point lua file
    pub entry: PathBuf,
    /// directories to search for required modules
    pub search_paths: Vec<PathBuf>,
    /// contents of the file to inject at the top of the bundle
    pub inject_top: Option<String>,
    /// contents of the file to inject at the bottom of the bundle
    pub inject_bottom: Option<String>,
    /// module names to treat as external
    pub externals: Vec<String>,
    /// module name to file path overrides
    pub overrides: Vec<(String, PathBuf)>,
}

/// output of a successful bundle operation
pub struct BundleOutput {
    /// the final bundled lua source
    pub output: String,
    /// number of dependency modules bundled, excluding the entry point
    pub module_count: usize,
}

/// run the full bundling pipeline and return the bundled output.
///
/// the entry point is always the last module emitted, dependencies
/// are sorted topologically so each module appears before anything
/// that requires it.
pub fn bundle(opts: BundlerOptions) -> Result<BundleOutput> {
    let resolver = Resolver::new(
        opts.search_paths,
        opts.externals,
        opts.overrides.into_iter().collect(),
    );
    let graph = build_graph(opts.entry, &resolver)?;
    let module_count = graph.modules.len().saturating_sub(1);
    let emitter = Emitter::new(opts.inject_top, opts.inject_bottom);
    Ok(BundleOutput {
        output: emitter.emit(&graph),
        module_count,
    })
}
