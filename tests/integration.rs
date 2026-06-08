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
    })
    .unwrap()
    .output;

    assert!(out.contains("__modules[\"argparse\"]"));
}
