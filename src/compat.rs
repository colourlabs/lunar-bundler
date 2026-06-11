use std::collections::HashSet;
use std::str::FromStr;

use full_moon::ast;
use full_moon::visitors::Visitor;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CompatIssueKind {
    GotoUsed,
    ConstAttribute,
    ToBeClosedAttribute,
    IntegerDivision,
    BitwiseOps,
    BitwiseNot,
    Utf8Library,
    TableMove,
    StringPack,
    MathTointeger,
    MathType,
    FfiLibrary,
    BitLibrary,
    JitLibrary,
}

impl CompatIssueKind {
    pub fn supported_in(&self) -> &[&str] {
        match self {
            Self::GotoUsed => &["52", "53", "54", "55", "jit"],
            Self::ConstAttribute => &["54", "55"],
            Self::ToBeClosedAttribute => &["54", "55"],
            Self::BitwiseOps => &["53", "54", "55"],
            Self::BitwiseNot => &["53", "54", "55"],
            Self::IntegerDivision => &["53", "54", "55"],
            Self::FfiLibrary => &["jit"],
            Self::BitLibrary => &["jit"],
            Self::JitLibrary => &["jit"],
            Self::Utf8Library => &["53", "54", "55"],
            Self::TableMove => &["53", "54", "55"],
            Self::StringPack => &["53", "54", "55"],
            Self::MathTointeger => &["53", "54", "55"],
            Self::MathType => &["53", "54", "55"],
        }
    }

    pub fn is_issue_for(&self, target_version: &str) -> bool {
        let lowered = target_version.to_lowercase();
        let normalized = match lowered.as_str() {
            "luajit" | "jit" => "jit",
            "51" | "5.1" => "51",
            "52" | "5.2" => "52",
            "53" | "5.3" => "53",
            "54" | "5.4" => "54",
            "55" | "5.5" => "55",
            other => other,
        };
        !self.supported_in().contains(&normalized)
    }
}

impl FromStr for CompatIssueKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GotoUsed" => Ok(Self::GotoUsed),
            "ConstAttribute" => Ok(Self::ConstAttribute),
            "ToBeClosedAttribute" => Ok(Self::ToBeClosedAttribute),
            "IntegerDivision" => Ok(Self::IntegerDivision),
            "BitwiseOps" => Ok(Self::BitwiseOps),
            "BitwiseNot" => Ok(Self::BitwiseNot),
            "Utf8Library" => Ok(Self::Utf8Library),
            "TableMove" => Ok(Self::TableMove),
            "StringPack" => Ok(Self::StringPack),
            "MathTointeger" => Ok(Self::MathTointeger),
            "MathType" => Ok(Self::MathType),
            "FfiLibrary" => Ok(Self::FfiLibrary),
            "BitLibrary" => Ok(Self::BitLibrary),
            "JitLibrary" => Ok(Self::JitLibrary),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct CompatIssue {
    pub line: usize,
    pub kind: CompatIssueKind,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum CompatLevel {
    #[default]
    Off,
    Warn,
    Error,
}

pub struct CompatConfig {
    pub level: CompatLevel,
    pub ignore: Vec<CompatIssueKind>,
}

pub fn check_compat(
    source: &str,
    target_version: &str,
    ignore: &[CompatIssueKind],
) -> Vec<CompatIssue> {
    let preprocessed = crate::preprocessor::preprocess(source);
    let ast = match full_moon::parse(&preprocessed) {
        Ok(ast) => ast,
        Err(_) => return Vec::new(),
    };

    let ignore_set: HashSet<CompatIssueKind> = ignore.iter().cloned().collect();
    let mut visitor = CompatVisitor {
        target_version: target_version.to_string(),
        ignore: ignore_set,
        issues: Vec::new(),
    };
    visitor.visit_ast(&ast);
    visitor.issues
}

fn get_line_from_expr(expr: &ast::Expression) -> usize {
    match expr {
        ast::Expression::Number(t) | ast::Expression::String(t) | ast::Expression::Symbol(t) => {
            t.token().start_position().line()
        }
        ast::Expression::Var(ast::Var::Name(t)) => t.token().start_position().line(),
        ast::Expression::Var(ast::Var::Expression(ve)) => {
            if let ast::Prefix::Name(t) = ve.prefix() {
                t.token().start_position().line()
            } else {
                0
            }
        }
        ast::Expression::BinaryOperator { lhs, .. } => get_line_from_expr(lhs),
        ast::Expression::UnaryOperator { expression, .. } => get_line_from_expr(expression),
        ast::Expression::Parentheses { expression, .. } => get_line_from_expr(expression),
        _ => 0,
    }
}

fn extract_require_module(node: &ast::FunctionCall) -> Option<String> {
    let suffix = node.suffixes().next()?;
    let args = match suffix {
        ast::Suffix::Call(ast::Call::AnonymousCall(args)) => args,
        _ => return None,
    };
    let args = match args.as_ref() {
        ast::FunctionArgs::Parentheses { arguments, .. } => arguments,
        _ => return None,
    };
    if args.len() != 1 {
        return None;
    }
    match args.iter().next()? {
        ast::Expression::String(s) => {
            let raw = s.token().to_string();
            Some(raw.trim_matches(|c| c == '"' || c == '\'').to_string())
        }
        _ => None,
    }
}

fn token_line(token_ref: &full_moon::tokenizer::TokenReference) -> usize {
    token_ref.token().start_position().line()
}

struct CompatVisitor {
    target_version: String,
    ignore: HashSet<CompatIssueKind>,
    issues: Vec<CompatIssue>,
}

impl CompatVisitor {
    fn check(&mut self, kind: CompatIssueKind, line: usize) {
        if kind.is_issue_for(&self.target_version) && !self.ignore.contains(&kind) {
            self.issues.push(CompatIssue { line, kind });
        }
    }

    fn stdlib_kind(&self, prefix: &str, name: &str) -> Option<CompatIssueKind> {
        match (prefix, name) {
            ("table", "move") => Some(CompatIssueKind::TableMove),
            ("string", "pack") | ("string", "unpack") | ("string", "packsize") => {
                Some(CompatIssueKind::StringPack)
            }
            ("math", "tointeger") => Some(CompatIssueKind::MathTointeger),
            ("math", "type") => Some(CompatIssueKind::MathType),
            ("utf8", _) => Some(CompatIssueKind::Utf8Library),
            _ => None,
        }
    }
}

impl Visitor for CompatVisitor {
    fn visit_function_call(&mut self, node: &ast::FunctionCall) {
        if let ast::Prefix::Name(prefix_name) = node.prefix() {
            let prefix_str = prefix_name.token().to_string();
            let line = token_line(prefix_name);

            if prefix_str == "require" {
                if let Some(module) = extract_require_module(node) {
                    match module.as_str() {
                        "ffi" => self.check(CompatIssueKind::FfiLibrary, line),
                        "bit" => self.check(CompatIssueKind::BitLibrary, line),
                        "jit" => self.check(CompatIssueKind::JitLibrary, line),
                        "utf8" => self.check(CompatIssueKind::Utf8Library, line),
                        _ => {}
                    }
                }
            } else {
                let suffixes: Vec<_> = node.suffixes().collect();
                if suffixes.len() >= 2
                    && let ast::Suffix::Index(ast::Index::Dot { name, .. }) = &suffixes[0]
                {
                    let func_name = name.token().to_string();
                    if let Some(kind) = self.stdlib_kind(&prefix_str, &func_name) {
                        self.check(kind, token_line(name));
                    }
                }
            }
        }
    }

    fn visit_expression(&mut self, node: &ast::Expression) {
        match node {
            ast::Expression::BinaryOperator { binop, lhs, .. } => {
                let line = get_line_from_expr(lhs);
                match &binop {
                    ast::BinOp::DoubleSlash(_) => {
                        self.check(CompatIssueKind::IntegerDivision, line);
                    }
                    ast::BinOp::Ampersand(_)
                    | ast::BinOp::Pipe(_)
                    | ast::BinOp::Tilde(_)
                    | ast::BinOp::DoubleLessThan(_)
                    | ast::BinOp::DoubleGreaterThan(_) => {
                        self.check(CompatIssueKind::BitwiseOps, line);
                    }
                    _ => {}
                }
            }
            ast::Expression::UnaryOperator {
                unop: ast::UnOp::Tilde(token),
                expression: _,
            } => {
                self.check(
                    CompatIssueKind::BitwiseNot,
                    token.token().start_position().line(),
                );
            }
            ast::Expression::Var(ast::Var::Expression(ve)) => {
                if let ast::Prefix::Name(prefix_name) = ve.prefix() {
                    let prefix_str = prefix_name.token().to_string();
                    let mut suffixes = ve.suffixes();
                    if let Some(ast::Suffix::Index(ast::Index::Dot { name, .. })) = suffixes.next()
                    {
                        let func_name = name.token().to_string();
                        if let Some(kind) = self.stdlib_kind(&prefix_str, &func_name) {
                            self.check(kind, token_line(name));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn visit_stmt(&mut self, _node: &ast::Stmt) {
        if let ast::Stmt::Goto(goto) = _node {
            let line = goto.goto_token().token().start_position().line();
            self.check(CompatIssueKind::GotoUsed, line);
        }
    }

    fn visit_local_assignment(&mut self, _node: &ast::LocalAssignment) {
        for attr in _node.attributes().flatten() {
            let name = attr.name().token().to_string();
            let line = attr.name().token().start_position().line();
            match name.as_str() {
                "const" => self.check(CompatIssueKind::ConstAttribute, line),
                "close" => self.check(CompatIssueKind::ToBeClosedAttribute, line),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(source: &str, target: &str, ignore: &[CompatIssueKind]) -> Vec<CompatIssueKind> {
        check_compat(source, target, ignore)
            .into_iter()
            .map(|i| i.kind)
            .collect()
    }

    #[test]
    fn test_syntax_error_silently_skipped() {
        let kinds = check("syntax error {{{", "51", &[]);
        assert!(kinds.is_empty());
    }

    #[test]
    fn test_require_ffi() {
        let kinds = check(r#"require("ffi")"#, "51", &[]);
        assert!(kinds.contains(&CompatIssueKind::FfiLibrary));
    }

    #[test]
    fn test_require_bit() {
        let kinds = check(r#"require("bit")"#, "51", &[]);
        assert!(kinds.contains(&CompatIssueKind::BitLibrary));
    }

    #[test]
    fn test_require_jit() {
        let kinds = check(r#"require("jit")"#, "51", &[]);
        assert!(kinds.contains(&CompatIssueKind::JitLibrary));
    }

    #[test]
    fn test_require_utf8() {
        let kinds = check(r#"require("utf8")"#, "51", &[]);
        assert!(kinds.contains(&CompatIssueKind::Utf8Library));
    }

    #[test]
    fn test_require_utf8_53_not_flagged() {
        let kinds = check(r#"require("utf8")"#, "53", &[]);
        assert!(!kinds.contains(&CompatIssueKind::Utf8Library));
    }

    #[test]
    fn test_table_move() {
        let kinds = check("table.move({}, 1, 2, 3)", "51", &[]);
        assert!(kinds.contains(&CompatIssueKind::TableMove));
    }

    #[test]
    fn test_string_pack() {
        let kinds = check(r#"string.pack("I4", 42)"#, "51", &[]);
        assert!(kinds.contains(&CompatIssueKind::StringPack));
    }

    #[test]
    fn test_math_tointeger() {
        let kinds = check("math.tointeger(3.0)", "51", &[]);
        assert!(kinds.contains(&CompatIssueKind::MathTointeger));
    }

    #[test]
    fn test_math_type() {
        let kinds = check("print(math.type(3))", "51", &[]);
        assert!(kinds.contains(&CompatIssueKind::MathType));
    }

    #[test]
    fn test_bitwise_ops() {
        let kinds = check("local x = 1 & 2", "51", &[]);
        assert!(kinds.contains(&CompatIssueKind::BitwiseOps));
    }

    #[test]
    fn test_integer_division() {
        let kinds = check("local x = 5 // 2", "51", &[]);
        assert!(kinds.contains(&CompatIssueKind::IntegerDivision));
    }

    #[test]
    fn test_bitwise_not() {
        let kinds = check("local x = ~5", "51", &[]);
        assert!(kinds.contains(&CompatIssueKind::BitwiseNot));
    }

    #[test]
    fn test_53_not_flagged_for_bitwise() {
        let kinds = check("local x = 1 & 2", "53", &[]);
        assert!(!kinds.contains(&CompatIssueKind::BitwiseOps));
    }

    #[test]
    fn test_ignore_works() {
        let kinds = check(r#"require("ffi")"#, "51", &[CompatIssueKind::FfiLibrary]);
        assert!(kinds.is_empty());
    }

    #[test]
    fn test_goto_detected() {
        let kinds = check("::label::\ngoto label\n", "51", &[]);
        assert!(kinds.contains(&CompatIssueKind::GotoUsed));
    }

    #[test]
    fn test_utf8_method_call() {
        let kinds = check("local n = utf8.len('hello')", "51", &[]);
        assert!(kinds.contains(&CompatIssueKind::Utf8Library));
    }

    #[test]
    fn test_table_var_reference() {
        let kinds = check("local f = table.move", "51", &[]);
        assert!(kinds.contains(&CompatIssueKind::TableMove));
    }

    #[test]
    fn test_version_normalization() {
        // "luajit" and "jit" should not flag FfiLibrary
        let kinds_luajit = check(r#"require("ffi")"#, "luajit", &[]);
        assert!(!kinds_luajit.contains(&CompatIssueKind::FfiLibrary));

        let kinds_jit = check(r#"require("ffi")"#, "jit", &[]);
        assert!(!kinds_jit.contains(&CompatIssueKind::FfiLibrary));

        // "5.3" and "53" should not flag BitwiseOps
        let kinds_53 = check("local x = 1 & 2", "5.3", &[]);
        assert!(!kinds_53.contains(&CompatIssueKind::BitwiseOps));

        let kinds_53_no_dot = check("local x = 1 & 2", "53", &[]);
        assert!(!kinds_53_no_dot.contains(&CompatIssueKind::BitwiseOps));
    }
}
