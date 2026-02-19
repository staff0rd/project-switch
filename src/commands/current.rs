use crate::config::ConfigManager;
use anyhow::Result;
use colored::*;

pub fn execute() -> Result<()> {
    let config_manager = ConfigManager::new()?;

    if let Some(current_project) = config_manager.get_current_project() {
        println!(
            "{}",
            format!("Current project: {}", current_project).green()
        );
    } else {
        println!("{}", "No current project selected".yellow());
    }

    Ok(())
}
