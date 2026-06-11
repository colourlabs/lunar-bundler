use std::collections::HashSet;
use std::str::FromStr;

use full_moon::ast;
use full_moon::visitors::Visitor;

#[derive(Debug)]
pub struct SandboxViolation {
    pub name: String,
    pub line: usize,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum SandboxLevel {
    #[default]
    Off,
    Warn,
    Error,
}

impl FromStr for SandboxLevel {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "error" => Ok(SandboxLevel::Error),
            "warn" => Ok(SandboxLevel::Warn),
            _ => Ok(SandboxLevel::Off),
        }
    }
}

pub fn check_sandbox(source: &str, deny: &[String]) -> Vec<SandboxViolation> {
    if deny.is_empty() {
        return Vec::new();
    }

    let preprocessed = crate::preprocessor::preprocess(source);
    let ast = match full_moon::parse(&preprocessed) {
        Ok(ast) => ast,
        Err(_) => return Vec::new(),
    };

    let deny_set: HashSet<String> = deny.iter().cloned().collect();
    let mut visitor = SandboxVisitor {
        deny: deny_set,
        violations: Vec::new(),
    };
    visitor.visit_ast(&ast);
    visitor.violations
}

fn check_name(
    violations: &mut Vec<SandboxViolation>,
    deny: &HashSet<String>,
    token_ref: &full_moon::tokenizer::TokenReference,
) {
    let name_str = token_ref.token().to_string();
    if deny.contains(&name_str) {
        let line = token_ref.token().start_position().line();
        violations.push(SandboxViolation {
            name: name_str,
            line,
        });
    }
}

struct SandboxVisitor {
    deny: HashSet<String>,
    violations: Vec<SandboxViolation>,
}

impl Visitor for SandboxVisitor {
    fn visit_expression(&mut self, node: &ast::Expression) {
        match node {
            // Bare name reference: os, io, print, etc.
            ast::Expression::Symbol(token_ref) => {
                check_name(&mut self.violations, &self.deny, token_ref);
            }
            // Complex expression like os.execute() or a.b
            ast::Expression::Var(var) => match var {
                ast::Var::Name(token_ref) => {
                    check_name(&mut self.violations, &self.deny, token_ref);
                }
                ast::Var::Expression(ve) => {
                    if let ast::Prefix::Name(token_ref) = ve.prefix() {
                        check_name(&mut self.violations, &self.deny, token_ref);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn visit_function_call(&mut self, node: &ast::FunctionCall) {
        if let ast::Prefix::Name(token_ref) = node.prefix() {
            check_name(&mut self.violations, &self.deny, token_ref);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(source: &str, deny: &[&str]) -> Vec<SandboxViolation> {
        let deny: Vec<String> = deny.iter().map(|s| s.to_string()).collect();
        check_sandbox(source, &deny)
    }

    #[test]
    fn test_empty_deny_returns_nothing() {
        assert!(check("os.execute('rm')", &[]).is_empty());
    }

    #[test]
    fn test_direct_function_call_as_statement() {
        let v = check("dofile('x')", &["dofile"]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "dofile");
    }

    #[test]
    fn test_direct_function_call_in_expression() {
        let v = check("local x = dofile('x')", &["dofile"]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "dofile");
    }

    #[test]
    fn test_table_method_call() {
        let v = check("os.execute('rm')", &["os"]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "os");
    }

    #[test]
    fn test_denied_name_in_expression() {
        let v = check("local x = os", &["os"]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "os");
    }

    #[test]
    fn test_denied_name_as_argument() {
        let v = check("print(os)", &["os"]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "os");
    }

    #[test]
    fn test_nested_denied_globals() {
        let v = check("os.execute(io.open('file'))", &["os", "io"]);
        assert_eq!(v.len(), 2);
        let names: Vec<&str> = v.iter().map(|x| x.name.as_str()).collect();
        assert!(names.contains(&"os"));
        assert!(names.contains(&"io"));
    }

    #[test]
    fn test_local_declaration_not_flagged() {
        let v = check("local os = {}", &["os"]);
        assert!(v.is_empty());
    }

    #[test]
    fn test_string_literal_not_flagged() {
        let v = check(r#"local x = "os.execute('rm')""#, &["os"]);
        assert!(v.is_empty());
    }

    #[test]
    fn test_multiple_violations() {
        let v = check("os.execute('a')\nio.open('b')", &["os", "io"]);
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn test_syntax_error_silently_skipped() {
        let v = check("syntax error {{{", &["os"]);
        assert!(v.is_empty());
    }

    #[test]
    fn test_loadfile_and_load() {
        let v = check(
            r#"
            local f = loadfile('x')
            local g = load('code')
        "#,
            &["loadfile", "load"],
        );
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn test_debug_table_access() {
        let v = check("debug.sethook(function() end)", &["debug"]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "debug");
    }

    #[test]
    fn test_io_and_os() {
        let v = check("io.open('file')\nos.execute('cmd')", &["io", "os"]);
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn test_package_path_access() {
        let v = check("local p = package.path", &["package", "os"]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "package");
    }

    #[test]
    fn test_method_call_on_denied_table() {
        let v = check("debug.sethook(function() end, 'cr')", &["debug"]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "debug");
    }
}
