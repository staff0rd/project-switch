use anyhow::Result;
use colored::*;
use std::process::Command;

pub fn open_url_in_browser(url: &str, browser: &str) -> Result<()> {
    let cmd_result = if cfg!(target_os = "windows") {
        if browser.to_lowercase() == "default" {
            Command::new("powershell")
                .args(&["-Command", &format!("Set-Location C:\\; Start-Process '{}'", url)])
                .status()
        } else {
            Command::new("powershell")
                .args(&["-Command", &format!("Set-Location C:\\; Start-Process '{}' '{}'", browser, url)])
                .status()
        }
    } else if cfg!(target_os = "macos") {
        if browser.to_lowercase() == "default" {
            Command::new("open")
                .arg(url)
                .status()
        } else {
            Command::new("open")
                .args(&["-a", browser, url])
                .status()
        }
    } else {
        // Linux/Unix
        if browser.to_lowercase() == "default" {
            Command::new("xdg-open")
                .arg(url)
                .status()
        } else {
            Command::new(browser)
                .arg(url)
                .status()
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