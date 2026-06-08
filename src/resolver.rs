use std::{collections::HashMap, path::PathBuf};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn test_simple_resolve() {
        let dir = make_temp_dir();
        fs::create_dir_all(dir.path().join("foo")).unwrap();
        fs::write(dir.path().join("foo/bar.lua"), "").unwrap();

        let resolver = Resolver::with_paths(vec![dir.path().to_path_buf()]);
        assert!(matches!(
            resolver.resolve("foo.bar"),
            ResolveResult::Found(p) if p == dir.path().join("foo/bar.lua")
        ));
    }

    #[test]
    fn test_init_lua() {
        let dir = make_temp_dir();
        fs::create_dir_all(dir.path().join("foo/bar")).unwrap();
        fs::write(dir.path().join("foo/bar/init.lua"), "").unwrap();

        let resolver = Resolver::with_paths(vec![dir.path().to_path_buf()]);
        assert!(matches!(
            resolver.resolve("foo.bar"),
            ResolveResult::Found(p) if p == dir.path().join("foo/bar/init.lua")
        ));
    }

    #[test]
    fn test_not_found() {
        let dir = make_temp_dir();
        let resolver = Resolver::with_paths(vec![dir.path().to_path_buf()]);
        assert!(matches!(
            resolver.resolve("does.not.exist"),
            ResolveResult::NotFound
        ));
    }

    #[test]
    fn test_multiple_search_paths() {
        let dir1 = make_temp_dir();
        let dir2 = make_temp_dir();
        fs::write(dir2.path().join("foo.lua"), "").unwrap();

        let resolver =
            Resolver::with_paths(vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()]);
        assert!(matches!(
            resolver.resolve("foo"),
            ResolveResult::Found(p) if p == dir2.path().join("foo.lua")
        ));
    }

    #[test]
    fn test_external() {
        let dir = make_temp_dir();
        let resolver = Resolver::new(
            vec![dir.path().to_path_buf()],
            vec!["socket".to_string()],
            HashMap::new(),
        );
        assert!(matches!(
            resolver.resolve("socket"),
            ResolveResult::External
        ));
    }

    #[test]
    fn test_wildcard_external() {
        let dir = make_temp_dir();
        let resolver = Resolver::new(
            vec![dir.path().to_path_buf()],
            vec!["lunar/*".to_string()],
            HashMap::new(),
        );
        assert!(matches!(
            resolver.resolve("lunar/router"),
            ResolveResult::External
        ));
        assert!(matches!(
            resolver.resolve("lunar/middleware"),
            ResolveResult::External
        ));
    }
}

pub enum ResolveResult {
    Found(PathBuf),
    External,
    NotFound,
}

pub struct Resolver {
    search_paths: Vec<PathBuf>,
    externals: Vec<String>,
    overrides: HashMap<String, PathBuf>,
}

impl Resolver {
    pub fn new(
        search_paths: Vec<PathBuf>,
        externals: Vec<String>,
        overrides: HashMap<String, PathBuf>,
    ) -> Self {
        Self {
            search_paths,
            externals,
            overrides,
        }
    }

    pub fn with_paths(search_paths: Vec<PathBuf>) -> Self {
        Self::new(search_paths, vec![], HashMap::new())
    }

    pub fn resolve(&self, module: &str) -> ResolveResult {
        if self.is_external(module) {
            tracing::debug!(module, "skipping external module");
            return ResolveResult::External;
        }

        if let Some(path) = self.overrides.get(module) {
            tracing::debug!(module, path = %path.display(), "resolved via override");
            return ResolveResult::Found(path.clone());
        }

        let as_path = module.replace(['.', '/'], std::path::MAIN_SEPARATOR_STR);

        for base in &self.search_paths {
            let candidate = base.join(format!("{}.lua", as_path));
            if candidate.exists() {
                if crate::luarocks::is_native_module(&candidate) {
                    tracing::warn!(
                        module,
                        "native C module cannot be bundled!, treating as external"
                    );
                    return ResolveResult::External;
                }
                return ResolveResult::Found(candidate);
            }

            let candidate = base.join(&as_path).join("init.lua");
            if candidate.exists() {
                return ResolveResult::Found(candidate);
            }
        }

        ResolveResult::NotFound
    }

    fn is_external(&self, module: &str) -> bool {
        self.externals.iter().any(|ext| {
            if ext.ends_with("/*") {
                // wildcard match: "lunar/*" matches "lunar/router", "lunar/middleware" etc
                let prefix = ext.trim_end_matches("/*");
                module.starts_with(prefix)
            } else {
                module == ext
            }
        })
    }
}
