use crate::config::ConfigManager;
use crate::utils::browser;
use anyhow::Result;
use colored::*;
use inquire::Select;

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

    // Collect commands from both project and global
    let mut all_commands = Vec::new();
    
    // Add project-specific commands
    if let Some(project_commands) = &project.commands {
        all_commands.extend(project_commands.iter().cloned());
    }
    
    // Add global commands
    if let Some(global_commands) = config_manager.get_global_commands() {
        all_commands.extend(global_commands.iter().cloned());
    }

    if all_commands.is_empty() {
        println!("{}", format!("No openable items found in project '{}' or global commands", current_project_name).yellow());
        println!("{}", "Use \"project-switch add\" to add commands to your project".blue());
        return Ok(());
    }

    let mut sorted_commands = all_commands;
    sorted_commands.sort_by(|a, b| a.key.cmp(&b.key));
    // Remove duplicates, keeping project-specific commands over global ones
    sorted_commands.dedup_by(|a, b| a.key == b.key);

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

    browser::open_command_with_args(url, browser, selected_command.args.as_deref())?;

    Ok(())
}