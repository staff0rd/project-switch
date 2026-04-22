//! Deprecated: Use the `list` command instead for interactive command selection.

use crate::config::ConfigManager;
use crate::utils::browser;
use crate::utils::url::is_url;
use anyhow::Result;

#[deprecated(note = "Use the `list` command instead")]
pub fn execute(key: &str) -> Result<()> {
    let config_manager = ConfigManager::new()?;

    // No client configured - check if input is a URL as fallback
    if config_manager.resolve_current_client().is_none() && is_url(key) {
        let url = if key.starts_with("http://") || key.starts_with("https://") {
            key.to_string()
        } else {
            format!("https://{}", key)
        };
        return browser::open_url_in_browser(&url, config_manager.get_default_browser(), false);
    }

    let (current_client_name, client, project) = config_manager.resolve_current()
        .ok_or_else(|| anyhow::anyhow!("No current client selected. Use \"project-switch switch\" to select a client first"))?;

    // Browser inheritance: project > client > global default.
    let scope_browser = project
        .as_ref()
        .and_then(|(_, p)| p.browser.as_deref())
        .or(client.browser.as_deref());

    let command = match config_manager.get_effective_command(key) {
        Some(cmd) => cmd,
        None => {
            // No matching command - check if it's a URL
            if is_url(key) {
                let url = if key.starts_with("http://") || key.starts_with("https://") {
                    key.to_string()
                } else {
                    format!("https://{}", key)
                };
                let browser = scope_browser.unwrap_or_else(|| config_manager.get_default_browser());
                return browser::open_url_in_browser(&url, browser, false);
            }
            let scope = match &project {
                Some((pname, _)) => {
                    format!("project '{}' in client '{}'", pname, current_client_name)
                }
                None => format!("client '{}'", current_client_name),
            };
            anyhow::bail!(
                "Command with key '{}' not found in {} or global commands",
                key,
                scope
            );
        }
    };

    let url = command
        .url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Command '{}' does not have a URL configured", key))?;

    // Browser hierarchy: command > project > client > config > default
    let browser = command
        .browser
        .as_deref()
        .or(scope_browser)
        .unwrap_or_else(|| config_manager.get_default_browser());

    browser::open_command_with_args(url, Some(browser), command.args.as_deref(), false)?;

    Ok(())
}
