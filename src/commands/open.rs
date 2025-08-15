use crate::config::ConfigManager;
use crate::utils::browser;
use anyhow::Result;
use colored::*;

pub fn execute(key: &str) -> Result<()> {
    let config_manager = ConfigManager::new()?;
    
    let current_project_name = match config_manager.get_current_project() {
        Some(name) => name,
        None => {
            println!("{}", "Error: No current project selected".red());
            println!("{}", "Use \"project-switch switch\" to select a project first".yellow());
            return Ok(());
        }
    };

    let project = config_manager.get_project(current_project_name)
        .ok_or_else(|| anyhow::anyhow!("Current project not found"))?;

    let command = config_manager.get_project_command(current_project_name, key)
        .ok_or_else(|| anyhow::anyhow!("Command with key '{}' not found in project '{}'", key, current_project_name))?;

    let url = command.url.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Command '{}' does not have a URL configured", key))?;

    // Browser hierarchy: command > project > config > default
    let browser = command.browser.as_deref()
        .or(project.browser.as_deref())
        .unwrap_or_else(|| config_manager.get_default_browser());

    browser::open_url_in_browser(url, browser)?;

    Ok(())
}