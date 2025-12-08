//! Deprecated: Use the `list` command instead for interactive command selection.

use crate::config::ConfigManager;
use crate::utils::browser;
use anyhow::Result;
use colored::*;

/// Check if a string looks like a URL
fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://") ||
    s.starts_with("www.") ||
    // Check for common URL patterns like domain.tld
    (s.contains('.') && !s.contains(' ') && {
        let parts: Vec<&str> = s.split('.').collect();
        parts.len() >= 2 && !parts.last().unwrap_or(&"").is_empty()
    })
}

#[deprecated(note = "Use the `list` command instead")]
pub fn execute(key: &str) -> Result<()> {
    let config_manager = ConfigManager::new()?;

    let current_project_name = match config_manager.get_current_project() {
        Some(name) => name,
        None => {
            // No current project - check if it's a URL
            if is_url(key) {
                let url = if key.starts_with("http://") || key.starts_with("https://") {
                    key.to_string()
                } else {
                    format!("https://{}", key)
                };
                return browser::open_url_in_browser(&url, config_manager.get_default_browser());
            }
            println!("{}", "Error: No current project selected".red());
            println!("{}", "Use \"project-switch switch\" to select a project first".yellow());
            return Ok(());
        }
    };

    let project = config_manager.get_project(current_project_name)
        .ok_or_else(|| anyhow::anyhow!("Current project not found"))?;

    let command = match config_manager.get_project_command(current_project_name, key) {
        Some(cmd) => cmd,
        None => {
            // No matching command - check if it's a URL
            if is_url(key) {
                let url = if key.starts_with("http://") || key.starts_with("https://") {
                    key.to_string()
                } else {
                    format!("https://{}", key)
                };
                let browser = project.browser.as_deref()
                    .unwrap_or_else(|| config_manager.get_default_browser());
                return browser::open_url_in_browser(&url, browser);
            }
            anyhow::bail!("Command with key '{}' not found in project '{}' or global commands", key, current_project_name);
        }
    };

    let url = command.url.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Command '{}' does not have a URL configured", key))?;

    // Browser hierarchy: command > project > config > default
    let browser = command.browser.as_deref()
        .or(project.browser.as_deref())
        .unwrap_or_else(|| config_manager.get_default_browser());

    browser::open_command_with_args(url, browser, command.args.as_deref(), command.url_encode)?;

    Ok(())
}