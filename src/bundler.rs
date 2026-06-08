use anyhow::Result;
use std::path::PathBuf;

use crate::emitter::Emitter;
use crate::graph::build_graph;
use crate::resolver::Resolver;

pub struct BundlerOptions {
    pub entry: PathBuf,
    pub search_paths: Vec<PathBuf>,
    pub inject_top: Option<String>,
    pub inject_bottom: Option<String>,
    pub externals: Vec<String>,
    pub overrides: Vec<(String, PathBuf)>,
}

pub struct BundleOutput {
    pub output: String,
    pub module_count: usize,
}

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
