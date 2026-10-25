use crate::config::{ConfigManager, PaprikaConfig};
use crate::paprika::{self, GroceryCommand};
use anyhow::{Context, Result};
use colored::*;
use inquire::InquireError;

fn paprika_config(config_manager: &ConfigManager) -> Result<&PaprikaConfig> {
    config_manager
        .get_paprika_config()
        .context("Paprika is not configured: add a `paprika:` section to ~/.project-switch.yml")
}

pub fn execute(command: &GroceryCommand) -> Result<()> {
    let mut config_manager = ConfigManager::new()?;

    if *command == GroceryCommand::Auth {
        let config = config_manager
            .get_paprika_config()
            .cloned()
            .unwrap_or_default();
        let token = paprika::authenticate(&config)?;
        config_manager.set_local_paprika_token(&token)?;
        println!("{}", "Saved Paprika token to ~/.project-switch.yml".green());
        return Ok(());
    }

    print!(
        "{}",
        paprika::run(paprika_config(&config_manager)?, command)?
    );
    Ok(())
}

pub fn launcher_add(args: Option<&str>) -> Option<GroceryCommand> {
    let item = args.map(str::trim).filter(|item| !item.is_empty())?;
    Some(GroceryCommand::Add {
        item: item.to_string(),
        list: None,
    })
}

pub fn run_with_loaded_config(command: &GroceryCommand) -> Result<String> {
    let config_manager = ConfigManager::new()?;
    paprika::run(paprika_config(&config_manager)?, command)
}

pub fn apply_and_list(command: Option<&GroceryCommand>) -> Result<Vec<String>> {
    let config_manager = ConfigManager::new()?;
    let config = paprika_config(&config_manager)?;
    if let Some(command) = command {
        paprika::run(config, command)?;
    }
    paprika::default_list_items(config)
}

pub fn execute_from_launcher(config_manager: &ConfigManager, args: Option<&str>) -> Result<()> {
    let config = paprika_config(config_manager)?;
    if let Some(command) = launcher_add(args) {
        print!("{}", paprika::run(config, &command)?);
        return Ok(());
    }

    let items = paprika::default_list_items(config)?;
    if items.is_empty() {
        println!("{}", "The default grocery list is empty".yellow());
        return Ok(());
    }
    let item = match inquire::Select::new("Remove from the default grocery list:", items).prompt() {
        Ok(item) => item,
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let remove = GroceryCommand::Remove { item, list: None };
    print!("{}", paprika::run(config, &remove)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_args_add_to_default_list() {
        assert_eq!(
            launcher_add(Some("oat milk ")),
            Some(GroceryCommand::Add {
                item: "oat milk".to_string(),
                list: None,
            })
        );
    }

    #[test]
    fn launcher_without_args_shows_list() {
        assert_eq!(launcher_add(None), None);
        assert_eq!(launcher_add(Some("  ")), None);
    }
}
