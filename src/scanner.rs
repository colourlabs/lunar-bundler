use std::path::PathBuf;

use anyhow::Result;
use full_moon::{ast, visitors::Visitor};

use crate::error::BundlerError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_require() {
        let result = scan_requires(
            r#"local x = require("foo.bar")"#,
            &PathBuf::from("test.lua"),
        )
        .unwrap();
        assert_eq!(result.requires, vec!["foo.bar"]);
        assert!(result.dynamic_requires.is_empty());
    }

    #[test]
    fn test_single_quotes() {
        let result = scan_requires(
            r#"local x = require('foo.bar')"#,
            &PathBuf::from("test.lua"),
        )
        .unwrap();
        assert_eq!(result.requires, vec!["foo.bar"]);
    }

    #[test]
    fn test_dynamic_require_tracked() {
        let result =
            scan_requires(r#"local x = require(some_var)"#, &PathBuf::from("test.lua")).unwrap();
        assert!(result.requires.is_empty());
        assert_eq!(result.dynamic_requires.len(), 1);
        assert!(result.dynamic_requires[0].contains("test.lua"));
    }

    #[test]
    fn test_multiple_requires() {
        let src = r#"
            local a = require("foo")
            local b = require("bar.baz")
        "#;
        let result = scan_requires(src, &PathBuf::from("test.lua")).unwrap();
        assert_eq!(result.requires, vec!["foo", "bar.baz"]);
    }

    #[test]
    fn test_lua55_global_ignored() {
        let src = r#"
            global print, os
            local a = require("foo")
        "#;
        let result = scan_requires(src, &PathBuf::from("test.lua")).unwrap();
        assert_eq!(result.requires, vec!["foo"]);
    }
}

pub struct ScanResult {
    pub requires: Vec<String>,
    pub dynamic_requires: Vec<String>, // locations of unresolvable dynamic requires
}

#[derive(Default)]
struct RequireVisitor {
    requires: Vec<String>,
    dynamic_requires: Vec<String>,
    path: String,
}

impl Visitor for RequireVisitor {
    fn visit_function_call(&mut self, call: &ast::FunctionCall) {
        // check prefix is "require"
        let name = match call.prefix() {
            ast::Prefix::Name(name) => name.token().to_string(),
            _ => return,
        };
        if name != "require" {
            return;
        }

        // get the first suffix
        let suffix = match call.suffixes().next() {
            Some(s) => s,
            None => return,
        };

        // must be a function call suffix
        let args = match suffix {
            ast::Suffix::Call(ast::Call::AnonymousCall(args)) => args,
            _ => return,
        };

        // must be parenthesised args
        let args = match args.as_ref() {
            ast::FunctionArgs::Parentheses { arguments, .. } => arguments,
            _ => return,
        };

        // must be exactly one argument
        if args.len() != 1 {
            return;
        }

        match args.iter().next() {
            Some(ast::Expression::String(s)) => {
                // static string literal - resolvable
                let raw = s.token().to_string();
                let module = raw.trim_matches(|c| c == '"' || c == '\'').to_string();
                self.requires.push(module);
            }
            _ => {
                // dynamic require - unresolvable at bundle time
                self.dynamic_requires.push(self.path.clone());
            }
        }
    }
}

pub fn scan_requires(source: &str, path: &PathBuf) -> Result<ScanResult> {
    let preprocessed = crate::preprocessor::preprocess(source);
    let ast = full_moon::parse(&preprocessed).map_err(|e| BundlerError::ParseError {
        path: path.clone(),
        reason: format!("{:?}", e),
    })?;
    let mut visitor = RequireVisitor {
        requires: vec![],
        dynamic_requires: vec![],
        path: path.display().to_string(),
    };
    visitor.visit_ast(&ast);
    Ok(ScanResult {
        requires: visitor.requires,
        dynamic_requires: visitor.dynamic_requires,
    })
}
