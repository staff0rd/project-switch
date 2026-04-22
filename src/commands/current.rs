use crate::config::ConfigManager;
use anyhow::Result;
use colored::*;

pub fn execute() -> Result<()> {
    let config_manager = ConfigManager::new()?;

    match config_manager.resolve_current() {
        Some((client_name, _, Some((project_name, _)))) => {
            println!(
                "{}",
                format!("Current: {} / {}", client_name, project_name).green()
            );
        }
        Some((client_name, _, None)) => {
            println!("{}", format!("Current client: {}", client_name).green());
        }
        None => {
            if let Some(current_client) = config_manager.get_current_client() {
                println!(
                    "{}",
                    format!(
                        "Current client '{}' is set but not found in config",
                        current_client
                    )
                    .yellow()
                );
            } else {
                println!("{}", "No current client selected".yellow());
            }
        }
    }

    Ok(())
}
