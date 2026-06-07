pub fn preprocess(source: &str) -> String {
    // remove 'global' syntax from Lua 5.5 files
    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("global ") {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}