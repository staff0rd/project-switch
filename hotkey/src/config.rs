use serde_yaml::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".project-switch.yml"))
}

fn load_config_doc() -> Option<(PathBuf, Value)> {
    let path = config_path()?;
    let contents = if path.exists() {
        fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };
    let doc = if contents.is_empty() {
        Value::Mapping(serde_yaml::Mapping::new())
    } else {
        serde_yaml::from_str(&contents).unwrap_or(Value::Mapping(serde_yaml::Mapping::new()))
    };
    Some((path, doc))
}

fn save_config_doc(path: &Path, doc: &Value) {
    if let Ok(yaml) = serde_yaml::to_string(doc) {
        let _ = fs::write(path, yaml);
    }
}

/// Read the `include` path from the config file and resolve `~/` to the home directory.
pub fn read_include_path() -> Option<PathBuf> {
    let path = config_path().filter(|p| p.exists())?;
    let contents = fs::read_to_string(&path).ok()?;
    let doc: Value = serde_yaml::from_str(&contents).ok()?;
    let include = doc.get("include")?.as_str()?;
    let resolved = if let Some(rest) = include.strip_prefix("~/").or_else(|| include.strip_prefix("~\\")) {
        dirs::home_dir()?.join(rest)
    } else {
        PathBuf::from(include)
    };
    Some(resolved)
}

/// Create the config file with minimal defaults if it doesn't exist.
pub fn create_if_missing() {
    let path = match config_path() {
        Some(p) => p,
        None => return,
    };
    if !path.exists() {
        let _ = fs::write(&path, "projects: []\n");
    }
}

/// Read the current value of `shortcuts.enabled` from the config file.
/// Returns `true` if the field is missing or the file doesn't exist (default behaviour).
pub fn read_shortcuts_enabled() -> bool {
    let path = match config_path() {
        Some(p) if p.exists() => p,
        _ => return true,
    };
    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return true,
    };
    let doc: Value = match serde_yaml::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return true,
    };
    doc.get("shortcuts")
        .and_then(|s| s.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

/// Read the selected monitor index (1-based). Defaults to 1.
pub fn read_monitor_index() -> u32 {
    let path = match config_path() {
        Some(p) if p.exists() => p,
        _ => return 1,
    };
    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return 1,
    };
    let doc: Value = match serde_yaml::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return 1,
    };
    doc.get("monitor")
        .and_then(|v| v.as_u64())
        .map(|v| v.max(1) as u32)
        .unwrap_or(1)
}

/// Write the selected monitor index (1-based) to the config file.
pub fn write_monitor_index(index: u32) {
    let (path, mut doc) = match load_config_doc() {
        Some(v) => v,
        None => return,
    };

    if let Value::Mapping(ref mut map) = doc {
        map.insert(
            Value::String("monitor".into()),
            Value::from(index as u64),
        );
    }

    save_config_doc(&path, &doc);
}

/// Toggle `shortcuts.enabled` in the config file. Creates the `shortcuts` section if needed.
/// Returns the new value.
pub fn toggle_shortcuts_enabled() -> bool {
    let (path, mut doc) = match load_config_doc() {
        Some(v) => v,
        None => return true,
    };

    let current = doc
        .get("shortcuts")
        .and_then(|s| s.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let new_value = !current;

    // Ensure shortcuts mapping exists
    if doc.get("shortcuts").is_none() {
        if let Value::Mapping(ref mut map) = doc {
            map.insert(
                Value::String("shortcuts".into()),
                Value::Mapping(serde_yaml::Mapping::new()),
            );
        }
    }

    if let Some(shortcuts) = doc.get_mut("shortcuts") {
        if let Value::Mapping(ref mut map) = shortcuts {
            map.insert(
                Value::String("enabled".into()),
                Value::Bool(new_value),
            );
        }
    }

    save_config_doc(&path, &doc);

    new_value
}
