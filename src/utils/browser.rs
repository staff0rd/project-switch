use anyhow::Result;
use colored::*;
use std::process::Command;

pub fn open_command_with_args(
    command: &str,
    browser: Option<&str>,
    args: Option<&str>,
    debug: bool,
) -> Result<()> {
    if let Some(browser) = browser {
        open_url_in_browser(command, browser, debug)
    } else {
        run_terminal_command(command, args, debug)
    }
}

fn run_terminal_command(command: &str, args: Option<&str>, debug: bool) -> Result<()> {
    let mut full_command = command.to_string();
    if let Some(args_str) = args {
        if !args_str.is_empty() {
            full_command.push(' ');
            full_command.push_str(args_str);
        }
    }

    let args_str = args
        .filter(|s| !s.is_empty())
        .map(|a| format!(" {}", a))
        .unwrap_or_default();

    if cfg!(target_os = "windows") {
        if debug {
            println!("{}", format!("[debug] cmd /c {}", full_command).dimmed());
        }
        // Spawn via cmd so PATH and .cmd scripts are resolved; don't wait.
        Command::new("cmd")
            .args(["/c", &full_command])
            .spawn()
            .map_err(|e| anyhow::anyhow!("Error running command: {}", e))?;
    } else {
        if debug {
            println!("{}", format!("[debug] sh -c {}", full_command).dimmed());
        }
        let status = Command::new("sh")
            .args(["-c", &full_command])
            .status()
            .map_err(|e| anyhow::anyhow!("Error running command: {}", e))?;
        if !status.success() {
            anyhow::bail!("Command failed: {}{}", command, args_str);
        }
    }

    println!(
        "{}",
        format!("Running command: {}{}", command, args_str).green()
    );
    Ok(())
}

pub fn launch_shortcut(path: &str, debug: bool) -> Result<()> {
    let status = if cfg!(target_os = "windows") {
        let ps_cmd = format!("Start-Process '{}'", path);
        if debug {
            println!(
                "{}",
                format!("[debug] powershell -Command {}", ps_cmd).dimmed()
            );
        }
        Command::new("powershell")
            .args(["-Command", &ps_cmd])
            .status()
    } else if cfg!(target_os = "macos") {
        if debug {
            println!("{}", format!("[debug] open {}", path).dimmed());
        }
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

/// The `prefix`-joined extra args as they appear in a debug command line, or an
/// empty string when there are none (so it can be spliced in unconditionally).
fn debug_extra_args(prefix: &str, extra_args: &[&str]) -> String {
    if extra_args.is_empty() {
        String::new()
    } else {
        format!("{}{}", prefix, extra_args.join(" "))
    }
}

pub fn open_url_in_browser(url: &str, browser: &str, debug: bool) -> Result<()> {
    let cmd_result = if cfg!(target_os = "windows") {
        // Encode spaces so PowerShell doesn't split the URL when passing to Start-Process
        let url = &url.replace(' ', "%20");
        if browser.to_lowercase() == "default" {
            let ps_cmd = format!("Set-Location C:\\; Start-Process '{}'", url);
            if debug {
                println!(
                    "{}",
                    format!("[debug] powershell -Command {}", ps_cmd).dimmed()
                );
            }
            Command::new("powershell")
                .args(["-Command", &ps_cmd])
                .status()
        } else {
            let (browser_cmd, extra_args) = parse_browser_with_args(browser);
            let ps_command = if extra_args.is_empty() {
                format!(
                    "Set-Location C:\\; Start-Process '{}' @('{}')",
                    browser_cmd, url
                )
            } else {
                format!(
                    "Set-Location C:\\; Start-Process '{}' @({}, '{}')",
                    browser_cmd,
                    extra_args
                        .iter()
                        .map(|a| format!("'{}'", a))
                        .collect::<Vec<_>>()
                        .join(", "),
                    url
                )
            };
            if debug {
                println!(
                    "{}",
                    format!("[debug] powershell -Command {}", ps_command).dimmed()
                );
            }
            Command::new("powershell")
                .args(["-Command", &ps_command])
                .status()
        }
    } else if cfg!(target_os = "macos") {
        if browser.to_lowercase() == "default" {
            if debug {
                println!("{}", format!("[debug] open {}", url).dimmed());
            }
            Command::new("open").arg(url).status()
        } else {
            let (browser_cmd, extra_args) = parse_browser_with_args(browser);
            if debug {
                let extra = debug_extra_args(" --args ", &extra_args);
                println!(
                    "{}",
                    format!("[debug] open -a {}{} {}", browser_cmd, extra, url).dimmed()
                );
            }
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
            if debug {
                println!("{}", format!("[debug] xdg-open {}", url).dimmed());
            }
            Command::new("xdg-open").arg(url).status()
        } else {
            let (browser_cmd, extra_args) = parse_browser_with_args(browser);
            if debug {
                let extra = debug_extra_args(" ", &extra_args);
                println!(
                    "{}",
                    format!("[debug] {}{} {}", browser_cmd, extra, url).dimmed()
                );
            }
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
