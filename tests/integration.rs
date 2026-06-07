use std::path::PathBuf;
use lunar_bundler::{BundleOptions, bundle};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn test_diamond_bundle() {
    let out = bundle(BundleOptions {
        entry: fixture("diamond/main.lua"),
        search_paths: vec![fixture("diamond")],
        lua_version: "54".to_string(),
        inject_top: None,
        inject_bottom: None,
    }).unwrap();

    // c should appear exactly once
    assert_eq!(out.matches("__modules[\"c\"]").count(), 1);
    assert!(out.contains("__modules[\"a\"]"));
    assert!(out.contains("__modules[\"b\"]"));
}

#[test]
fn test_nested_bundle() {
    let out = bundle(BundleOptions {
        entry: fixture("nested/main.lua"),
        search_paths: vec![fixture("nested")],
        lua_version: "54".to_string(),
        inject_top: None,
        inject_bottom: None,
    }).unwrap();

    // lib.utils should appear before lib since lib depends on it
    assert!(out.contains("__modules[\"lib.utils\"]"));
    assert!(out.contains("__modules[\"lib\"]"));
    let utils_pos = out.find("__modules[\"lib.utils\"]").unwrap();
    let lib_pos = out.find("__modules[\"lib\"]").unwrap();
    assert!(utils_pos < lib_pos);
}

#[test]
fn test_simple_bundle() {
    let out = bundle(BundleOptions {
        entry: fixture("simple/main.lua"),
        search_paths: vec![fixture("simple")],
        lua_version: "54".to_string(),
        inject_top: None,
        inject_bottom: None,
    }).unwrap();

    assert!(out.contains("__modules[\"foo\"]"));
}

#[test]
fn test_lua55_bundle() {
    let out = bundle(BundleOptions {
        entry: fixture("lua55/main.lua"),
        search_paths: vec![fixture("lua55")],
        lua_version: "55".to_string(),
        inject_top: None,
        inject_bottom: None,
    }).unwrap();

    assert!(out.contains("__modules[\"foo\"]"));
}