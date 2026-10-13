use crate::config::BrowserOption;
use anyhow::Result;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn choices_path() -> Result<PathBuf> {
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Unable to determine home directory"))?;
    Ok(home.join(".project-switch-browser-choices.yml"))
}

fn load_from(path: &Path) -> BTreeMap<String, String> {
    fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_yaml::from_str(&contents).ok())
        .unwrap_or_default()
}

fn record_to(path: &Path, command_key: &str, label: &str) -> Result<()> {
    let mut choices = load_from(path);
    choices.insert(command_key.to_lowercase(), label.to_string());
    fs::write(path, serde_yaml::to_string(&choices)?)?;
    Ok(())
}

fn remembered_label(command_key: &str) -> Option<String> {
    let path = choices_path().ok()?;
    load_from(&path).remove(&command_key.to_lowercase())
}

pub fn record(command_key: &str, label: &str) -> Result<()> {
    record_to(&choices_path()?, command_key, label)
}

pub fn ordered(command_key: &str, options: &[BrowserOption]) -> Vec<BrowserOption> {
    order_options(options, remembered_label(command_key).as_deref())
}

fn order_options(options: &[BrowserOption], remembered: Option<&str>) -> Vec<BrowserOption> {
    let (first, rest): (Vec<_>, Vec<_>) = options
        .iter()
        .cloned()
        .partition(|option| Some(option.label.as_str()) == remembered);
    first.into_iter().chain(rest).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Vec<BrowserOption> {
        ["integrator", "integrator+coach", "admin"]
            .into_iter()
            .map(|label| BrowserOption {
                label: label.to_string(),
                value: format!("firefox -P {label}"),
            })
            .collect()
    }

    fn labels(options: Vec<BrowserOption>) -> Vec<String> {
        options.into_iter().map(|option| option.label).collect()
    }

    fn with_temp_file(test: &str, f: impl FnOnce(&Path)) {
        let dir = std::env::temp_dir().join(format!(
            "ps-browser-choice-test-{}-{test}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        f(&dir.join("choices.yml"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remembered_label_is_listed_first_and_rest_keep_config_order() {
        assert_eq!(
            labels(order_options(&options(), Some("admin"))),
            vec!["admin", "integrator", "integrator+coach"]
        );
    }

    #[test]
    fn missing_label_keeps_config_order() {
        assert_eq!(
            labels(order_options(&options(), None)),
            vec!["integrator", "integrator+coach", "admin"]
        );
    }

    #[test]
    fn stale_label_keeps_config_order() {
        assert_eq!(
            labels(order_options(&options(), Some("removed"))),
            vec!["integrator", "integrator+coach", "admin"]
        );
    }

    #[test]
    fn load_returns_empty_when_no_file() {
        with_temp_file("no-file", |path| assert!(load_from(path).is_empty()));
    }

    #[test]
    fn load_returns_empty_when_file_is_corrupt() {
        with_temp_file("corrupt", |path| {
            fs::write(path, "- not\n- a map\n").unwrap();
            assert!(load_from(path).is_empty());
        });
    }

    #[test]
    fn record_keeps_latest_label_per_command() {
        with_temp_file("record", |path| {
            record_to(path, "member-dev", "integrator").unwrap();
            record_to(path, "other", "admin").unwrap();
            record_to(path, "Member-Dev", "integrator+coach").unwrap();
            let choices = load_from(path);
            assert_eq!(choices.len(), 2);
            assert_eq!(choices["member-dev"], "integrator+coach");
            assert_eq!(choices["other"], "admin");
        });
    }
}
