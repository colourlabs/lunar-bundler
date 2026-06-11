use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::compat::{CompatIssueKind, CompatLevel, check_compat};
use crate::emitter::Emitter;
use crate::graph::build_graph;
use crate::minify::minify_lua;
use crate::resolver::Resolver;
use crate::sandbox::SandboxLevel;
use crate::treeshake::{self, TreeShakeLevel};
use crate::{BuildMode, Loader, LoaderContext};

pub struct BundlerOptions {
    pub entry: PathBuf,
    pub search_paths: Vec<PathBuf>,
    pub lua_version: String,
    pub inject_top: Option<String>,
    pub inject_bottom: Option<String>,
    pub externals: Vec<String>,
    pub overrides: Vec<(String, PathBuf)>,
    pub resolve_extensions: Vec<String>,
    pub loaders: Vec<(String, Vec<Loader>)>,
    pub mode: BuildMode,
    pub sandbox_level: SandboxLevel,
    pub sandbox_deny: Vec<String>,
    pub compat_level: CompatLevel,
    pub compat_ignore: Vec<CompatIssueKind>,
    pub treeshake_level: TreeShakeLevel,
}

pub struct BundleOutput {
    pub output: String,
    pub sourcemap: String,
    pub module_count: usize,
}

pub fn run_loaders(
    source: String,
    path: &Path,
    module_name: &str,
    rules: &[(String, Vec<Loader>)],
    mode: &BuildMode,
) -> anyhow::Result<String> {
    let mut source = source;

    for (pattern, loaders) in rules {
        let path_str = path.to_string_lossy();
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if glob_match::glob_match(pattern, &path_str) || glob_match::glob_match(pattern, file_name)
        {
            for loader in loaders {
                source = loader(LoaderContext {
                    source,
                    path: path.to_path_buf(),
                    module_name: module_name.to_string(),
                    mode: mode.clone(),
                })?;
            }
        }
    }

    Ok(source)
}

pub fn bundle(opts: BundlerOptions) -> Result<BundleOutput> {
    let extensions = if opts.resolve_extensions.is_empty() {
        vec!["lua".to_string()]
    } else {
        opts.resolve_extensions
    };
    let resolver = Resolver::new(
        opts.search_paths,
        opts.externals,
        opts.overrides.into_iter().collect(),
    )
    .with_extensions(extensions);

    let mut graph = build_graph(opts.entry, &resolver, &opts.loaders, &opts.mode)?;

    // Sandbox check
    if opts.sandbox_level != SandboxLevel::Off && !opts.sandbox_deny.is_empty() {
        let mut all_violations = Vec::new();
        for module in &graph.modules {
            let violations = crate::sandbox::check_sandbox(&module.source, &opts.sandbox_deny);
            for v in &violations {
                all_violations.push(format!(
                    "{}:{}: use of denied global '{}'",
                    module.path.display(),
                    v.line,
                    v.name
                ));
            }
        }
        if !all_violations.is_empty() {
            match opts.sandbox_level {
                SandboxLevel::Error => {
                    anyhow::bail!("sandbox violations:\n{}", all_violations.join("\n"));
                }
                SandboxLevel::Warn => {
                    for msg in &all_violations {
                        eprintln!("warning: sandbox violation: {}", msg);
                    }
                }
                _ => {}
            }
        }
    }

    // Compatibility check
    if opts.compat_level != CompatLevel::Off {
        let mut all_issues = Vec::new();
        for module in &graph.modules {
            let issues = check_compat(&module.source, &opts.lua_version, &opts.compat_ignore);
            for issue in &issues {
                all_issues.push(format!(
                    "{}:{}: {:?} is not supported in Lua {}",
                    module.path.display(),
                    issue.line + 1,
                    issue.kind,
                    opts.lua_version
                ));
            }
        }
        if !all_issues.is_empty() {
            match opts.compat_level {
                CompatLevel::Error => {
                    anyhow::bail!("compatibility issues:\n{}", all_issues.join("\n"));
                }
                CompatLevel::Warn => {
                    for msg in &all_issues {
                        eprintln!("warning: compat: {}", msg);
                    }
                }
                _ => {}
            }
        }
    }

    // Tree shaking
    if opts.treeshake_level != TreeShakeLevel::Off {
        for module in &mut graph.modules {
            module.source = treeshake::treeshake(&module.source, opts.treeshake_level);
        }
    }

    // Minify module sources in production mode
    if opts.mode == BuildMode::Production {
        for module in &mut graph.modules {
            module.source = minify_lua(&module.source);
        }
    }

    let module_count = graph.modules.len().saturating_sub(1);
    let emitter = Emitter::new(opts.inject_top, opts.inject_bottom, &opts.mode);
    let (output, sourcemap) = emitter.emit(&graph);

    Ok(BundleOutput {
        output,
        sourcemap,
        module_count,
    })
}
