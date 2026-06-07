use std::path::PathBuf;

use anyhow::Result;
use full_moon::{ast, visitors::Visitor};

use crate::error::BundlerError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_require() {
        let src = r#"local x = require("foo.bar")"#;
        assert_eq!(scan_requires(src, &PathBuf::from("test.lua")).unwrap(), vec!["foo.bar"]);
    }

    #[test]
    fn test_single_quotes() {
        let src = r#"local x = require('foo.bar')"#;
        assert_eq!(scan_requires(src, &PathBuf::from("test.lua")).unwrap(), vec!["foo.bar"]);
    }

    #[test]
    fn test_dynamic_require_ignored() {
        let src = r#"local x = require(some_var)"#;
        assert_eq!(scan_requires(src, &PathBuf::from("test.lua")).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn test_multiple_requires() {
        let src = r#"
            local a = require("foo")
            local b = require("bar.baz")
        "#;
        assert_eq!(scan_requires(src, &PathBuf::from("test.lua")).unwrap(), vec!["foo", "bar.baz"]);
    }

    #[test]
    fn test_lua55_global_ignored() {
        let src = r#"
            global print, os
            local a = require("foo")
        "#;
        assert_eq!(scan_requires(src, &PathBuf::from("test.lua")).unwrap(), vec!["foo"]);
    }
}

#[derive(Default)]
struct RequireVisitor {
    requires: Vec<String>,
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

        // must be a string literal
        let expr = match args.iter().next() {
            Some(ast::Expression::String(s)) => s,
            _ => return,
        };

        // strip the surrounding quotes
        let raw = expr.token().to_string();
        let module = raw.trim_matches(|c| c == '"' || c == '\'').to_string();
        self.requires.push(module);
    }
}

pub fn scan_requires(source: &str, path: &PathBuf) -> Result<Vec<String>> {
    let preprocessed = crate::preprocessor::preprocess(source);
    let ast = full_moon::parse(&preprocessed)
        .map_err(|e| BundlerError::ParseError {
            path: path.clone(),
            reason: format!("{:?}", e),
        })?;
    let mut visitor = RequireVisitor::default();
    visitor.visit_ast(&ast);
    Ok(visitor.requires)
}