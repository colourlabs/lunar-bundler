//! Error types for lunar-bundler.
//!
//! All errors implement [`std::error::Error`] via [`thiserror`] and include
//! enough context to produce useful messages without needing a backtrace.
//!
//! Errors are returned as [`anyhow::Error`] at the public API boundary in
//! [`crate::bundle`], so callers don't need to depend on this module directly
//! unless they want to match on specific variants.

use std::path::PathBuf;
use thiserror::Error;

/// All possible errors that can occur during bundling.
#[derive(Debug, Error)]
pub enum BundlerError {
    /// the entry file path does not exist on disk
    #[error("entry file does not exist: {0}")]
    EntryNotFound(PathBuf),

    /// a `require()` call could not be resolved to a file and was not
    /// marked as external in the bundler config
    #[error("cannot resolve module '{module}' required by '{requirer}'")]
    UnresolvedModule { module: String, requirer: PathBuf },

    /// a circular dependency was detected during graph traversal.
    /// lua allows circular requires at runtime via lazy loading but
    /// static bundling cannot resolve them
    #[error("circular dependency detected: {cycle}")]
    CircularDependency { cycle: String },

    /// full-moon failed to parse a lua source file.
    /// this can happen if the file contains syntax errors or lua 5.5
    /// syntax that the parser does not yet support
    #[error("failed to parse '{path}': {reason}")]
    ParseError { path: PathBuf, reason: String },

    /// a file could not be read from disk
    #[error("failed to read '{path}': {source}")]
    IoError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// an injection file specified via `--inject-top` or `--inject-bottom`
    /// could not be read from disk
    #[error("failed to read injection file '{path}': {source}")]
    InjectionReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

}
