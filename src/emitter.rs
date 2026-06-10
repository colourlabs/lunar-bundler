use crate::BuildMode;
use crate::graph::{DependencyGraph, Module};
use crate::sourcemap::SourceMapBuilder;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{DependencyGraph, Module};
    use std::path::PathBuf;

    fn make_module(name: &str, source: &str) -> Module {
        Module {
            path: PathBuf::from(format!("{}.lua", name)),
            module_name: name.to_string(),
            source: source.to_string(),
        }
    }

    #[test]
    fn test_emit_basic() {
        let graph = DependencyGraph {
            modules: vec![
                make_module("foo", "return {}"),
                make_module("__entry__", r#"local x = require("foo")"#),
            ],
        };

        let emitter = Emitter::new(None, None, &BuildMode::Development);
        let (out, _sm) = emitter.emit(&graph);

        assert!(out.contains("__modules[\"foo\"]"));
        assert!(out.contains("return {}"));
        assert!(out.contains(r#"local x = require("foo")"#));
        assert!(!out.contains("__modules[\"__entry__\"]"));
    }

    #[test]
    fn test_emit_inject_top() {
        let graph = DependencyGraph {
            modules: vec![make_module("__entry__", "print('hello')")],
        };

        let emitter = Emitter::new(
            Some("-- injected top".to_string()),
            None,
            &BuildMode::Development,
        );
        let (out, _sm) = emitter.emit(&graph);

        let top_pos = out.find("-- injected top").unwrap();
        let shim_pos = out.find("local __modules").unwrap();
        assert!(top_pos < shim_pos);
    }

    #[test]
    fn test_emit_inject_bottom() {
        let graph = DependencyGraph {
            modules: vec![make_module("__entry__", "print('hello')")],
        };

        let emitter = Emitter::new(
            None,
            Some("-- injected bottom".to_string()),
            &BuildMode::Development,
        );
        let (out, _sm) = emitter.emit(&graph);

        let entry_pos = out.find("print('hello')").unwrap();
        let bottom_pos = out.find("-- injected bottom").unwrap();
        assert!(entry_pos < bottom_pos);
    }

    #[test]
    fn test_module_wrapper_format_no_trailing_newline_in_source_dev() {
        let graph = DependencyGraph {
            modules: vec![
                make_module("bar", "local x = 1"),
                make_module("__entry__", ""),
            ],
        };

        let emitter = Emitter::new(None, None, &BuildMode::Development);
        let (out, _sm) = emitter.emit(&graph);

        let wrapper_start = out.find("__modules[\"bar\"]").unwrap();
        let after_wrapper = &out[wrapper_start..];
        assert!(
            after_wrapper.starts_with(
                "__modules[\"bar\"] = __module_wrap(\"bar\", \"bar.lua\", function()\n"
            )
        );
        let end_pos = after_wrapper.find("end)").unwrap();
        let before_end = &after_wrapper[..end_pos];
        assert!(
            before_end.ends_with("\nlocal x = 1\n"),
            "end) must be on its own line, got: {:?}",
            &before_end[before_end.len().saturating_sub(20)..]
        );
    }

    #[test]
    fn test_module_wrapper_format_no_trailing_newline_in_source_prod() {
        let graph = DependencyGraph {
            modules: vec![
                make_module("bar", "local x = 1"),
                make_module("__entry__", ""),
            ],
        };

        let emitter = Emitter::new(None, None, &BuildMode::Production);
        let (out, _sm) = emitter.emit(&graph);

        let wrapper_start = out.find("__modules[\"bar\"]").unwrap();
        let after_wrapper = &out[wrapper_start..];
        assert!(after_wrapper.starts_with("__modules[\"bar\"] = (function()\n"));
        let end_pos = after_wrapper.find("end)()").unwrap();
        let before_end = &after_wrapper[..end_pos];
        assert!(
            before_end.ends_with("\nlocal x = 1\n"),
            "end)() must be on its own line, got: {:?}",
            &before_end[before_end.len().saturating_sub(20)..]
        );
    }

    #[test]
    fn test_sourcemap_generated() {
        let graph = DependencyGraph {
            modules: vec![
                make_module("foo", "return {}"),
                make_module("__entry__", "local f = require(\"foo\")\nprint(f)"),
            ],
        };

        let emitter = Emitter::new(None, None, &BuildMode::Development);
        let (_out, sm_json) = emitter.emit(&graph);
        let sm: serde_json::Value = serde_json::from_str(&sm_json).unwrap();

        assert_eq!(sm["version"], 3);
        assert!(sm["sources"].as_array().unwrap().len() >= 2);
        assert!(!sm["mappings"].as_str().unwrap().is_empty());
    }

    #[test]
    fn test_dev_shim_comment() {
        let graph = DependencyGraph {
            modules: vec![make_module("__entry__", "print('hello')")],
        };

        let emitter = Emitter::new(None, None, &BuildMode::Development);
        let (out, _sm) = emitter.emit(&graph);
        assert!(
            out.contains("sourceMappingURL"),
            "dev mode should include sourceMappingURL hint"
        );
    }

    #[test]
    fn test_no_sourcemap_in_production_output() {
        let graph = DependencyGraph {
            modules: vec![make_module("__entry__", "print('hello')")],
        };

        let emitter = Emitter::new(None, None, &BuildMode::Production);
        let (out, _sm) = emitter.emit(&graph);
        assert!(
            !out.contains("sourceMappingURL"),
            "production output should not have dev hints"
        );
    }
}

pub struct Emitter {
    pub inject_top: Option<String>,
    pub inject_bottom: Option<String>,
    mode: BuildMode,
}

impl Emitter {
    pub fn new(
        inject_top: Option<String>,
        inject_bottom: Option<String>,
        mode: &BuildMode,
    ) -> Self {
        Self {
            inject_top,
            inject_bottom,
            mode: mode.clone(),
        }
    }

    /// Emit the bundle and return (output_string, sourcemap_json).
    pub fn emit(&self, graph: &DependencyGraph) -> (String, String) {
        let output_name = "bundle.lua";
        let mut sm = SourceMapBuilder::new(output_name.to_string());
        let mut out = String::new();
        // Track current output line (0-indexed)
        let mut line: u32 = 0;

        // ---- header ----
        out.push_str("-- generated by lunar-bundler\n");
        line += 1;

        // ---- top injection ----
        if let Some(top) = &self.inject_top {
            out.push_str(top);
            let added = count_lines(top);
            if !top.ends_with('\n') {
                out.push('\n');
            }
            line += added + 1;
        }

        // ---- runtime shim ----
        let shim = if self.mode == BuildMode::Development {
            r#"
local __modules = {}
local __require = require
local function require(name)
    if __modules[name] ~= nil then
        return __modules[name]
    end
    return __require(name)
end

local function __module_wrap(name, path, fn)
    local ok, result = xpcall(fn, function(err)
        local msg = "module '" .. name .. "' (" .. path .. "): " .. tostring(err)
        if debug and debug.traceback then
            msg = debug.traceback(msg, 2)
        end
        return msg
    end)
    if not ok then
        error(result, 0)
    end
    return result
end
"#
        } else {
            r#"
local __modules = {}
local __require = require
local function require(name)
    if __modules[name] ~= nil then
        return __modules[name]
    end
    return __require(name)
end
"#
        };
        out.push_str(shim);
        line += count_lines(shim);

        // ---- dependencies ----
        let (entry, deps) = graph.modules.split_last().unwrap();

        for module in deps {
            line += self.emit_module(module, &mut out, &mut sm, line);
        }

        // ---- entry point ----
        self.emit_entry(entry, &mut out, &mut sm, line);

        // ---- bottom injection ----
        if let Some(bottom) = &self.inject_bottom {
            out.push_str(bottom);
            if !bottom.ends_with('\n') {
                out.push('\n');
            }
        }

        // ---- dev shim: sourceMappingURL hint ----
        if self.mode == BuildMode::Development {
            out.push_str(&format!("--# sourceMappingURL={}.map\n", output_name));
        }

        let sm_json = sm.to_json();
        (out, sm_json)
    }

    /// Emit a single dependency module (wrapped). Returns the number of
    /// output lines added.
    fn emit_module(
        &self,
        module: &Module,
        out: &mut String,
        sm: &mut SourceMapBuilder,
        start_line: u32,
    ) -> u32 {
        let path_str = module.path.display().to_string();
        let source_idx = sm.add_source(&path_str, &module.source);

        // Comment line
        out.push_str(&format!("\n-- {}\n", path_str));
        let mut lines_used = 2u32; // newline + comment

        // __modules["name"] = (function()\n  or __module_wrap(...  in dev mode
        let header = if self.mode == BuildMode::Development {
            let escaped_name = module
                .module_name
                .replace('\\', "\\\\")
                .replace('"', "\\\"");
            let escaped_path = path_str.replace('\\', "\\\\").replace('"', "\\\"");
            format!(
                "__modules[\"{name}\"] = __module_wrap(\"{name}\", \"{path}\", function()\n",
                name = escaped_name,
                path = escaped_path,
            )
        } else {
            format!(
                "__modules[\"{name}\"] = (function()\n",
                name = module.module_name
            )
        };
        out.push_str(&header);
        lines_used += 1;

        // Source content
        let source_lines: Vec<&str> = module.source.lines().collect();
        for (i, _) in source_lines.iter().enumerate() {
            let output_line = start_line + lines_used + i as u32;
            sm.map_line(output_line, source_idx, i as u32);
        }

        out.push_str(&module.source);

        // Ensure newline before `end)()` wrapper
        if !module.source.ends_with('\n') {
            out.push('\n');
            lines_used += 1;
        }

        let src_line_count = count_lines(&module.source);
        lines_used += src_line_count;

        // `end)()` in production or `end)` in dev mode
        if self.mode == BuildMode::Development {
            out.push_str("end)\n");
        } else {
            out.push_str("end)()\n");
        }
        lines_used += 1;

        lines_used
    }

    /// Emit the entry point module (raw, unwrapped).
    fn emit_entry(
        &self,
        module: &Module,
        out: &mut String,
        sm: &mut SourceMapBuilder,
        start_line: u32,
    ) {
        let path_str = module.path.display().to_string();
        let source_idx = sm.add_source(&path_str, &module.source);

        out.push_str(&format!("\n-- {}\n", path_str));
        let header_lines = 2u32;

        let source_lines: Vec<&str> = module.source.lines().collect();
        for (i, _) in source_lines.iter().enumerate() {
            let output_line = start_line + header_lines + i as u32;
            sm.map_line(output_line, source_idx, i as u32);
        }

        out.push_str(&module.source);

        if !module.source.ends_with('\n') {
            out.push('\n');
        }
    }
}

fn count_lines(s: &str) -> u32 {
    s.chars().filter(|&c| c == '\n').count() as u32
}
