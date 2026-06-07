use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;

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

pub fn bundle(opts: BundlerOptions) -> Result<String> {
    let overrides = opts.overrides
        .into_iter()
        .collect::<HashMap<String, PathBuf>>();

    let resolver = Resolver::new(opts.search_paths, opts.externals, overrides);
    let graph = build_graph(opts.entry, &resolver)?;
    let emitter = Emitter::new(opts.inject_top, opts.inject_bottom);
    Ok(emitter.emit(&graph))
}
