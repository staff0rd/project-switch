use serde_yaml::Value;
use std::fs;
use std::path::PathBuf;

fn config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".project-switch.yml"))
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

/// Toggle `shortcuts.enabled` in the config file. Creates the `shortcuts` section if needed.
/// Returns the new value.
pub fn toggle_shortcuts_enabled() -> bool {
    let path = match config_path() {
        Some(p) => p,
        None => return true,
    };

    let contents = if path.exists() {
        fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut doc: Value = if contents.is_empty() {
        Value::Mapping(serde_yaml::Mapping::new())
    } else {
        serde_yaml::from_str(&contents).unwrap_or(Value::Mapping(serde_yaml::Mapping::new()))
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

    if let Ok(yaml) = serde_yaml::to_string(&doc) {
        let _ = fs::write(&path, yaml);
    }

    new_value
}
