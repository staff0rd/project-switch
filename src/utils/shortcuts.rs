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

#[cfg(target_os = "windows")]
pub fn collect_shortcuts(
    extra_paths: &[String],
    exclude_patterns: &[String],
) -> Vec<ShortcutEntry> {
    use std::collections::HashSet;

    let mut seen: HashSet<String> = HashSet::new();
    let mut entries: Vec<ShortcutEntry> = Vec::new();

    // Build scan locations in priority order
    let mut scan_dirs: Vec<(PathBuf, bool)> = Vec::new(); // (path, recursive)

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

    for (dir, recursive) in &scan_dirs {
        scan_directory_windows(dir, *recursive, exclude_patterns, &mut seen, &mut entries);
    }

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries
}

#[cfg(target_os = "windows")]
fn scan_directory_windows(
    dir: &PathBuf,
    recursive: bool,
    exclude_patterns: &[String],
    seen: &mut std::collections::HashSet<String>,
    entries: &mut Vec<ShortcutEntry>,
) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();

        if path.is_dir() && recursive {
            scan_directory_windows(&path, true, exclude_patterns, seen, entries);
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext != "lnk" && ext != "url" {
            continue;
        }

        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        // Dedup by lowercase stem
        let key = stem.to_lowercase();
        if seen.contains(&key) {
            continue;
        }

        // Check exclude patterns
        if matches_any_pattern(&stem, exclude_patterns) {
            continue;
        }

        seen.insert(key);
        entries.push(ShortcutEntry {
            name: stem,
            path: path.clone(),
        });
    }
}

#[cfg(target_os = "macos")]
pub fn collect_shortcuts(
    extra_paths: &[String],
    exclude_patterns: &[String],
) -> Vec<ShortcutEntry> {
    use std::collections::HashSet;

    let mut seen: HashSet<String> = HashSet::new();
    let mut entries: Vec<ShortcutEntry> = Vec::new();

    // Build scan locations in priority order
    let mut scan_dirs: Vec<(PathBuf, bool)> = Vec::new(); // (path, recursive)

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

    for (dir, recursive) in &scan_dirs {
        scan_directory_macos(dir, *recursive, exclude_patterns, &mut seen, &mut entries);
    }

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries
}

#[cfg(target_os = "macos")]
fn scan_directory_macos(
    dir: &PathBuf,
    recursive: bool,
    exclude_patterns: &[String],
    seen: &mut std::collections::HashSet<String>,
    entries: &mut Vec<ShortcutEntry>,
) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // .app bundles are directories with .app extension
        if ext == "app" && path.is_dir() {
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };

            let key = stem.to_lowercase();
            if seen.contains(&key) {
                continue;
            }

            if matches_any_pattern(&stem, exclude_patterns) {
                continue;
            }

            seen.insert(key);
            entries.push(ShortcutEntry {
                name: stem,
                path: path.clone(),
            });
            continue;
        }

        // Recurse into non-.app directories when recursive
        if path.is_dir() && recursive {
            scan_directory_macos(&path, true, exclude_patterns, seen, entries);
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn collect_shortcuts(
    _extra_paths: &[String],
    _exclude_patterns: &[String],
) -> Vec<ShortcutEntry> {
    Vec::new()
}
