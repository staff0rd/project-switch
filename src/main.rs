mod commands;
mod config;
mod hotkey;
mod launcher;
mod ui;
mod utils;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "project-switch")]
#[command(about = "CLI tool to manage and switch between projects")]
#[command(version = "1.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
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
    #[command(hide = true)]
    /// (Deprecated) Open a URL associated with the current project - use 'list' instead
    Open {
        /// Command key
        key: String,
    },
    /// List all openable items from the current project (interactive)
    List {
        /// Print the full command and args before executing
        #[arg(long)]
        debug: bool,
        /// Launch the windowed GUI launcher instead of the terminal UI
        #[arg(long)]
        gui: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        // No subcommand: start the daemon (hotkey + tray + GUI)
        None => hotkey::daemon::run(),
        Some(Commands::Switch) => commands::switch::execute(),
        Some(Commands::Add { name }) => commands::add::execute(name),
        Some(Commands::Current) => commands::current::execute(),
        #[allow(deprecated)]
        Some(Commands::Open { key }) => commands::open::execute(&key),
        Some(Commands::List { debug, gui }) => {
            if gui {
                commands::list::execute_gui()
            } else {
                commands::list::execute(debug)
            }
        }
    };

    if let Err(e) = result {
        eprintln!("\nError: {e:#}");
        eprint!("\nPress Enter to exit...");
        let _ = std::io::stdin().read_line(&mut String::new());
        std::process::exit(1);
    }
}
