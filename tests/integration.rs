use lunar_bundler::{BundleOptions, bundle};
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn default_options(entry: PathBuf, search_paths: Vec<PathBuf>, lua_version: &str) -> BundleOptions {
    BundleOptions {
        entry,
        search_paths,
        lua_version: lua_version.to_string(),
        inject_top: None,
        inject_bottom: None,
        externals: vec![],
        overrides: vec![],
        ..Default::default()
    }
}

#[test]
fn test_simple_bundle() {
    let out = bundle(default_options(
        fixture("simple/main.lua"),
        vec![fixture("simple")],
        "54",
    ))
    .unwrap()
    .output;

    assert!(out.contains("__modules[\"foo\"]"));
}

#[test]
fn test_diamond_bundle() {
    let out = bundle(default_options(
        fixture("diamond/main.lua"),
        vec![fixture("diamond")],
        "54",
    ))
    .unwrap()
    .output;

    assert_eq!(out.matches("__modules[\"c\"]").count(), 1);
    assert!(out.contains("__modules[\"a\"]"));
    assert!(out.contains("__modules[\"b\"]"));
}

#[test]
fn test_nested_bundle() {
    let out = bundle(default_options(
        fixture("nested/main.lua"),
        vec![fixture("nested")],
        "54",
    ))
    .unwrap()
    .output;

    assert!(out.contains("__modules[\"lib.utils\"]"));
    assert!(out.contains("__modules[\"lib\"]"));
    let utils_pos = out.find("__modules[\"lib.utils\"]").unwrap();
    let lib_pos = out.find("__modules[\"lib\"]").unwrap();
    assert!(utils_pos < lib_pos);
}

#[test]
fn test_lua55_bundle() {
    let out = bundle(default_options(
        fixture("lua55/main.lua"),
        vec![fixture("lua55")],
        "55",
    ))
    .unwrap()
    .output;

    assert!(out.contains("__modules[\"foo\"]"));
}

#[test]
fn test_external_not_bundled() {
    let out = bundle(BundleOptions {
        entry: fixture("external/main.lua"),
        search_paths: vec![fixture("external")],
        lua_version: "54".to_string(),
        inject_top: None,
        inject_bottom: None,
        externals: vec!["socket".to_string(), "lunar/*".to_string()],
        overrides: vec![],
        ..Default::default()
    })
    .unwrap()
    .output;

    assert!(!out.contains("__modules[\"socket\"]"));
    assert!(out.contains("__modules[\"utils\"]"));
    assert!(out.contains(r#"require("socket")"#));
}

#[test]
fn test_wildcard_external_not_bundled() {
    let out = bundle(BundleOptions {
        entry: fixture("external/main.lua"),
        search_paths: vec![fixture("external")],
        lua_version: "54".to_string(),
        inject_top: None,
        inject_bottom: None,
        externals: vec!["socket".to_string(), "lunar/*".to_string()],
        overrides: vec![],
        ..Default::default()
    })
    .unwrap()
    .output;

    assert!(!out.contains("__modules[\"lunar/router\"]"));
    assert!(out.contains("__modules[\"utils\"]"));
    assert!(out.contains(r#"require("lunar/router")"#));
}

#[test]
fn test_unresolved_non_external_errors() {
    let result = bundle(BundleOptions {
        entry: fixture("external/main.lua"),
        search_paths: vec![fixture("external")],
        lua_version: "54".to_string(),
        inject_top: None,
        inject_bottom: None,
        externals: vec![],
        overrides: vec![],
        ..Default::default()
    });

    assert!(result.is_err());
}

#[test]
fn test_moonscript_loader() {
    // skip if moonc isn't available
    let has_moonc = std::process::Command::new("moonc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
        || dirs::home_dir()
            .unwrap_or_default()
            .join(".luarocks/bin/moonc")
            .exists();

    if !has_moonc {
        eprintln!("skipping: moonscript not installed (install with `luarocks install moonscript`)");
        return;
    }

    let out = bundle(BundleOptions {
        entry: fixture("moonscript/main.lua"),
        search_paths: vec![fixture("moonscript")],
        lua_version: "54".to_string(),
        loaders: vec![("*.moon".to_string(), vec![lunar_bundler::loader::moonscript_loader()])],
        resolve_extensions: vec!["moon".to_string(), "lua".to_string()],
        ..Default::default()
    })
    .unwrap()
    .output;

    // moonscript class -> Lua metatable should be compiled
    assert!(!out.contains("class Greeting"), "raw Moonscript should not appear");
    assert!(out.contains("local Greeting"), "compiled Lua should have local Greeting");
    assert!(out.contains("hello = function"), "compiled method should exist");

    // plain .lua file should pass through unmodified
    assert!(out.contains("function M.capitalize"), "plain lua should be present");

    // modules should be present in __modules
    assert!(out.contains("__modules[\"greeting\"]"));
    assert!(out.contains("__modules[\"utils\"]"));
}

#[test]
fn test_production_minification() {
    let out = bundle(BundleOptions {
        entry: fixture("simple/main.lua"),
        search_paths: vec![fixture("simple")],
        lua_version: "54".to_string(),
        mode: lunar_bundler::BuildMode::Production,
        ..Default::default()
    })
    .unwrap();

    // In production: no sourceMappingURL comment
    assert!(!out.output.contains("sourceMappingURL"), "no dev hint in production");

    // sourcemap should still be present on the result
    assert!(out.sourcemap.contains("\"version\":3"), "sourcemap generated in production too");
}

#[test]
fn test_dev_sourcemap() {
    // Fixture with comments to verify both outputs
    let out = bundle(BundleOptions {
        entry: fixture("simple/main.lua"),
        search_paths: vec![fixture("simple")],
        lua_version: "54".to_string(),
        mode: lunar_bundler::BuildMode::Development,
        ..Default::default()
    })
    .unwrap();

    // In dev: sourceMappingURL should be present
    assert!(out.output.contains("sourceMappingURL"), "dev output has sourcemap hint");

    // sourcemap should be valid JSON
    let sm: serde_json::Value = serde_json::from_str(&out.sourcemap)
        .expect("sourcemap should be valid JSON");
    assert_eq!(sm["version"], 3);
    assert!(!sm["sources"].as_array().unwrap().is_empty());
    assert!(!sm["mappings"].as_str().unwrap().is_empty());
}

#[test]
#[ignore]
fn test_luarocks_argparse() {
    // skip if luarocks isn't available
    if std::process::Command::new("luarocks")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("skipping: luarocks not installed");
        return;
    }

    // skip if argparse isn't installed
    let installed = std::process::Command::new("luarocks")
        .args(["show", "argparse"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !installed {
        eprintln!("skipping: inspect not installed via luarocks");
        return;
    }

    let out = bundle(BundleOptions {
        entry: fixture("luarocks/main.lua"),
        search_paths: vec![fixture("luarocks")],
        lua_version: "54".to_string(),
        inject_top: None,
        inject_bottom: None,
        externals: vec![],
        overrides: vec![],
        luarocks: true,
        mode: lunar_bundler::BuildMode::default(),
        loaders: vec![],
        resolve_extensions: vec![],
        ..Default::default()
    })
    .unwrap()
    .output;

    assert!(out.contains("__modules[\"argparse\"]"));
}
