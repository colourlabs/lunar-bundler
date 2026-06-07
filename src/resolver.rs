use std::path::{PathBuf};

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

        let resolver = Resolver::new(vec![dir.path().to_path_buf()]);
        assert_eq!(
            resolver.resolve("foo.bar").unwrap(),
            dir.path().join("foo/bar.lua")
        );
    }

    #[test]
    fn test_init_lua() {
        let dir = make_temp_dir();
        fs::create_dir_all(dir.path().join("foo/bar")).unwrap();
        fs::write(dir.path().join("foo/bar/init.lua"), "").unwrap();

        let resolver = Resolver::new(vec![dir.path().to_path_buf()]);
        assert_eq!(
            resolver.resolve("foo.bar").unwrap(),
            dir.path().join("foo/bar/init.lua")
        );
    }

    #[test]
    fn test_not_found() {
        let dir = make_temp_dir();
        let resolver = Resolver::new(vec![dir.path().to_path_buf()]);
        assert!(resolver.resolve("does.not.exist").is_none());
    }

    #[test]
    fn test_multiple_search_paths() {
        let dir1 = make_temp_dir();
        let dir2 = make_temp_dir();
        fs::write(dir2.path().join("foo.lua"), "").unwrap();

        let resolver = Resolver::new(vec![
            dir1.path().to_path_buf(),
            dir2.path().to_path_buf(),
        ]);
        assert_eq!(
            resolver.resolve("foo").unwrap(),
            dir2.path().join("foo.lua")
        );
    }
}

pub struct Resolver {
    search_paths: Vec<PathBuf>,
}

impl Resolver {
    pub fn new(search_paths: Vec<PathBuf>) -> Self {
        Self { search_paths }
    }

    pub fn resolve(&self, module: &str) -> Option<PathBuf> {
        // "foo.bar" -> "foo/bar"
        let as_path = module.replace('.', "/");

        for base in &self.search_paths {
            // try base/foo/bar.lua
            let candidate = base.join(format!("{}.lua", as_path));
            if candidate.exists() {
                return Some(candidate);
            }

            // try base/foo/bar/init.lua
            let candidate = base.join(format!("{}/init.lua", as_path));
            if candidate.exists() {
                return Some(candidate);
            }
        }

        None
    }
}