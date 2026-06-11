use std::collections::HashMap;
use std::io::Write;

use crate::{BuildMode, Loader, LoaderContext};

/// Create a Loader that compiles Teal via `tl gen` using temp files.
/// Works on all platforms (no stdin pipe reliance).
pub fn teal_loader() -> Loader {
    Box::new(move |ctx: LoaderContext| {
        let dir = tempfile::tempdir()?;
        let tl_path = dir.path().join("input.tl");
        let lua_path = dir.path().join("input.lua");

        std::fs::write(&tl_path, &ctx.source)
            .map_err(|e| anyhow::anyhow!("failed to write temp .tl file: {}", e))?;

        let output = std::process::Command::new("tl")
            .arg("gen")
            .arg(&tl_path)
            .arg("--output")
            .arg(&lua_path)
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run tl: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("tl gen failed:\n{}", stderr.trim());
        }

        let result = std::fs::read_to_string(&lua_path)
            .map_err(|e| anyhow::anyhow!("tl did not produce output .lua file: {}", e))?;

        Ok(result)
    })
}

/// Create a Loader that compiles Moonscript via `moonc` using temp files.
/// Works on all platforms (no stdin pipe reliance).
pub fn moonscript_loader() -> Loader {
    Box::new(move |ctx: LoaderContext| {
        let dir = tempfile::tempdir()?;
        let moon_path = dir.path().join("input.moon");
        let lua_path = dir.path().join("input.lua");

        std::fs::write(&moon_path, &ctx.source)
            .map_err(|e| anyhow::anyhow!("failed to write temp .moon file: {}", e))?;

        let output = std::process::Command::new("moonc")
            .arg("-o")
            .arg(&lua_path)
            .arg(&moon_path)
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run moonc: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("moonc failed:\n{}", stderr.trim());
        }

        let result = std::fs::read_to_string(&lua_path)
            .map_err(|e| anyhow::anyhow!("moonc did not produce output .lua file: {}", e))?;

        Ok(result)
    })
}

/// Create a Loader that pipes source through a shell command.
/// The command receives the source on stdin and must emit the
/// transformed source on stdout.
pub fn command_loader(command: &str) -> Loader {
    let command = command.to_string();
    Box::new(move |ctx: LoaderContext| {
        let mut child = if cfg!(target_os = "windows") {
            std::process::Command::new("cmd")
                .args(["/C", &command])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::inherit())
                .spawn()
        } else {
            std::process::Command::new("sh")
                .args(["-c", &command])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::inherit())
                .spawn()
        }
        .map_err(|e| anyhow::anyhow!("failed to spawn loader '{}': {}", command, e))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(ctx.source.as_bytes())
                .map_err(|e| anyhow::anyhow!("failed to write to loader stdin: {}", e))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| anyhow::anyhow!("failed to read loader output: {}", e))?;

        if !output.status.success() {
            anyhow::bail!(
                "loader '{}' failed with exit code: {:?}",
                command,
                output.status.code()
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    })
}

/// Resolve config-based loader rules into direct (pattern, [Loader]) pairs.
/// Filters by build mode and resolves named loaders from the commands map.
pub fn resolve_rules(
    rules: &[(String, Vec<String>, Option<String>)],
    commands: &HashMap<String, String>,
    mode: &BuildMode,
) -> Vec<(String, Vec<Loader>)> {
    let mut resolved: Vec<(String, Vec<Loader>)> = Vec::new();

    for (test, names, rule_mode_str) in rules {
        if let Some(rule_mode) = rule_mode_str
            && BuildMode::from_mode_str(rule_mode) != *mode
        {
            continue;
        }

        let mut loaders: Vec<Loader> = Vec::new();
        for name in names {
            if let Some(cmd) = commands.get(name) {
                loaders.push(command_loader(cmd));
            } else {
                tracing::warn!("loader '{}' not found in commands, skipping", name);
            }
        }

        if !loaders.is_empty() {
            resolved.push((test.clone(), loaders));
        }
    }

    resolved
}
