use crate::config::ConfigManager;
use anyhow::Result;
use colored::*;
use std::process::Command;

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

    let cmd_result = if cfg!(target_os = "windows") {
        if browser.to_lowercase() == "default" {
            Command::new("cmd")
                .args(&["/C", "start", "", url])
                .status()
        } else {
            Command::new("cmd")
                .args(&["/C", "start", browser, url])
                .status()
        }
    } else if cfg!(target_os = "macos") {
        if browser.to_lowercase() == "default" {
            Command::new("open")
                .arg(url)
                .status()
        } else {
            Command::new("open")
                .args(&["-a", browser, url])
                .status()
        }
    } else {
        // Linux/Unix
        if browser.to_lowercase() == "default" {
            Command::new("xdg-open")
                .arg(url)
                .status()
        } else {
            Command::new(browser)
                .arg(url)
                .status()
        }
    };

    match cmd_result {
        Ok(status) if status.success() => {
            println!("{}", format!("Opening {} in {}...", url, browser).green());
        }
        Ok(_) => {
            anyhow::bail!("Failed to open URL");
        }
        Err(e) => {
            anyhow::bail!("Error opening URL: {}", e);
        }
    }

    Ok(())
}