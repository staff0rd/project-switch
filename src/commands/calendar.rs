use crate::calendar::{self, CalendarCommand};
use crate::config::ConfigManager;
use anyhow::{Context, Result};
use colored::*;

const SETUP_HINT: &str = "Add a Google Cloud Desktop OAuth client to ~/.project-switch.yml as calendar.clientId and calendar.clientSecret";

pub fn execute(command: &CalendarCommand) -> Result<()> {
    match command {
        CalendarCommand::Auth { label } => auth(label),
    }
}

fn auth(label: &str) -> Result<()> {
    let mut config_manager = ConfigManager::new()?;
    let config = config_manager
        .get_calendar_config()
        .cloned()
        .unwrap_or_default();
    let client = calendar::client(&config).context(SETUP_HINT)?;
    let token = calendar::oauth::authorize(&client, label)?;
    config_manager.set_local_calendar_token(label, &token)?;
    println!(
        "{}",
        format!("Saved the Google Calendar login for '{label}' to ~/.project-switch.yml").green()
    );
    Ok(())
}
