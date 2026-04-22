use crate::config::ConfigManager;
use anyhow::Result;
use colored::*;
use inquire::Select;

pub fn execute() -> Result<()> {
    let mut config_manager = ConfigManager::new()?;
    let current_client = config_manager.get_current_client().cloned();
    let current_project = config_manager.get_current_project().cloned();

    let clients = config_manager.get_clients();
    if clients.is_empty() {
        println!(
            "{}",
            "No clients found. Edit ~/.project-switch.yml to add one.".yellow()
        );
        return Ok(());
    }

    let options: Vec<String> = clients
        .iter()
        .map(|client| {
            if Some(&client.name) == current_client.as_ref() {
                format!("▶ {} (current)", client.name).green().to_string()
            } else {
                format!("  {}", client.name)
            }
        })
        .collect();

    let client_names: Vec<String> = clients.iter().map(|c| c.name.clone()).collect();

    let starting_cursor = current_client
        .as_ref()
        .and_then(|current| client_names.iter().position(|name| name == current))
        .unwrap_or(0);

    let selected_option = Select::new("Select a client:", options.clone())
        .with_starting_cursor(starting_cursor)
        .prompt()?;

    let selected_index = options
        .iter()
        .position(|opt| opt == &selected_option)
        .unwrap();
    let selected_client = client_names[selected_index].clone();

    // If the selected client has nested projects, show a second prompt.
    let nested_project_names: Vec<String> = config_manager
        .get_client(&selected_client)
        .and_then(|c| c.projects.as_ref())
        .map(|projects| projects.iter().map(|p| p.name.clone()).collect())
        .unwrap_or_default();

    let selected_project: Option<String> = if nested_project_names.is_empty() {
        None
    } else {
        let client_entry_label = format!("{} (client)", selected_client);
        let mut sub_options: Vec<String> = Vec::with_capacity(nested_project_names.len() + 1);

        let is_current_client_only = current_client.as_deref() == Some(selected_client.as_str())
            && current_project.is_none();
        sub_options.push(if is_current_client_only {
            format!("▶ {} (current)", client_entry_label)
                .green()
                .to_string()
        } else {
            format!("  {}", client_entry_label)
        });

        for name in &nested_project_names {
            let is_current = current_client.as_deref() == Some(selected_client.as_str())
                && current_project.as_deref() == Some(name.as_str());
            if is_current {
                sub_options.push(format!("▶ {} (current)", name).green().to_string());
            } else {
                sub_options.push(format!("  {}", name));
            }
        }

        let starting_cursor = if is_current_client_only {
            0
        } else if current_client.as_deref() == Some(selected_client.as_str()) {
            current_project
                .as_ref()
                .and_then(|p| nested_project_names.iter().position(|n| n == p))
                .map(|i| i + 1)
                .unwrap_or(0)
        } else {
            0
        };

        let sub_selected = Select::new(
            &format!("Select '{}' or a project:", selected_client),
            sub_options.clone(),
        )
        .with_starting_cursor(starting_cursor)
        .prompt()?;

        let sub_index = sub_options
            .iter()
            .position(|opt| opt == &sub_selected)
            .unwrap();

        if sub_index == 0 {
            None
        } else {
            Some(nested_project_names[sub_index - 1].clone())
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
