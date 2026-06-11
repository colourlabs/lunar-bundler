pub mod bundler;
pub mod compat;
pub mod config;
pub mod emitter;
pub mod error;
pub mod graph;
pub mod loader;
pub mod luarocks;
pub mod minify;
pub mod preprocessor;
pub mod resolver;
pub mod sandbox;
pub mod scanner;
pub mod sourcemap;
pub mod treeshake;

#[derive(Default, Debug, Clone, PartialEq)]
pub enum BuildMode {
    #[default]
    Development,
    Production,
}

impl BuildMode {
    pub fn from_mode_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "production" | "prod" | "release" => BuildMode::Production,
            _ => BuildMode::Development,
        }
    }
}

pub struct LoaderContext {
    pub source: String,
    pub path: std::path::PathBuf,
    pub module_name: String,
    pub mode: BuildMode,
}

pub type Loader = Box<dyn Fn(LoaderContext) -> anyhow::Result<String> + Send + Sync>;

pub struct BundleOptions {
    pub entry: std::path::PathBuf,
    pub search_paths: Vec<std::path::PathBuf>,
    pub lua_version: String,
    pub inject_top: Option<std::path::PathBuf>,
    pub inject_bottom: Option<std::path::PathBuf>,
    pub externals: Vec<String>,
    pub overrides: Vec<(String, std::path::PathBuf)>,
    pub luarocks: bool,
    pub resolve_extensions: Vec<String>,
    pub mode: BuildMode,
    pub loaders: Vec<(String, Vec<Loader>)>,
    pub sandbox_level: crate::sandbox::SandboxLevel,
    pub sandbox_deny: Vec<String>,
    pub compat_level: crate::compat::CompatLevel,
    pub compat_ignore: Vec<crate::compat::CompatIssueKind>,
    pub treeshake_level: crate::treeshake::TreeShakeLevel,
}

impl Default for BundleOptions {
    fn default() -> Self {
        Self {
            entry: std::path::PathBuf::new(),
            search_paths: vec![],
            lua_version: "55".to_string(),
            inject_top: None,
            inject_bottom: None,
            externals: vec![],
            overrides: vec![],
            luarocks: false,
            resolve_extensions: vec![],
            mode: BuildMode::default(),
            loaders: vec![],
            sandbox_level: crate::sandbox::SandboxLevel::Off,
            sandbox_deny: vec![],
            compat_level: crate::compat::CompatLevel::Off,
            compat_ignore: vec![],
            treeshake_level: crate::treeshake::TreeShakeLevel::default(),
        }
    }
}

pub struct BundleResult {
    pub output: String,
    pub sourcemap: String,
    pub module_count: usize,
    pub entry: std::path::PathBuf,
}

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
        lua_version: opts.lua_version,
        inject_top,
        inject_bottom,
        externals: opts.externals,
        overrides: opts.overrides,
        loaders: opts.loaders,
        mode: opts.mode,
        resolve_extensions: opts.resolve_extensions,
        sandbox_level: opts.sandbox_level,
        sandbox_deny: opts.sandbox_deny,
        compat_level: opts.compat_level,
        compat_ignore: opts.compat_ignore,
        treeshake_level: opts.treeshake_level,
    })?;

    Ok(BundleResult {
        module_count: result.module_count,
        output: result.output,
        sourcemap: result.sourcemap,
        entry: opts.entry,
    })
}
