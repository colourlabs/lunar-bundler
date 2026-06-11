use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use full_moon::ast::{self, punctuated::Punctuated, Expression, Stmt};
use full_moon::node::Node;
use full_moon::visitors::Visitor;

/// Level of tree shaking to apply
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TreeShakeLevel {
    Off,
    /// Remove unused local variables and local function declarations
    Basic,
    /// Remove unused locals + attempt cross-module unused export removal
    Aggressive,
}

impl Default for TreeShakeLevel {
    fn default() -> Self {
        Self::Off
    }
}

impl FromStr for TreeShakeLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "off" => Ok(Self::Off),
            "basic" => Ok(Self::Basic),
            "aggressive" | "full" => Ok(Self::Aggressive),
            other => Err(format!(
                "unknown tree shake level '{}', expected: off, basic, or aggressive",
                other
            )),
        }
    }
}

/// Apply tree shaking to a Lua source string.
pub fn treeshake(source: &str, level: TreeShakeLevel) -> String {
    match level {
        TreeShakeLevel::Off => source.to_string(),
        TreeShakeLevel::Basic => remove_unused_locals(source),
        TreeShakeLevel::Aggressive => {
            let s = remove_unused_locals(source);
            // Aggressive could add cross-module analysis in the future
            s
        }
    }
}

/// Collects local variable names and reference usage from the AST.
#[derive(Default)]
struct UsageCollector {
    /// Names of local variables declared (from `local x` and `local function x()`)
    locals: HashSet<String>,
    /// Names that are referenced as expressions or function calls
    references: HashSet<String>,
    /// Count of references per name (to distinguish single-reference from
    /// multi-reference when needed)
    ref_counts: HashMap<String, usize>,
}

impl Visitor for UsageCollector {
    fn visit_local_assignment(&mut self, node: &ast::LocalAssignment) {
        let mut names = Vec::new();
        for name in node.names().iter() {
            let text = name.token().to_string().trim().to_string();
            names.push(text);
        }

        // Check for `local x = x` patterns - the RHS reference might refer
        // to the same name in an outer scope. We need to handle this carefully:
        // we add locals first, then let expressions accumulate references.
        // To avoid counting the RHS reference as a use of the same-named local
        // (which would prevent removal), we note these names for post-processing.
        if node.expressions().is_empty() {
            // No RHS - it's `local x` (declaration only)
        }

        for name in names {
            self.locals.insert(name);
        }
    }

    fn visit_local_function(&mut self, node: &ast::LocalFunction) {
        let name = node.name().token().to_string().trim().to_string();
        self.locals.insert(name);
    }

    fn visit_expression(&mut self, node: &Expression) {
        match node {
            // `x` appearing as a bare expression (e.g. `return x`, `x + 1`)
            Expression::Symbol(token) => {
                let text = token.token().to_string();
                // Only count identifiers, not keywords like true/false/nil
                if is_identifier_token(&text) {
                    let name = text.trim().to_string();
                    self.references.insert(name.clone());
                    *self.ref_counts.entry(name).or_insert(0) += 1;
                }
            }
            // Variable reference like `x.y` - prefix is `x`
            Expression::Var(var) => match var {
                ast::Var::Name(token) => {
                    let text = token.token().to_string().trim().to_string();
                    self.references.insert(text.clone());
                    *self.ref_counts.entry(text).or_insert(0) += 1;
                }
                ast::Var::Expression(ve) => {
                    if let ast::Prefix::Name(token) = ve.prefix() {
                        let text = token.token().to_string().trim().to_string();
                        self.references.insert(text.clone());
                        *self.ref_counts.entry(text).or_insert(0) += 1;
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn visit_function_call(&mut self, node: &ast::FunctionCall) {
        if let ast::Prefix::Name(token) = node.prefix() {
            let text = token.token().to_string().trim().to_string();
            self.references.insert(text.clone());
            *self.ref_counts.entry(text).or_insert(0) += 1;
        }
    }
}

/// Checks if the text looks like an identifier (not a keyword).
fn is_identifier_token(text: &str) -> bool {
    let trimmed = text.trim();
    !matches!(
        trimmed,
        "true" | "false" | "nil" | "and" | "or" | "not" | "function" | "end"
            | "if" | "then" | "else" | "elseif" | "for" | "while" | "repeat"
            | "until" | "do" | "break" | "return" | "local" | "in" | "goto"
            | "global"
    )
}

/// Check if an expression is a pure literal (no side effects).
fn is_pure_expression(expr: &Expression) -> bool {
    match expr {
        Expression::Number(_) | Expression::String(_) => true,
        Expression::Symbol(token) => {
            let text = token.token().to_string();
            let trimmed = text.trim();
            matches!(trimmed, "true" | "false" | "nil")
        }
        Expression::Parentheses { expression, .. } => is_pure_expression(expression),
        _ => false,
    }
}

/// Check if a list of expressions all have no side effects.
fn all_pure_expressions(
    exprs: &Punctuated<Expression>,
) -> bool {
    exprs.iter().all(is_pure_expression)
}

/// Remove unused local variable declarations from the source.
fn remove_unused_locals(source: &str) -> String {
    let preprocessed = crate::preprocessor::preprocess(source);
    let ast = match full_moon::parse(&preprocessed) {
        Ok(ast) => ast,
        Err(_) => return source.to_string(),
    };

    let mut collector = UsageCollector::default();
    collector.visit_ast(&ast);

    // Determine unused locals
    let unused: HashSet<String> = collector
        .locals
        .iter()
        .filter(|name| !collector.references.contains(*name))
        .cloned()
        .collect();

    if unused.is_empty() {
        return preprocessed;
    }

    // Collect byte ranges to remove from the preprocessed source
    let block = ast.nodes();
    let mut removals: Vec<(usize, usize)> = Vec::new();

    // Walk top-level statements
    for (stmt, semi) in block.stmts_with_semicolon() {
        if let Some((start, end)) = stmt_removal_range(stmt, &unused) {
            // Extend to include the optional semicolon
            let end = semi
                .as_ref()
                .and_then(|s| s.end_position())
                .map(|p| p.bytes())
                .unwrap_or(end);
            // Extend to end of line to remove the newline too
            let end = extend_to_line(&preprocessed, end);
            removals.push((start, end));
        }
    }

    // Walk the last statement (return/break)
    if let Some((last_stmt, semi)) = block.last_stmt_with_semicolon() {
        if let Some((start, end)) = last_stmt_removal_range(last_stmt, &unused) {
            let end = semi
                .as_ref()
                .and_then(|s| s.end_position())
                .map(|p| p.bytes())
                .unwrap_or(end);
            let end = extend_to_line(&preprocessed, end);
            removals.push((start, end));
        }
    }

    if removals.is_empty() {
        return preprocessed;
    }

    // Apply removals in reverse byte order to preserve positions
    removals.sort_by(|a, b| b.0.cmp(&a.0));
    let mut result = preprocessed;
    for (start, end) in &removals {
        if *start < result.len() && *end <= result.len() && *start < *end {
            result.replace_range(*start..*end, "");
        }
    }

    result
}

/// Get byte range to remove for a statement, or None if it should stay.
fn stmt_removal_range(
    stmt: &Stmt,
    unused: &HashSet<String>,
) -> Option<(usize, usize)> {
    let start = stmt.start_position()?.bytes();
    let end = stmt.end_position()?.bytes();

    match stmt {
        Stmt::LocalAssignment(assign) => {
            // Check if ALL assigned names are unused
            let all_unused = assign.names().iter().all(|name| {
                let text = name.token().to_string().trim().to_string();
                unused.contains(&text)
            });

            if !all_unused {
                return None;
            }

            // Only remove if the RHS has no side effects (or is empty)
            if !all_pure_expressions(assign.expressions()) {
                return None;
            }

            Some((start, end))
        }
        Stmt::LocalFunction(lf) => {
            let name = lf.name().token().to_string().trim().to_string();
            if unused.contains(&name) {
                Some((start, end))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Get byte range to remove for a last statement, or None.
fn last_stmt_removal_range(
    last_stmt: &full_moon::ast::LastStmt,
    _unused: &HashSet<String>,
) -> Option<(usize, usize)> {
    // We don't remove return/break statements for now
    let _ = last_stmt;
    None
}

/// Extend a byte offset to the end of the line (inclusive of newline).
fn extend_to_line(source: &str, pos: usize) -> usize {
    if pos >= source.len() {
        return source.len();
    }
    let rest = &source[pos..];
    let newline_pos = rest.find('\n');
    match newline_pos {
        Some(nl) => pos + nl + 1, // include the newline
        None => source.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_shake_level_from_str() {
        assert_eq!("off".parse::<TreeShakeLevel>().unwrap(), TreeShakeLevel::Off);
        assert_eq!("basic".parse::<TreeShakeLevel>().unwrap(), TreeShakeLevel::Basic);
        assert_eq!("aggressive".parse::<TreeShakeLevel>().unwrap(), TreeShakeLevel::Aggressive);
        assert_eq!("full".parse::<TreeShakeLevel>().unwrap(), TreeShakeLevel::Aggressive);
        assert!("unknown".parse::<TreeShakeLevel>().is_err());
    }

    #[test]
    fn test_unused_local_no_init() {
        let source = "local unused\nlocal used = 1\nprint(used)\n";
        let result = treeshake(source, TreeShakeLevel::Basic);
        assert!(!result.contains("local unused"), "should remove unused local");
        assert!(result.contains("local used = 1"), "should keep used local");
        assert!(result.contains("print(used)"), "should keep print call");
    }

    #[test]
    fn test_unused_local_with_literal_init() {
        let source = "local unused = 5\nlocal used = 1\nprint(used)\n";
        let result = treeshake(source, TreeShakeLevel::Basic);
        assert!(!result.contains("local unused = 5"), "should remove unused literal init");
        assert!(result.contains("local used = 1"), "should keep used local");
    }

    #[test]
    fn test_unused_local_function() {
        let source = "local function helper() return 1 end\nlocal function used() return 2 end\nprint(used())\n";
        let result = treeshake(source, TreeShakeLevel::Basic);
        assert!(!result.contains("helper"), "should remove unused local function");
        assert!(result.contains("used"), "should keep used function");
    }

    #[test]
    fn test_recursive_function_kept() {
        let source = "local function recurse(n) if n > 0 then return recurse(n - 1) else return 0 end end\nprint(recurse(5))\n";
        let result = treeshake(source, TreeShakeLevel::Basic);
        assert!(result.contains("recurse"), "should keep recursive function");
    }

    #[test]
    fn test_used_variable_kept() {
        let source = "local x = 1\nlocal y = x + 1\nprint(y)\n";
        let result = treeshake(source, TreeShakeLevel::Basic);
        assert!(result.contains("local x = 1"), "x is used by y's init");
        assert!(result.contains("local y = x + 1"), "y is used by print");
    }

    #[test]
    fn test_mutual_recursion_kept() {
        let source = "local function a() return b() end\nlocal function b() return a() end\nprint(a())\n";
        let result = treeshake(source, TreeShakeLevel::Basic);
        assert!(result.contains("function a()"), "a is used");
        assert!(result.contains("function b()"), "b is called by a");
    }

    #[test]
    fn test_table_return_not_removed() {
        let source = "local t = { a = 1, b = 2 }\nreturn t\n";
        let result = treeshake(source, TreeShakeLevel::Basic);
        assert!(result.contains("local t ="), "table constructor stays (not pure)");
    }

    #[test]
    fn test_syntax_error_returns_original() {
        let source = "syntax error {{{";
        let result = treeshake(source, TreeShakeLevel::Basic);
        assert_eq!(result, source);
    }

    #[test]
    fn test_off_level_does_nothing() {
        let source = "local unused = 5\nlocal used = 1\nprint(used)\n";
        let result = treeshake(source, TreeShakeLevel::Off);
        assert_eq!(result, source);
    }

    #[test]
    fn test_true_false_nil_are_pure() {
        let source = "local a = true\nlocal b = false\nlocal c = nil\nlocal used = 1\nprint(used)\n";
        let result = treeshake(source, TreeShakeLevel::Basic);
        assert!(!result.contains("local a = true"), "true literal is pure, so removed");
        assert!(!result.contains("local b = false"), "false literal is pure, so removed");
        assert!(!result.contains("local c = nil"), "nil literal is pure, so removed");
        assert!(result.contains("local used = 1"), "should keep used local");
    }

    #[test]
    fn test_side_effect_expression_not_removed() {
        let source = "local x = io.open('file')\nlocal y = 1\nprint(y)\n";
        let result = treeshake(source, TreeShakeLevel::Basic);
        // x has a side-effect RHS, should NOT be removed even if unused
        assert!(result.contains("local x = io.open"), "side-effect expression keeps the statement");
        assert!(result.contains("local y = 1"));
    }

    #[test]
    fn test_unused_multiple_locals() {
        let source = "local a, b = 1, 2\nlocal c = 3\nlocal d = 4\nprint(c)\n";
        let result = treeshake(source, TreeShakeLevel::Basic);
        // a, b are unused but they share a local stmt with pure literals - removed
        assert!(!result.contains("local a, b = 1, 2"), "all names unused and pure -> removed");
        assert!(!result.contains("local d = 4"), "d unused -> removed");
        assert!(result.contains("local c = 3"), "c is used");
    }

    #[test]
    fn test_aggressive_includes_basic() {
        let source = "local unused = 5\nlocal used = 1\nprint(used)\n";
        let result = treeshake(source, TreeShakeLevel::Aggressive);
        assert!(!result.contains("local unused = 5"));
        assert!(result.contains("local used = 1"));
    }
}
