use crate::config::ConfigManager;
use crate::utils::browser;
use anyhow::Result;
use colored::*;
use inquire::Autocomplete;

/// Check if a string looks like a URL
fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://") ||
    s.starts_with("www.") ||
    // Check for common URL patterns like domain.tld
    (s.contains('.') && !s.contains(' ') && {
        let parts: Vec<&str> = s.split('.').collect();
        parts.len() >= 2 && !parts.last().unwrap_or(&"").is_empty()
    })
}

fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Skip the escape sequence
            if chars.next() == Some('[') {
                // Skip until we find a letter (end of escape sequence)
                for ch in chars.by_ref() {
                    if ch.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

#[derive(Clone)]
struct CommandOption {
    key: String,
    url: String,
}

#[derive(Clone)]
struct CommandAutocomplete {
    options: Vec<CommandOption>,
}

impl Autocomplete for CommandAutocomplete {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, inquire::CustomUserError> {
        // Extract only the keyword part (before first space) for filtering
        let keyword = input.split_whitespace().next().unwrap_or(input);
        
        // Check if user has typed a space (indicating they want to add arguments)
        let has_space = input.contains(' ');
        
        // If there's a space, check for exact match first
        let suggestions: Vec<String> = if has_space {
            // Check if there's an exact match for the keyword
            let exact_match = self.options
                .iter()
                .find(|opt| opt.key.to_lowercase() == keyword.to_lowercase());
            
            if let Some(matched_opt) = exact_match {
                // Only show the exact match
                let truncated_url = if matched_opt.url.len() > 60 {
                    format!("{}...", &matched_opt.url[..57])
                } else {
                    matched_opt.url.clone()
                };
                vec![format!("{} → {}", matched_opt.key.green().bold(), truncated_url.bright_blue())]
            } else {
                // No exact match, show all partial matches
                self.options
                    .iter()
                    .filter(|opt| opt.key.to_lowercase().contains(&keyword.to_lowercase()))
                    .map(|opt| {
                        let truncated_url = if opt.url.len() > 60 {
                            format!("{}...", &opt.url[..57])
                        } else {
                            opt.url.clone()
                        };
                        format!("{} → {}", opt.key.green().bold(), truncated_url.bright_blue())
                    })
                    .collect()
            }
        } else {
            // No space yet, show all partial matches
            self.options
                .iter()
                .filter(|opt| opt.key.to_lowercase().contains(&keyword.to_lowercase()))
                .map(|opt| {
                    let truncated_url = if opt.url.len() > 60 {
                        format!("{}...", &opt.url[..57])
                    } else {
                        opt.url.clone()
                    };
                    format!("{} → {}", opt.key.green().bold(), truncated_url.bright_blue())
                })
                .collect()
        };
        
        Ok(suggestions)
    }

    fn get_completion(
        &mut self,
        input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<inquire::autocompletion::Replacement, inquire::CustomUserError> {
        // If a suggestion is highlighted, extract just the command key from it
        if let Some(suggestion) = highlighted_suggestion {
            // The suggestion format is "key → url", so extract the key part
            if let Some(arrow_pos) = suggestion.find(" → ") {
                let key = suggestion[..arrow_pos].trim();
                // Strip ANSI color codes from the key
                let key_clean = strip_ansi_codes(key);
                return Ok(Some(key_clean));
            }
        }
        // Otherwise keep what the user typed
        Ok(Some(input.to_string()))
    }
}

pub fn execute() -> Result<()> {
    let config_manager = ConfigManager::new()?;
    
    let current_project_name = match config_manager.get_current_project() {
        Some(name) => name,
        None => {
            println!("{}", "Error: No current project selected".red());
            println!("{}", "Use \"project-switch switch\" to select a project first".yellow());
            return Ok(());
        }
    };

    let project = config_manager.get_project(current_project_name)
        .ok_or_else(|| anyhow::anyhow!("Current project not found"))?;

    // Collect commands from both project and global
    let mut all_commands = Vec::new();
    
    // Add project-specific commands
    if let Some(project_commands) = &project.commands {
        all_commands.extend(project_commands.iter().cloned());
    }
    
    // Add global commands
    if let Some(global_commands) = config_manager.get_global_commands() {
        all_commands.extend(global_commands.iter().cloned());
    }

    if all_commands.is_empty() {
        println!("{}", format!("No openable items found in project '{}' or global commands", current_project_name).yellow());
        println!("{}", "Use \"project-switch add\" to add commands to your project".blue());
        return Ok(());
    }

    let mut sorted_commands = all_commands;
    sorted_commands.sort_by(|a, b| a.key.cmp(&b.key));
    // Remove duplicates, keeping project-specific commands over global ones
    sorted_commands.dedup_by(|a, b| a.key == b.key);

    let autocomplete_options: Vec<CommandOption> = sorted_commands
        .iter()
        .map(|cmd| CommandOption {
            key: cmd.key.clone(),
            url: cmd.url.clone().unwrap_or_default(),
        })
        .collect();

    let autocomplete = CommandAutocomplete {
        options: autocomplete_options,
    };

    // Prompt for the keyword and optional arguments with autocomplete
    let user_input = inquire::Text::new(&format!("Enter command (with optional arguments) for '{}':", current_project_name))
        .with_autocomplete(autocomplete)
        .prompt()?;

    // Clean the input in case it contains the formatted suggestion
    let cleaned_input = if let Some(arrow_pos) = user_input.find(" → ") {
        // Extract just the key part before the arrow
        strip_ansi_codes(&user_input[..arrow_pos])
    } else {
        user_input.clone()
    };

    // Parse the input to extract keyword and arguments
    let (keyword, args) = if let Some(space_pos) = cleaned_input.find(' ') {
        let keyword = &cleaned_input[..space_pos];
        let args = cleaned_input[space_pos + 1..].trim();
        (keyword, if args.is_empty() { None } else { Some(args.to_string()) })
    } else {
        (cleaned_input.as_str(), None)
    };

    // Find matching command (case-insensitive)
    let matching_commands: Vec<_> = sorted_commands
        .iter()
        .filter(|cmd| cmd.key.to_lowercase() == keyword.to_lowercase())
        .collect();

    let selected_command = if matching_commands.is_empty() {
        // Try partial match if exact match not found
        let partial_matches: Vec<_> = sorted_commands
            .iter()
            .filter(|cmd| cmd.key.to_lowercase().contains(&keyword.to_lowercase()))
            .collect();

        if partial_matches.is_empty() {
            // No matches - check if the input looks like a URL
            if is_url(keyword) {
                let url = if keyword.starts_with("http://") || keyword.starts_with("https://") {
                    keyword.to_string()
                } else {
                    format!("https://{}", keyword)
                };
                let browser = project.browser.as_deref()
                    .unwrap_or_else(|| config_manager.get_default_browser());
                return browser::open_url_in_browser(&url, browser);
            }
            anyhow::bail!("No command found matching '{}'", keyword);
        }
        partial_matches[0]
    } else {
        matching_commands[0]
    };

    let url = selected_command.url.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Command '{}' does not have a URL configured", selected_command.key))?;

    // Browser hierarchy: command > project > config > default
    let browser = selected_command.browser.as_deref()
        .or(project.browser.as_deref())
        .unwrap_or_else(|| config_manager.get_default_browser());

    // Combine command args with user-provided args
    let final_args = match (selected_command.args.as_deref(), args.as_deref()) {
        (Some(cmd_args), Some(user_args)) => Some(format!("{} {}", cmd_args, user_args)),
        (Some(cmd_args), None) => Some(cmd_args.to_string()),
        (None, Some(user_args)) => Some(user_args.to_string()),
        (None, None) => None,
    };

    browser::open_command_with_args(url, browser, final_args.as_deref(), selected_command.url_encode)?;

    Ok(())
}
