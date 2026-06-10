use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::bundler::run_loaders;
use crate::error::BundlerError;
use crate::resolver::{ResolveResult, Resolver};
use crate::scanner::scan_requires;
use crate::{BuildMode, Loader};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_simple_graph() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.lua"), r#"local x = require("foo")"#).unwrap();
        fs::write(dir.path().join("foo.lua"), "return {}").unwrap();

        let resolver = Resolver::new(vec![dir.path().to_path_buf()], vec![], HashMap::new());
        let graph = build_graph(
            dir.path().join("main.lua"),
            &resolver,
            &[],
            &BuildMode::Development,
        )
        .unwrap();

        assert_eq!(graph.modules.len(), 2);
        assert_eq!(graph.modules[0].module_name, "foo");
        assert_eq!(graph.modules[1].module_name, "__entry__");
    }

    #[test]
    fn test_cycle_detected() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.lua"), r#"local x = require("b")"#).unwrap();
        fs::write(dir.path().join("b.lua"), r#"local x = require("a")"#).unwrap();

        let resolver = Resolver::new(vec![dir.path().to_path_buf()], vec![], HashMap::new());
        let result = build_graph(
            dir.path().join("a.lua"),
            &resolver,
            &[],
            &BuildMode::Development,
        );

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("circular dependency")
        );
    }

    #[test]
    fn test_unresolved_module() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("main.lua"),
            r#"local x = require("missing")"#,
        )
        .unwrap();

        let resolver = Resolver::new(vec![dir.path().to_path_buf()], vec![], HashMap::new());
        let result = build_graph(
            dir.path().join("main.lua"),
            &resolver,
            &[],
            &BuildMode::Development,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_diamond_dependency() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("main.lua"),
            r#"
            local a = require("a")
            local b = require("b")
        "#,
        )
        .unwrap();
        fs::write(
            dir.path().join("a.lua"),
            r#"local c = require("c") return {}"#,
        )
        .unwrap();
        fs::write(
            dir.path().join("b.lua"),
            r#"local c = require("c") return {}"#,
        )
        .unwrap();
        fs::write(dir.path().join("c.lua"), "return {}").unwrap();

        let resolver = Resolver::new(vec![dir.path().to_path_buf()], vec![], HashMap::new());
        let graph = build_graph(
            dir.path().join("main.lua"),
            &resolver,
            &[],
            &BuildMode::Development,
        )
        .unwrap();

        let names: Vec<&str> = graph
            .modules
            .iter()
            .map(|m| m.module_name.as_str())
            .collect();
        assert_eq!(names.iter().filter(|&&n| n == "c").count(), 1);
        assert_eq!(graph.modules.len(), 4);
    }

    #[test]
    fn test_loaders_applied_to_source() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.lua"), r#"local x = require("foo")"#).unwrap();
        fs::write(dir.path().join("foo.lua"), "-- ORIGINAL").unwrap();

        let loader: Loader = Box::new(|ctx| Ok(ctx.source.replace("ORIGINAL", "TRANSFORMED")));

        let rules = vec![("*.lua".to_string(), vec![loader])];
        let resolver = Resolver::new(vec![dir.path().to_path_buf()], vec![], HashMap::new());
        let graph = build_graph(
            dir.path().join("main.lua"),
            &resolver,
            &rules,
            &BuildMode::Development,
        )
        .unwrap();

        let foo = graph
            .modules
            .iter()
            .find(|m| m.module_name == "foo")
            .unwrap();
        assert!(foo.source.contains("TRANSFORMED"));
        assert!(!foo.source.contains("ORIGINAL"));
    }
}

#[derive(Debug)]
pub struct Module {
    pub path: PathBuf,
    pub module_name: String,
    pub source: String,
}

#[derive(Debug)]
pub struct DependencyGraph {
    pub modules: Vec<Module>,
}

type Visited = HashMap<PathBuf, (String, String)>;

pub fn build_graph(
    entry: PathBuf,
    resolver: &Resolver,
    loaders: &[(String, Vec<Loader>)],
    mode: &BuildMode,
) -> Result<DependencyGraph> {
    let mut visited: Visited = HashMap::new();
    let mut order: Vec<PathBuf> = Vec::new();
    let mut in_stack: HashSet<PathBuf> = HashSet::new();
    let entry = entry.canonicalize().map_err(|e| BundlerError::IoError {
        path: entry.clone(),
        source: e,
    })?;

    visit(
        &entry,
        "__entry__",
        resolver,
        loaders,
        mode,
        &mut visited,
        &mut in_stack,
        &mut order,
    )?;

    let modules = order
        .into_iter()
        .map(|path| {
            let (module_name, source) = visited
                .remove(&path)
                .expect("missing source for visited module");
            Ok(Module {
                path,
                module_name,
                source,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(DependencyGraph { modules })
}

fn visit(
    path: &Path,
    module_name: &str,
    resolver: &Resolver,
    loaders: &[(String, Vec<Loader>)],
    mode: &BuildMode,
    visited: &mut Visited,
    in_stack: &mut HashSet<PathBuf>,
    order: &mut Vec<PathBuf>,
) -> Result<()> {
    let canonical = &path.canonicalize().map_err(|e| BundlerError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    if visited.contains_key(canonical) {
        return Ok(());
    }

    if in_stack.contains(canonical) {
        return Err(BundlerError::CircularDependency {
            cycle: canonical.display().to_string(),
        }
        .into());
    }

    in_stack.insert(canonical.clone());

    let source = std::fs::read_to_string(canonical).map_err(|e| BundlerError::IoError {
        path: canonical.clone(),
        source: e,
    })?;

    let source = run_loaders(source, canonical, module_name, loaders, mode)?;

    let scan_result = scan_requires(&source, canonical)?;

    for location in &scan_result.dynamic_requires {
        eprintln!(
            "warning: dynamic require() in '{}' cannot be bundled and will be resolved at runtime",
            location
        );
    }

    for req in scan_result.requires {
        match resolver.resolve(&req) {
            ResolveResult::Found(dep_path) => visit(
                &dep_path, &req, resolver, loaders, mode, visited, in_stack, order,
            )?,
            ResolveResult::External => {}
            ResolveResult::NotFound => {
                return Err(BundlerError::UnresolvedModule {
                    module: req.clone(),
                    requirer: canonical.clone(),
                }
                .into());
            }
        }
    }

    in_stack.remove(canonical);
    visited.insert(canonical.clone(), (module_name.to_string(), source));
    order.push(canonical.clone());

    Ok(())
}
