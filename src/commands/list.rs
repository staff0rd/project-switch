use crate::config::ConfigManager;
use anyhow::Result;
use colored::*;
use inquire::Select;
use std::process::Command;

pub fn execute() -> Result<()> {
    println!("DEBUG: List command starting");
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

    let commands = match &project.commands {
        Some(commands) if !commands.is_empty() => commands,
        _ => {
            println!("{}", format!("No openable items found in project '{}'", current_project_name).yellow());
            println!("{}", "Use \"project-switch add\" to add commands to your project".blue());
            return Ok(());
        }
    };

    let mut sorted_commands = commands.clone();
    sorted_commands.sort_by(|a, b| a.key.cmp(&b.key));

    let options: Vec<String> = sorted_commands
        .iter()
        .map(|cmd| {
            let display_text = if let Some(url) = &cmd.url {
                let truncated_url = if url.len() > 60 {
                    format!("{}...", &url[..57])
                } else {
                    url.clone()
                };
                format!("{} → {}", cmd.key.green().bold(), truncated_url.bright_blue())
            } else {
                cmd.key.green().bold().to_string()
            };
            display_text
        })
        .collect();

    let selected_option = Select::new(
        &format!("Select an item to open from '{}':", current_project_name),
        options.clone()
    ).prompt()?;

    let selected_index = options.iter().position(|opt| opt == &selected_option).unwrap();
    let selected_command = &sorted_commands[selected_index];

    let url = selected_command.url.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Command '{}' does not have a URL configured", selected_command.key))?;

    // Browser hierarchy: command > project > config > default
    let browser = selected_command.browser.as_deref()
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