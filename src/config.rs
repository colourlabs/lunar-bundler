//! Configuration loading for lunar-bundler.
//!
//! Supports `lunar_bundler.toml` and `lunar_bundler.jsonc` config files. The config file
//! is optional - if none is found, all options fall back to CLI args or
//! their defaults.
//!
//! ## file format
//!
//! both formats are supported and auto-detected by file extension:
//!
//! - `.toml` - standard TOML format
//! - `.json` / `.jsonc` - JSON with C-style comments (`//` and `/* */`)
//!
//! ## lookup order
//!
//! 1. `--config` CLI flag if provided
//! 2. `lunar.toml` in the current directory
//! 3. `lunar.jsonc` in the current directory
//! 4. no config, all defaults

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// top-level config structure, all sections are optional
#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub bundle: Option<BundleConfig>,
    pub paths: Option<PathsConfig>,
    pub inject: Option<InjectConfig>,
    pub resolve: Option<ResolveConfig>,
    pub loaders: Option<LoadersConfig>,
    pub sandbox: Option<SandboxConfig>,
    pub compat: Option<CompatConfig>,
}

/// controls the core bundling behaviour
#[derive(Debug, Deserialize)]
pub struct BundleConfig {
    /// entry point lua file
    pub entry: Option<PathBuf>,
    /// output file path
    pub output: Option<PathBuf>,
    /// lua version to target: "51", "52", "53", "54", "55", "luajit"
    pub lua_version: Option<String>,
    /// enable luarocks path discovery, defaults to false
    pub luarocks: Option<bool>,
}

/// configures module search paths
#[derive(Debug, Deserialize)]
pub struct PathsConfig {
    /// additional directories to search for required modules
    pub search: Option<Vec<PathBuf>>,
}

/// files to inject at the top or bottom of the bundle
#[derive(Debug, Deserialize)]
pub struct InjectConfig {
    /// injected before the runtime shim
    pub top: Option<PathBuf>,
    /// injected after the entry point
    pub bottom: Option<PathBuf>,
}

/// controls module resolution behaviour
#[derive(Debug, Deserialize, Default)]
pub struct ResolveConfig {
    /// modules to leave as require() calls at runtime.
    /// supports wildcards: "lunar/*" matches "lunar/router", "lunar/middleware"
    pub externals: Option<Vec<String>>,
    /// override where specific modules resolve to on disk
    pub overrides: Option<Vec<ModuleOverride>>,
    /// file extensions to try when resolving modules (e.g. ["lua", "moon"])
    pub extensions: Option<Vec<String>>,
}

/// maps a module name to a specific file path, bypassing normal resolution
#[derive(Debug, Deserialize, Clone)]
pub struct ModuleOverride {
    /// the require() string to match, e.g. "json"
    pub module: String,
    /// the file to resolve it to
    pub path: PathBuf,
}

/// Configures file loaders (webpack-style module rules).
#[derive(Debug, Deserialize, Default)]
pub struct LoadersConfig {
    /// Named shell commands that can be referenced in rules.
    /// e.g. `strip-comments = "lua tools/strip-comments.lua"`
    pub commands: Option<HashMap<String, String>>,
    /// Module rules that match files by glob pattern and apply loaders.
    pub rules: Option<Vec<LoaderRuleConfig>>,
}

/// A single loader rule.
#[derive(Debug, Deserialize, Clone)]
pub struct LoaderRuleConfig {
    /// Glob pattern to match files (e.g. "*.lua$", "*.moon$")
    pub test: String,
    /// Named loaders to apply (references keys from `[loaders.commands]`)
    #[serde(alias = "use")]
    pub use_: Option<Vec<String>>,
    /// Inline shell command to run as a loader
    pub run: Option<String>,
    /// Only apply this rule in the given mode ("development" or "production")
    pub mode: Option<String>,
}

/// Configures Lua version compatibility checks.
#[derive(Debug, Deserialize)]
pub struct CompatConfig {
    /// "error" (default), "warn", or "off"
    pub level: Option<String>,
    /// List of CompatIssueKind variant names to ignore
    pub ignore: Option<Vec<String>>,
}

/// Configures sandbox restrictions on allowed globals.
#[derive(Debug, Deserialize)]
pub struct SandboxConfig {
    /// "error" (default) or "warn"
    pub level: Option<String>,
    /// List of denied global names (e.g. "os", "io", "dofile")
    pub deny: Option<Vec<String>>,
}

impl Config {
    /// load config from a file path.
    ///
    /// returns `Config::default()` if the file does not exist rather than
    /// erroring, so callers don't need to check existence first.
    ///
    /// the format is auto-detected from the file extension:
    /// `.json`/`.jsonc` are parsed as JSONC, everything else as TOML.
    pub fn load(path: &PathBuf) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Config::default());
        }

        let contents = std::fs::read_to_string(path)?;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let config = match ext {
            "json" | "jsonc" => {
                let stripped = strip_jsonc_comments(&contents);
                serde_json::from_str(&stripped)
                    .map_err(|e| anyhow::anyhow!("failed to parse config: {}", e))?
            }
            _ => {
                // default to toml
                toml::from_str(&contents)
                    .map_err(|e| anyhow::anyhow!("failed to parse config: {}", e))?
            }
        };

        Ok(config)
    }
}

fn strip_jsonc_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            // string literal - pass through without stripping
            '"' => {
                out.push(c);
                while let Some(s) = chars.next() {
                    out.push(s);
                    if s == '\\' {
                        // escaped char - push next unconditionally
                        if let Some(escaped) = chars.next() {
                            out.push(escaped);
                        }
                    } else if s == '"' {
                        break;
                    }
                }
            }
            '/' => match chars.peek() {
                Some('/') => {
                    // line comment - skip to end of line
                    chars.next();
                    for c in chars.by_ref() {
                        if c == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                }
                Some('*') => {
                    // block comment - skip to */
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '*' && chars.peek() == Some(&'/') {
                            chars.next();
                            break;
                        }
                        // preserve newlines so line numbers stay accurate
                        if c == '\n' {
                            out.push('\n');
                        }
                    }
                }
                _ => out.push(c),
            },
            _ => out.push(c),
        }
    }

    out
}
