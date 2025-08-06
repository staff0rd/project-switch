use crate::config::ConfigManager;
use anyhow::Result;
use colored::*;
use inquire::Select;

pub fn execute() -> Result<()> {
    let mut config_manager = ConfigManager::new()?;
    let projects = config_manager.get_projects();
    let current_project = config_manager.get_current_project();

    if projects.is_empty() {
        println!("{}", "No projects found. Use \"add\" command to add a project.".yellow());
        return Ok(());
    }

    let options: Vec<String> = projects
        .iter()
        .map(|project| {
            if Some(&project.name) == current_project {
                format!("▶ {} (current)", project.name).green().to_string()
            } else {
                format!("  {}", project.name)
            }
        })
        .collect();

    let project_names: Vec<&String> = projects.iter().map(|p| &p.name).collect();

    let selected_option = Select::new("Select a project:", options.clone())
        .with_starting_cursor(
            current_project
                .and_then(|current| project_names.iter().position(|&name| name == current))
                .unwrap_or(0),
        )
        .prompt()?;

    let selected_index = options.iter().position(|opt| opt == &selected_option).unwrap();
    let selected_project = project_names[selected_index].clone();

    if Some(&selected_project) != current_project {
        config_manager.set_current_project(&selected_project)?;
        println!("{}", format!("Switched to project: {}", selected_project).green());
    } else {
        println!("{}", format!("Already on project: {}", selected_project).blue());
    }

    Ok(())
}