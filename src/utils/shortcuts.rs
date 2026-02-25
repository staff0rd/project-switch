use std::path::PathBuf;

#[derive(Clone)]
pub struct ShortcutEntry {
    pub name: String,  // filename stem (e.g. "Visual Studio Code")
    pub path: PathBuf, // full path to the .lnk/.url/.app file
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn matches_any_pattern(name: &str, patterns: &[String]) -> bool {
    let name_lower = name.to_lowercase();
    for pattern in patterns {
        let pat = pattern.to_lowercase();
        if pat.starts_with('*') && pat.ends_with('*') && pat.len() > 2 {
            // *something* — contains
            if name_lower.contains(&pat[1..pat.len() - 1]) {
                return true;
            }
        } else if let Some(suffix) = pat.strip_prefix('*') {
            // *suffix — ends with
            if name_lower.ends_with(suffix) {
                return true;
            }
        } else if let Some(prefix) = pat.strip_suffix('*') {
            // prefix* — starts with
            if name_lower.starts_with(prefix) {
                return true;
            }
        } else {
            // Exact match
            if name_lower == pat {
                return true;
            }
        }
    }
    false
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn try_add_entry(
    path: &std::path::Path,
    exclude_patterns: &[String],
    seen: &mut std::collections::HashSet<String>,
    entries: &mut Vec<ShortcutEntry>,
) {
    let stem = match path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s.to_string(),
        None => return,
    };
    let key = stem.to_lowercase();
    if seen.contains(&key) || matches_any_pattern(&stem, exclude_patterns) {
        return;
    }
    seen.insert(key);
    entries.push(ShortcutEntry {
        name: stem,
        path: path.to_path_buf(),
    });
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn scan_directory(
    dir: &PathBuf,
    recursive: bool,
    exclude_patterns: &[String],
    seen: &mut std::collections::HashSet<String>,
    entries: &mut Vec<ShortcutEntry>,
    is_shortcut: fn(&std::path::Path) -> bool,
    is_recursible: fn(&std::path::Path) -> bool,
) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();

        if is_shortcut(&path) {
            try_add_entry(&path, exclude_patterns, seen, entries);
        } else if recursive && is_recursible(&path) {
            scan_directory(
                &path,
                true,
                exclude_patterns,
                seen,
                entries,
                is_shortcut,
                is_recursible,
            );
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn collect_from_dirs(
    scan_dirs: &[(PathBuf, bool)],
    exclude_patterns: &[String],
    is_shortcut: fn(&std::path::Path) -> bool,
    is_recursible: fn(&std::path::Path) -> bool,
) -> Vec<ShortcutEntry> {
    let mut seen = std::collections::HashSet::new();
    let mut entries = Vec::new();
    for (dir, recursive) in scan_dirs {
        scan_directory(
            dir,
            *recursive,
            exclude_patterns,
            &mut seen,
            &mut entries,
            is_shortcut,
            is_recursible,
        );
    }
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn ext_lowercase(path: &std::path::Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

#[cfg(target_os = "windows")]
pub fn collect_shortcuts(
    extra_paths: &[String],
    exclude_patterns: &[String],
) -> Vec<ShortcutEntry> {
    let mut scan_dirs: Vec<(PathBuf, bool)> = Vec::new();

    // 1. User Desktop (non-recursive)
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        scan_dirs.push((PathBuf::from(profile).join("Desktop"), false));
    }

    // 2. Public Desktop (non-recursive)
    scan_dirs.push((PathBuf::from(r"C:\Users\Public\Desktop"), false));

    // 3. User Start Menu (recursive)
    if let Some(appdata) = std::env::var_os("APPDATA") {
        scan_dirs.push((
            PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs"),
            true,
        ));
    }

    // 4. All Users Start Menu (recursive)
    if let Some(all_users) = std::env::var_os("ALLUSERSPROFILE") {
        scan_dirs.push((
            PathBuf::from(all_users).join(r"Microsoft\Windows\Start Menu\Programs"),
            true,
        ));
    }

    // 5. Extra paths from config (recursive)
    for extra in extra_paths {
        scan_dirs.push((PathBuf::from(extra), true));
    }

    fn is_shortcut(path: &std::path::Path) -> bool {
        let ext = ext_lowercase(path);
        !path.is_dir() && (ext == "lnk" || ext == "url")
    }
    fn is_recursible(path: &std::path::Path) -> bool {
        path.is_dir()
    }

    collect_from_dirs(&scan_dirs, exclude_patterns, is_shortcut, is_recursible)
}

#[cfg(target_os = "macos")]
pub fn collect_shortcuts(
    extra_paths: &[String],
    exclude_patterns: &[String],
) -> Vec<ShortcutEntry> {
    let mut scan_dirs: Vec<(PathBuf, bool)> = Vec::new();

    // 1. /Applications (non-recursive)
    scan_dirs.push((PathBuf::from("/Applications"), false));

    // 2. /Applications/Utilities (non-recursive)
    scan_dirs.push((PathBuf::from("/Applications/Utilities"), false));

    // 3. ~/Applications (non-recursive — Homebrew Cask, user apps)
    if let Some(home) = dirs::home_dir() {
        scan_dirs.push((home.join("Applications"), false));
    }

    // 4. Extra paths from config (recursive)
    for extra in extra_paths {
        scan_dirs.push((PathBuf::from(extra), true));
    }

    fn is_shortcut(path: &std::path::Path) -> bool {
        ext_lowercase(path) == "app" && path.is_dir()
    }
    fn is_recursible(path: &std::path::Path) -> bool {
        path.is_dir() && ext_lowercase(path) != "app"
    }

    collect_from_dirs(&scan_dirs, exclude_patterns, is_shortcut, is_recursible)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn collect_shortcuts(
    _extra_paths: &[String],
    _exclude_patterns: &[String],
) -> Vec<ShortcutEntry> {
    Vec::new()
}
