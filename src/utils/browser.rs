use anyhow::Result;
use colored::*;
use std::process::Command;

pub fn open_command_with_args(command: &str, browser: &str, args: Option<&str>) -> Result<()> {
    // Check if the command is a URL (starts with http)
    if command.starts_with("http") {
        // If there are arguments, append them to the URL with proper encoding
        if let Some(args_str) = args {
            if !args_str.is_empty() {
                let encoded_args = urlencoding::encode(args_str);
                let url_with_args = format!("{}{}", command, encoded_args);
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
        cmd.args(&["-Command", command]);
        
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
        
        Command::new("sh")
            .args(&["-c", &full_command])
            .spawn()
    };

    match cmd_result {
        Ok(_) => {
            let args_str = args
                .filter(|s| !s.is_empty())
                .map(|a| format!(" {}", a))
                .unwrap_or_default();
            println!("{}", format!("Running command: {}{}", command, args_str).green());
            Ok(())
        }
        Err(e) => {
            anyhow::bail!("Error running command: {}", e);
        }
    }
}

pub fn open_url_in_browser(url: &str, browser: &str) -> Result<()> {
    let cmd_result = if cfg!(target_os = "windows") {
        if browser.to_lowercase() == "default" {
            Command::new("powershell")
                .args(&["-Command", &format!("Set-Location C:\\; Start-Process '{}'", url)])
                .status()
        } else {
            // Parse browser string to handle command + args (e.g., "firefox -P someProfile")
            let parts: Vec<&str> = browser.split_whitespace().collect();
            let (browser_cmd, browser_args) = if parts.len() > 1 {
                (parts[0], parts[1..].join(" "))
            } else {
                (browser, String::new())
            };
            
            let ps_command = if browser_args.is_empty() {
                format!("Set-Location C:\\; Start-Process '{}' '{}'", browser_cmd, url)
            } else {
                format!("Set-Location C:\\; Start-Process '{}' '{} {}'", browser_cmd, browser_args, url)
            };
            
            Command::new("powershell")
                .args(&["-Command", &ps_command])
                .status()
        }
    } else if cfg!(target_os = "macos") {
        if browser.to_lowercase() == "default" {
            Command::new("open")
                .arg(url)
                .status()
        } else {
            // Parse browser string to handle command + args (e.g., "firefox -P someProfile")
            let parts: Vec<&str> = browser.split_whitespace().collect();
            if parts.len() > 1 {
                // Browser has arguments
                let browser_cmd = parts[0];
                let mut cmd = Command::new("open");
                cmd.args(&["-a", browser_cmd]);
                // Add additional args
                cmd.arg("--args");
                for arg in &parts[1..] {
                    cmd.arg(arg);
                }
                cmd.arg(url);
                cmd.status()
            } else {
                Command::new("open")
                    .args(&["-a", browser, url])
                    .status()
            }
        }
    } else {
        // Linux/Unix
        if browser.to_lowercase() == "default" {
            Command::new("xdg-open")
                .arg(url)
                .status()
        } else {
            // Parse browser string to handle command + args (e.g., "firefox -P someProfile")
            let parts: Vec<&str> = browser.split_whitespace().collect();
            if parts.len() > 1 {
                // Browser has arguments
                let browser_cmd = parts[0];
                let mut cmd = Command::new(browser_cmd);
                for arg in &parts[1..] {
                    cmd.arg(arg);
                }
                cmd.arg(url);
                cmd.status()
            } else {
                Command::new(browser)
                    .arg(url)
                    .status()
            }
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