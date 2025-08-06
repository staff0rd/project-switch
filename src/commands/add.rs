use crate::config::{ConfigManager, Project};
use anyhow::Result;
use colored::*;
use inquire::Text;

pub fn execute(name: Option<String>) -> Result<()> {
    let mut config_manager = ConfigManager::new()?;

    let project_name = if let Some(name) = name {
        let trimmed_name = name.trim().to_string();
        if config_manager.project_exists(&trimmed_name) {
            anyhow::bail!("Project '{}' already exists", trimmed_name);
        }
        trimmed_name
    } else {
        Text::new("Enter project name:")
            .with_validator(|input: &str| {
                let trimmed = input.trim();
                if trimmed.is_empty() {
                    Ok(inquire::validator::Validation::Invalid(
                        "Project name cannot be empty".into(),
                    ))
                } else if ConfigManager::new()
                    .map(|cm| cm.project_exists(trimmed))
                    .unwrap_or(false)
                {
                    Ok(inquire::validator::Validation::Invalid(
                        format!("Project '{}' already exists", trimmed).into(),
                    ))
                } else {
                    Ok(inquire::validator::Validation::Valid)
                }
            })
            .prompt()?
            .trim()
            .to_string()
    };

    let project = Project {
        name: project_name.clone(),
        path: None,
        description: None,
        browser: None,
        commands: None,
    };

    config_manager.add_project(project)?;
    println!("{}", format!("Project '{}' added successfully!", project_name).green());

    let projects = config_manager.get_projects();
    if projects.len() == 1 {
        println!("{}", format!("'{}' is now the current project.", project_name).blue());
    }

    Ok(())
}