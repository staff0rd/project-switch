use crate::config::ConfigManager;
use crate::launcher::picker::{PickerClient, PickerEntry, PickerOutcome, PickerState};
use anyhow::Result;
use colored::*;
use inquire::Select;

/// Build the picker model from the current config. Used by the GUI launcher,
/// which shows the same choices without an interactive terminal prompt.
pub fn picker_state() -> Result<PickerState> {
    Ok(build_picker(&ConfigManager::new()?))
}

fn build_picker(config_manager: &ConfigManager) -> PickerState {
    let clients = config_manager
        .get_clients()
        .iter()
        .map(|client| PickerClient {
            name: client.name.clone(),
            projects: client
                .projects
                .as_ref()
                .map(|projects| projects.iter().map(|p| p.name.clone()).collect())
                .unwrap_or_default(),
        })
        .collect();

    PickerState::new(
        clients,
        config_manager.get_current_client().cloned(),
        config_manager.get_current_project().cloned(),
    )
}

/// Persist a picker choice, leaving the config untouched when it already
/// matches the active selection.
pub fn apply_selection(client: &str, project: Option<&str>) -> Result<()> {
    let mut config_manager = ConfigManager::new()?;
    if config_manager.get_current_client().map(String::as_str) == Some(client)
        && config_manager.get_current_project().map(String::as_str) == project
    {
        return Ok(());
    }
    config_manager.set_current_selection(client, project)
}

fn format_option(entry: &PickerEntry) -> String {
    if entry.is_current {
        format!("▶ {} (current)", entry.label).green().to_string()
    } else {
        format!("  {}", entry.label)
    }
}

pub fn execute() -> Result<()> {
    let mut config_manager = ConfigManager::new()?;
    let current_client = config_manager.get_current_client().cloned();
    let current_project = config_manager.get_current_project().cloned();

    let mut picker = build_picker(&config_manager);
    if picker.is_empty() {
        println!(
            "{}",
            "No clients found. Edit ~/.project-switch.yml to add one.".yellow()
        );
        return Ok(());
    }

    let (selected_client, selected_project) = loop {
        let entries = picker.entries("");
        let options: Vec<String> = entries.iter().map(format_option).collect();

        let selected_option = Select::new(&format!("{}:", picker.title()), options.clone())
            .with_starting_cursor(picker.cursor(""))
            .prompt()?;

        let selected_index = options
            .iter()
            .position(|opt| opt == &selected_option)
            .unwrap();

        match picker.activate(&entries[selected_index]) {
            PickerOutcome::Descended => continue,
            PickerOutcome::Chosen { client, project } => break (client, project),
        }
    };

    let is_same_selection = current_client.as_deref() == Some(selected_client.as_str())
        && current_project.as_deref() == selected_project.as_deref();

    if is_same_selection {
        match &selected_project {
            Some(p) => println!(
                "{}",
                format!("Already on project: {} / {}", selected_client, p).blue()
            ),
            None => println!(
                "{}",
                format!("Already on client: {}", selected_client).blue()
            ),
        }
    } else {
        config_manager.set_current_selection(&selected_client, selected_project.as_deref())?;
        match &selected_project {
            Some(p) => println!(
                "{}",
                format!("Switched to project: {} / {}", selected_client, p).green()
            ),
            None => println!(
                "{}",
                format!("Switched to client: {}", selected_client).green()
            ),
        }
    }

    Ok(())
}
