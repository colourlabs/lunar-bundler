use std::path::PathBuf;

pub fn discover_paths(lua_version: &str) -> Vec<PathBuf> {
    let version = normalize_version(lua_version);
    let mut paths = Vec::new();

    if let Some(lr_paths) = query_luarocks_bin() {
        paths.extend(lr_paths);
    }

    paths.extend(default_paths(&version));

    if let Some(home_paths) = home_paths(&version) {
        paths.extend(home_paths);
    }

    paths.dedup();
    paths.into_iter().filter(|p| p.exists()).collect()
}

fn normalize_version(lua_version: &str) -> String {
    match lua_version {
        "51" => "5.1".to_string(),
        "52" => "5.2".to_string(),
        "53" => "5.3".to_string(),
        "54" => "5.4".to_string(),
        "55" => "5.5".to_string(),
        v => v.to_string(),
    }
}

fn query_luarocks_bin() -> Option<Vec<PathBuf>> {
    let bin = if cfg!(target_os = "windows") {
        "luarocks.bat"
    } else {
        "luarocks"
    };

    let output = std::process::Command::new(bin)
        .args(["path", "--lr-path"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let lr_path = String::from_utf8_lossy(&output.stdout);

    // luarocks uses ; on all platforms for path separators in --lr-path
    let paths = lr_path
        .split(';')
        .map(|part| {
            let base = part
                .trim()
                .trim_end_matches("?/init.lua")
                .trim_end_matches("?.lua")
                .trim_end_matches(['/', '\\']);
            PathBuf::from(base)
        })
        .filter(|p| !p.as_os_str().is_empty())
        .collect();

    Some(paths)
}

fn default_paths(version: &str) -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut paths = vec![
            PathBuf::from(format!("C:/Program Files/Lua/{}/lua", version)),
            PathBuf::from(format!("C:/LuaRocks/share/lua/{}", version)),
            PathBuf::from(format!("C:/LuaRocks/lib/lua/{}", version)),
        ];

        if let Ok(appdata) = std::env::var("APPDATA") {
            paths.push(PathBuf::from(&appdata).join(format!("LuaRocks/share/lua/{}", version)));
            paths.push(PathBuf::from(&appdata).join(format!("LuaRocks/lib/lua/{}", version)));
        }

        paths
    }

    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from(format!("/usr/local/share/lua/{}", version)),
            PathBuf::from(format!("/usr/local/lib/lua/{}", version)),
            // homebrew paths
            PathBuf::from(format!("/opt/homebrew/share/lua/{}", version)),
            PathBuf::from(format!("/opt/homebrew/lib/lua/{}", version)),
            // macports
            PathBuf::from(format!("/opt/local/share/lua/{}", version)),
        ]
    }

    #[cfg(target_os = "linux")]
    {
        vec![
            PathBuf::from(format!("/usr/local/share/lua/{}", version)),
            PathBuf::from(format!("/usr/local/lib/lua/{}", version)),
            PathBuf::from(format!("/usr/share/lua/{}", version)),
            PathBuf::from(format!("/usr/lib/lua/{}", version)),
        ]
    }

    // fallback for any other platform
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        vec![
            PathBuf::from(format!("/usr/local/share/lua/{}", version)),
            PathBuf::from(format!("/usr/local/lib/lua/{}", version)),
        ]
    }
}

fn home_paths(version: &str) -> Option<Vec<PathBuf>> {
    let home = dirs::home_dir()?;

    #[cfg(target_os = "windows")]
    {
        Some(vec![
            home.join(format!("AppData/Roaming/LuaRocks/share/lua/{}", version)),
            home.join(format!("AppData/Roaming/LuaRocks/lib/lua/{}", version)),
        ])
    }

    #[cfg(not(target_os = "windows"))]
    {
        Some(vec![
            home.join(format!(".luarocks/share/lua/{}", version)),
            home.join(format!(".luarocks/lib/lua/{}", version)),
        ])
    }
}

pub fn is_native_module(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("so") | Some("dll") | Some("dylib")
    )
}
