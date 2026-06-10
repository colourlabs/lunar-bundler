/// Basic Lua minification for production builds.
///
/// Strips comments and unnecessary whitespace while preserving string
/// and multiline-string contents.
pub fn minify_lua(source: &str) -> String {
    let stripped = strip_comments(source);
    collapse_whitespace(&stripped)
}

/// Strip `--` line comments and `--[[ ... ]]` block comments, preserving
/// string literals so `"-- not a comment"` is left alone.
fn strip_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len);
    let mut i = 0;

    while i < len {
        // String literal — pass through unchanged
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            out.push(quote as char);
            i += 1;
            while i < len && bytes[i] != quote {
                if bytes[i] == b'\\' && i + 1 < len {
                    out.push(bytes[i] as char);
                    i += 1;
                    out.push(bytes[i] as char);
                    i += 1;
                } else {
                    out.push(bytes[i] as char);
                    i += 1;
                }
            }
            if i < len {
                out.push(bytes[i] as char);
                i += 1;
            }
            continue;
        }

        // Long string `[[ ... ]]` — pass through
        if bytes[i] == b'[' && i + 1 < len && bytes[i + 1] == b'[' {
            out.push_str("[[");
            i += 2;
            let mut depth = 1;
            while i + 1 < len && depth > 0 {
                if bytes[i] == b']' && bytes[i + 1] == b']' {
                    out.push_str("]]");
                    i += 2;
                    depth -= 1;
                } else {
                    if bytes[i] == b'[' && i + 1 < len && bytes[i + 1] == b'[' {
                        depth += 1;
                    }
                    out.push(bytes[i] as char);
                    i += 1;
                }
            }
            continue;
        }

        // `--` starts a comment (either `-- line` or `--[[ block ]]`)
        if bytes[i] == b'-' && i + 1 < len && bytes[i + 1] == b'-' {
            // Check for `--[[` (block comment)
            if i + 2 < len && bytes[i + 2] == b'[' {
                // Eat the `--[[`
                i += 3;
                // Find closing `]]`
                while i + 1 < len {
                    if bytes[i] == b']' && bytes[i + 1] == b']' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                continue;
            }

            // Line comment `--` — skip to end of line
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            // Preserve the newline (or end of file)
            continue;
        }

        out.push(bytes[i] as char);
        i += 1;
    }

    out
}

/// Collapse repeated blank lines into one, trim trailing whitespace on
/// each line, and strip leading/trailing blank lines.
fn collapse_whitespace(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut prev_blank = false;

    for line in source.lines() {
        let trimmed = line.trim_end();
        let is_blank = trimmed.is_empty();

        if is_blank && prev_blank {
            continue;
        }

        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(trimmed);
        prev_blank = is_blank;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_line_comment() {
        assert_eq!(strip_comments("x = 1 -- this is a comment\ny = 2"), "x = 1 \ny = 2");
    }

    #[test]
    fn test_strip_block_comment() {
        assert_eq!(
            strip_comments("x = 1 --[[ block\ncomment ]]\ny = 2"),
            "x = 1 \ny = 2"
        );
    }

    #[test]
    fn test_comment_not_stripped_in_string() {
        assert_eq!(
            strip_comments(r#"s = "-- not a comment""#),
            r#"s = "-- not a comment""#
        );
    }

    #[test]
    fn test_long_string_preserved() {
        assert_eq!(
            strip_comments("s = [[hello -- not a comment\nworld]]"),
            "s = [[hello -- not a comment\nworld]]"
        );
    }

    #[test]
    fn test_collapse_whitespace() {
        let input = "  hello  \n\n\nworld\n\n  ";
        assert_eq!(collapse_whitespace(input), "  hello\n\nworld\n");
    }

    #[test]
    fn test_minify_full() {
        let input = "-- header comment\nlocal x = 1 -- inline\n\n\nlocal y = 2\n--[[ block ]]\n";
        let result = minify_lua(input);
        assert!(!result.contains("header comment"));
        assert!(!result.contains("inline"));
        assert!(!result.contains("block"));
        assert!(result.contains("local x = 1"));
        assert!(result.contains("local y = 2"));
        // No repeated blank lines
        let lines: Vec<_> = result.lines().collect();
        assert!(lines.len() <= 3);
    }

    #[test]
    fn test_no_false_positives() {
        let input = r#"print("hello -- world")"#;
        assert_eq!(strip_comments(input), input);
    }
}
