mod commands;
mod config;
mod history;
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
        /// Monitor number to display the GUI window on (1-based)
        #[arg(long)]
        monitor: Option<u32>,
    },
}

fn main() {
    let cli = Cli::parse();
    let gui_mode = matches!(&cli.command, Some(Commands::List { gui: true, .. }));

    let result = match cli.command {
        // No subcommand: start the daemon (hotkey + tray + GUI)
        None => hotkey::daemon::run(),
        Some(Commands::Switch) => commands::switch::execute(),
        Some(Commands::Add { name }) => commands::add::execute(name),
        Some(Commands::Current) => commands::current::execute(),
        #[allow(deprecated)]
        Some(Commands::Open { key }) => commands::open::execute(&key),
        Some(Commands::List {
            debug,
            gui,
            monitor,
        }) => {
            if gui {
                commands::list::execute_gui(monitor)
            } else {
                commands::list::execute(debug)
            }
        }
    };

    if let Err(e) = result {
        let msg = format!("{e:#}");
        utils::log::append_error(&msg);

        if gui_mode {
            show_error_dialog(&msg);
        } else {
            eprintln!("\nError: {msg}");
            eprint!("\nPress Enter to exit...");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn show_error_dialog(msg: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, IDYES, MB_ICONERROR, MB_YESNO};

    let title = HSTRING::from("project-switch");
    let text = HSTRING::from(format!("Error: {msg}\n\nOpen config file in editor?"));
    unsafe {
        let result = MessageBoxW(None, &text, &title, MB_ICONERROR | MB_YESNO);
        if result == IDYES {
            if let Some(path) = dirs::home_dir().map(|h| h.join(".project-switch.yml")) {
                let _ = std::process::Command::new("cmd")
                    .args(["/c", "code"])
                    .arg(path)
                    .spawn();
            }
        }
    }
}

#[cfg(not(windows))]
fn show_error_dialog(msg: &str) {
    eprintln!("\nError: {msg}");
}
