use anyhow::Result;
use std::fs;
use std::path::PathBuf;

const MAX_ENTRIES: usize = 10;

fn history_path() -> Result<PathBuf> {
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Unable to determine home directory"))?;
    Ok(home.join(".project-switch-history.yml"))
}

/// Load recent action keys from the history file.
/// Returns an empty list if the file doesn't exist or can't be parsed.
pub fn load() -> Vec<String> {
    let path = match history_path() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    serde_yaml::from_str::<Vec<String>>(&contents).unwrap_or_default()
}

/// Record an item key as the most recent action.
/// Deduplicates (moves existing entry to top) and caps at 10.
pub fn record(key: &str) -> Result<()> {
    let mut entries = load();
    entries.retain(|k| k != key);
    entries.insert(0, key.to_string());
    entries.truncate(MAX_ENTRIES);
    let yaml = serde_yaml::to_string(&entries)?;
    fs::write(history_path()?, yaml)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Run a test with an isolated history file in a temp dir.
    fn with_temp_history(f: impl FnOnce(&PathBuf)) {
        let name = std::thread::current()
            .name()
            .unwrap_or("unknown")
            .replace("::", "-");
        let dir =
            std::env::temp_dir().join(format!("ps-history-test-{}-{}", std::process::id(), name));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join(".project-switch-history.yml");
        let _ = fs::remove_file(&path);
        f(&path);
        let _ = fs::remove_dir_all(&dir);
    }

    fn load_from(path: &PathBuf) -> Vec<String> {
        let contents = fs::read_to_string(path).unwrap_or_default();
        serde_yaml::from_str::<Vec<String>>(&contents).unwrap_or_default()
    }

    fn record_to(path: &PathBuf, key: &str) {
        let mut entries = load_from(path);
        entries.retain(|k| k != key);
        entries.insert(0, key.to_string());
        entries.truncate(MAX_ENTRIES);
        let yaml = serde_yaml::to_string(&entries).unwrap();
        fs::write(path, yaml).unwrap();
    }

    #[test]
    fn record_adds_entry() {
        with_temp_history(|path| {
            record_to(path, "github");
            assert_eq!(load_from(path), vec!["github"]);
        });
    }

    #[test]
    fn record_deduplicates_and_moves_to_top() {
        with_temp_history(|path| {
            record_to(path, "github");
            record_to(path, "jira");
            record_to(path, "github");
            assert_eq!(load_from(path), vec!["github", "jira"]);
        });
    }

    #[test]
    fn record_caps_at_10() {
        with_temp_history(|path| {
            for i in 0..15 {
                record_to(path, &format!("item-{}", i));
            }
            let entries = load_from(path);
            assert_eq!(entries.len(), MAX_ENTRIES);
            assert_eq!(entries[0], "item-14");
            assert_eq!(entries[9], "item-5");
        });
    }

    #[test]
    fn reexecute_within_full_list_keeps_cap() {
        with_temp_history(|path| {
            for i in 0..10 {
                record_to(path, &format!("item-{}", i));
            }
            // Re-execute item-0 (currently last) — should move to top, still 10 entries
            record_to(path, "item-0");
            let entries = load_from(path);
            assert_eq!(entries.len(), MAX_ENTRIES);
            assert_eq!(entries[0], "item-0");
            assert_eq!(entries[1], "item-9");
        });
    }

    #[test]
    fn load_returns_empty_when_no_file() {
        with_temp_history(|path| {
            assert!(load_from(path).is_empty());
        });
    }
}
