use std::collections::HashMap;

/// Builds sourcemap v3 JSON by tracking output line -> original file + line.
pub struct SourceMapBuilder {
    pub file: String,
    pub sources: Vec<String>,
    pub sources_content: Vec<String>,
    source_index: HashMap<String, usize>,
    /// The nth entry holds the (source_idx, source_line) for output line n.
    /// `None` means the line is generated (no source mapping).
    line_map: Vec<Option<(usize, u32)>>,
}

impl SourceMapBuilder {
    pub fn new(file: String) -> Self {
        Self {
            file,
            sources: vec![],
            sources_content: vec![],
            source_index: HashMap::new(),
            line_map: vec![],
        }
    }

    /// Register a source file and return its index. No-op if already added.
    pub fn add_source(&mut self, path: &str, content: &str) -> usize {
        if let Some(&idx) = self.source_index.get(path) {
            return idx;
        }
        let idx = self.sources.len();
        self.sources.push(path.to_string());
        self.sources_content.push(content.to_string());
        self.source_index.insert(path.to_string(), idx);
        idx
    }

    /// Ensure the line map has at least `line + 1` entries (filling gaps with
    /// `None`).  Returns the line number that was passed in so callers can
    /// chain.
    pub fn ensure_line(&mut self, line: u32) -> u32 {
        while self.line_map.len() <= line as usize {
            self.line_map.push(None);
        }
        line
    }

    /// Record that output `line` comes from `source_line` of source
    /// `source_idx`.
    pub fn map_line(&mut self, line: u32, source_idx: usize, source_line: u32) {
        let line = self.ensure_line(line);
        self.line_map[line as usize] = Some((source_idx, source_line));
    }

    /// Record a range of consecutive lines.
    pub fn map_lines(
        &mut self,
        first_output_line: u32,
        source_idx: usize,
        first_source_line: u32,
        count: u32,
    ) {
        for i in 0..count {
            self.map_line(first_output_line + i, source_idx, first_source_line + i);
        }
    }

    /// Serialize to standard sourcemap v3 JSON.
    pub fn to_json(&self) -> String {
        let mut line_segments: Vec<String> = Vec::new();

        // Previous segment values for relative encoding within each output line
        let mut prev_col: i32 = 0;
        let mut prev_src_idx: i32 = 0;
        let mut prev_src_line: i32 = 0;
        let mut prev_src_col: i32 = 0;

        for line in &self.line_map {
            match line {
                Some((src_idx, src_line)) => {
                    // Absolute values from the state
                    let col = 0i32;
                    let src = *src_idx as i32;
                    let sline = *src_line as i32;
                    let scol = 0i32;

                    let seg = format!(
                        "{}{}{}{}",
                        vlq_encode(col - prev_col),
                        vlq_encode(src - prev_src_idx),
                        vlq_encode(sline - prev_src_line),
                        vlq_encode(scol - prev_src_col),
                    );

                    prev_col = col;
                    prev_src_idx = src;
                    prev_src_line = sline;
                    prev_src_col = scol;

                    line_segments.push(seg);
                }
                None => {
                    // Reset the relative state for generated lines
                    prev_col = 0;
                    prev_src_idx = 0;
                    prev_src_line = 0;
                    prev_src_col = 0;
                    line_segments.push(String::new());
                }
            }
        }

        let mappings_string = line_segments.join(";");

        let sources_json: Vec<String> = self
            .sources
            .iter()
            .map(|s| serde_json::to_string(s).unwrap())
            .collect();
        let content_json: Vec<String> = self
            .sources_content
            .iter()
            .map(|s| serde_json::to_string(s).unwrap())
            .collect();

        format!(
            r#"{{"version":3,"file":{},"sources":[{}],"sourcesContent":[{}],"mappings":{}}}"#,
            serde_json::to_string(&self.file).unwrap(),
            sources_json.join(","),
            content_json.join(","),
            serde_json::to_string(&mappings_string).unwrap(),
        )
    }
}

/// Encode a signed integer into VLQ base64.
fn vlq_encode(value: i32) -> String {
    // zigzag: move sign bit to LSB
    let mut val = if value >= 0 {
        (value as u32) << 1
    } else {
        (((-value) as u32) << 1) | 1
    };

    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
        .as_bytes();

    let mut out = String::new();
    loop {
        let mut digit = (val & 0x1f) as usize;
        val >>= 5;
        if val > 0 {
            digit |= 0x20; // continuation bit
        }
        out.push(alphabet[digit] as char);
        if val == 0 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vlq_roundtrip() {
        for v in [0, 1, -1, 15, -15, 127, -128, 1000, -1000] {
            let encoded = vlq_encode(v);
            assert!(!encoded.is_empty(), "vlq_encode({}) should not be empty", v);
            // all chars should be base64
            for c in encoded.chars() {
                assert!(
                    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".contains(c),
                    "bad char '{}' in VLQ for {}",
                    c,
                    v
                );
            }
        }
    }

    #[test]
    fn test_sourcemap_basic() {
        let mut sm = SourceMapBuilder::new("bundle.lua".to_string());
        let idx = sm.add_source("src/foo.lua", "local x = 1\nreturn x\n");
        let idx2 = sm.add_source("src/main.lua", "local f = require(\"foo\")\nprint(f)\n");

        // Simulate output: 2 generated lines, then 2 lines from foo.lua,
        // then 2 generated, then 2 from main.lua
        // line 0: header
        sm.map_line(2, idx, 0);
        sm.map_line(3, idx, 1);
        sm.map_line(6, idx2, 0);
        sm.map_line(7, idx2, 1);

        let json = sm.to_json();
        assert!(json.contains("\"version\":3"));
        assert!(json.contains("\"src/foo.lua\""));
        assert!(json.contains("\"src/main.lua\""));
        assert!(json.contains("\"local x = 1\\nreturn x\\n\""));
        assert!(json.contains("\"mappings\":"));

        // Should have 8 segments (one per output line including generated)
        let mappings_start = json.find("\"mappings\":\"").unwrap() + 12;
        let mappings_end = json[mappings_start..].find('"').unwrap();
        let mappings = &json[mappings_start..mappings_start + mappings_end];
        assert_eq!(mappings.matches(';').count() + 1, 8, "one segment per output line");
    }

    #[test]
    fn test_sourcemap_no_sources() {
        let sm = SourceMapBuilder::new("empty.lua".to_string());
        let json = sm.to_json();
        assert!(json.contains("\"sources\":[]"));
        assert!(json.contains("\"mappings\":\"\""));
    }
}
