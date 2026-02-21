use anyhow::Result;
use colored::*;
use std::process::Command;

pub fn open_command_with_args(
    command: &str,
    browser: &str,
    args: Option<&str>,
    url_encode: bool,
) -> Result<()> {
    // Check if the command is a URL (starts with http)
    if command.starts_with("http") {
        // If there are arguments, append them to the URL with optional encoding
        if let Some(args_str) = args {
            if !args_str.is_empty() {
                let url_with_args = if url_encode {
                    let encoded_args = urlencoding::encode(args_str);
                    format!("{}{}", command, encoded_args)
                } else {
                    format!("{}{}", command, args_str)
                };
                open_url_in_browser(&url_with_args, browser)
            } else {
                open_url_in_browser(command, browser)
            }
        } else {
            open_url_in_browser(command, browser)
        }
    } else {
        // It's a terminal command, run it directly
        run_terminal_command(command, args)
    }
}

fn run_terminal_command(command: &str, args: Option<&str>) -> Result<()> {
    let cmd_result = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("powershell");
        cmd.args(["-Command", command]);

        if let Some(args_str) = args {
            if !args_str.is_empty() {
                cmd.arg(args_str);
            }
        }

        cmd.spawn()
    } else {
        let mut full_command = command.to_string();

        if let Some(args_str) = args {
            if !args_str.is_empty() {
                full_command.push(' ');
                full_command.push_str(args_str);
            }
        }

        Command::new("sh").args(["-c", &full_command]).spawn()
    };

    match cmd_result {
        Ok(_) => {
            let args_str = args
                .filter(|s| !s.is_empty())
                .map(|a| format!(" {}", a))
                .unwrap_or_default();
            println!(
                "{}",
                format!("Running command: {}{}", command, args_str).green()
            );
            Ok(())
        }
        Err(e) => {
            anyhow::bail!("Error running command: {}", e);
        }
    }
}

pub fn launch_shortcut(path: &str) -> Result<()> {
    let status = if cfg!(target_os = "windows") {
        Command::new("powershell")
            .args(["-Command", &format!("Start-Process '{}'", path)])
            .status()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(path).status()
    } else {
        anyhow::bail!("Shortcut launching is not supported on this platform")
    };

    match status {
        Ok(s) if s.success() => {
            println!("{}", format!("Launching {}...", path).green());
            Ok(())
        }
        Ok(_) => anyhow::bail!("Failed to launch shortcut: {}", path),
        Err(e) => anyhow::bail!("Error launching shortcut: {}", e),
    }
}

/// Parse a browser string into the executable name and any extra arguments.
/// e.g., "firefox -P someProfile" -> ("firefox", ["P", "someProfile"])
fn parse_browser_with_args(browser: &str) -> (&str, Vec<&str>) {
    let parts: Vec<&str> = browser.split_whitespace().collect();
    if parts.len() > 1 {
        (parts[0], parts[1..].to_vec())
    } else {
        (browser, vec![])
    }
}

pub fn open_url_in_browser(url: &str, browser: &str) -> Result<()> {
    let cmd_result = if cfg!(target_os = "windows") {
        if browser.to_lowercase() == "default" {
            Command::new("powershell")
                .args([
                    "-Command",
                    &format!("Set-Location C:\\; Start-Process '{}'", url),
                ])
                .status()
        } else {
            let (browser_cmd, extra_args) = parse_browser_with_args(browser);
            let ps_command = if extra_args.is_empty() {
                format!(
                    "Set-Location C:\\; Start-Process '{}' '{}'",
                    browser_cmd, url
                )
            } else {
                format!(
                    "Set-Location C:\\; Start-Process '{}' '{} {}'",
                    browser_cmd,
                    extra_args.join(" "),
                    url
                )
            };
            Command::new("powershell")
                .args(["-Command", &ps_command])
                .status()
        }
    } else if cfg!(target_os = "macos") {
        if browser.to_lowercase() == "default" {
            Command::new("open").arg(url).status()
        } else {
            let (browser_cmd, extra_args) = parse_browser_with_args(browser);
            let mut cmd = Command::new("open");
            cmd.args(["-a", browser_cmd]);
            if !extra_args.is_empty() {
                cmd.arg("--args");
                for arg in extra_args {
                    cmd.arg(arg);
                }
            }
            cmd.arg(url).status()
        }
    } else {
        // Linux/Unix
        if browser.to_lowercase() == "default" {
            Command::new("xdg-open").arg(url).status()
        } else {
            let (browser_cmd, extra_args) = parse_browser_with_args(browser);
            let mut cmd = Command::new(browser_cmd);
            for arg in extra_args {
                cmd.arg(arg);
            }
            cmd.arg(url).status()
        }
    };

    match cmd_result {
        Ok(status) if status.success() => {
            println!("{}", format!("Opening {} in {}...", url, browser).green());
        }
        Ok(_) => {
            anyhow::bail!("Failed to open URL");
        }
        Err(e) => {
            anyhow::bail!("Error opening URL: {}", e);
        }
    }

    Ok(())
}
