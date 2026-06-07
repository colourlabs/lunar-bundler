use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::error::BundlerError;
use crate::resolver::{ResolveResult, Resolver};
use crate::scanner::scan_requires;

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
        let graph = build_graph(dir.path().join("main.lua"), &resolver).unwrap();

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
        let result = build_graph(dir.path().join("a.lua"), &resolver);

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
        let result = build_graph(dir.path().join("main.lua"), &resolver);

        assert!(result.is_err());
    }

    #[test]
    fn test_diamond_dependency() {
        let dir = tempdir().unwrap();
        // main -> a, b; a -> c; b -> c  (c should only appear once)
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
        let graph = build_graph(dir.path().join("main.lua"), &resolver).unwrap();

        // c should only be included once despite being required twice
        let names: Vec<&str> = graph
            .modules
            .iter()
            .map(|m| m.module_name.as_str())
            .collect();
        assert_eq!(names.iter().filter(|&&n| n == "c").count(), 1);
        assert_eq!(graph.modules.len(), 4); // c, a, b, entry
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
    /// topologically sorted list of modules, entry point last
    pub modules: Vec<Module>,
}

pub fn build_graph(entry: PathBuf, resolver: &Resolver) -> Result<DependencyGraph> {
    let mut visited: HashMap<PathBuf, String> = HashMap::new(); // path -> module name
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
        &mut visited,
        &mut in_stack,
        &mut order,
    )?;

    let modules = order
        .into_iter()
        .map(|path| {
            let module_name = visited[&path].clone();
            let source = std::fs::read_to_string(&path)?;
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
    path: &PathBuf,
    module_name: &str,
    resolver: &Resolver,
    visited: &mut HashMap<PathBuf, String>,
    in_stack: &mut HashSet<PathBuf>,
    order: &mut Vec<PathBuf>,
) -> Result<()> {
    let path = &path.canonicalize().map_err(|e| BundlerError::IoError {
        path: path.clone(),
        source: e,
    })?;

    if visited.contains_key(path) {
        return Ok(());
    }

    if in_stack.contains(path) {
        return Err(BundlerError::CircularDependency {
            cycle: path.display().to_string(),
        }
        .into());
    }

    in_stack.insert(path.clone());

    let source = std::fs::read_to_string(path).map_err(|e| BundlerError::IoError {
        path: path.clone(),
        source: e,
    })?;
    let requires = scan_requires(&source, path)?;

    for req in requires {
        match resolver.resolve(&req) {
            ResolveResult::Found(dep_path) => {
                visit(&dep_path, &req, resolver, visited, in_stack, order)?
            }
            ResolveResult::External => {
                // skip - leave require() call intact for runtime
            }
            ResolveResult::NotFound => {
                return Err(BundlerError::UnresolvedModule {
                    module: req.clone(),
                    requirer: path.clone(),
                }
                .into());
            }
        }
    }

    in_stack.remove(path);
    visited.insert(path.clone(), module_name.to_string());
    order.push(path.clone());

    Ok(())
}
