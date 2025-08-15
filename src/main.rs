mod config;
mod commands;
mod utils;

use clap::{Parser, Subcommand};
use anyhow::Result;

#[derive(Parser)]
#[command(name = "project-switch")]
#[command(about = "CLI tool to manage and switch between projects")]
#[command(version = "1.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Switch between projects
    Switch,
    /// Add a new project
    Add {
        /// Project name
        name: Option<String>,
    },
    /// Show the current project
    Current,
    /// Open a URL associated with the current project
    Open {
        /// Command key
        key: String,
    },
    /// List all openable items from the current project (interactive)
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Switch => {
            commands::switch::execute()?;
        }
        Commands::Add { name } => {
            commands::add::execute(name)?;
        }
        Commands::Current => {
            commands::current::execute()?;
        }
        Commands::Open { key } => {
            commands::open::execute(&key)?;
        }
        Commands::List => {
            commands::list::execute()?;
        }
    }

    Ok(())
}