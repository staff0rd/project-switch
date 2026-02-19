use crate::config::ConfigManager;
use crate::utils::browser;
use crate::utils::shortcuts;
use crate::utils::url::is_url;
use anyhow::Result;
use colored::*;
use inquire::Autocomplete;

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
enum ListItemKind {
    Command,
    Shortcut { path: String },
}

#[derive(Clone)]
struct ListItem {
    key: String,
    display_detail: String,
    kind: ListItemKind,
}

const APP_PREFIX: &str = "[app] ";

impl ListItem {
    fn format_suggestion(&self) -> String {
        match &self.kind {
            ListItemKind::Command => {
                let truncated = if self.display_detail.len() > 60 {
                    format!("{}...", &self.display_detail[..57])
                } else {
                    self.display_detail.clone()
                };
                format!("{} → {}", self.key.green().bold(), truncated.bright_blue())
            }
            ListItemKind::Shortcut { .. } => {
                format!("{}{}", APP_PREFIX.cyan(), self.key.yellow())
            }
        }
    }
}

#[derive(Clone)]
struct ListAutocomplete {
    items: Vec<ListItem>,
}

impl ListAutocomplete {
    fn matching_suggestions(&self, keyword: &str) -> Vec<String> {
        self.items
            .iter()
            .filter(|item| item.key.to_lowercase().contains(&keyword.to_lowercase()))
            .map(|item| item.format_suggestion())
            .collect()
    }
}

impl Autocomplete for ListAutocomplete {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, inquire::CustomUserError> {
        let keyword = input.split_whitespace().next().unwrap_or(input);
        let has_space = input.contains(' ');

        let suggestions: Vec<String> = if has_space {
            // Check for exact match on the keyword part
            let exact_match = self
                .items
                .iter()
                .find(|item| item.key.to_lowercase() == keyword.to_lowercase());

            if let Some(matched) = exact_match {
                vec![matched.format_suggestion()]
            } else {
                self.matching_suggestions(keyword)
            }
        } else {
            self.matching_suggestions(keyword)
        };

        Ok(suggestions)
    }

    fn get_completion(
        &mut self,
        input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<inquire::autocompletion::Replacement, inquire::CustomUserError> {
        if let Some(suggestion) = highlighted_suggestion {
            let clean = strip_ansi_codes(&suggestion);
            // Command format: "key → url"
            if let Some(arrow_pos) = clean.find(" → ") {
                return Ok(Some(clean[..arrow_pos].trim().to_string()));
            }
            // Shortcut format: "[app] Name"
            if let Some(rest) = clean.strip_prefix(APP_PREFIX) {
                return Ok(Some(rest.to_string()));
            }
        }
        Ok(Some(input.to_string()))
    }
}

pub fn execute() -> Result<()> {
    let config_manager = ConfigManager::new()?;

    let (current_project_name, project) = match config_manager.resolve_current_project() {
        Some(result) => result,
        None => {
            println!(
                "{}",
                "No current project selected or project not found".red()
            );
            println!(
                "{}",
                "Use \"project-switch switch\" to select a project first".yellow()
            );
            return Ok(());
        }
    };

    // Collect commands from both project and global
    let mut all_commands = Vec::new();

    if let Some(project_commands) = &project.commands {
        all_commands.extend(project_commands.iter().cloned());
    }

    if let Some(global_commands) = config_manager.get_global_commands() {
        all_commands.extend(global_commands.iter().cloned());
    }

    let mut sorted_commands = all_commands;
    sorted_commands.sort_by(|a, b| a.key.cmp(&b.key));
    sorted_commands.dedup_by(|a, b| a.key == b.key);

    // Build list items from commands
    let mut all_items: Vec<ListItem> = sorted_commands
        .iter()
        .map(|cmd| ListItem {
            key: cmd.key.clone(),
            display_detail: cmd.url.clone().unwrap_or_default(),
            kind: ListItemKind::Command,
        })
        .collect();

    // Collect shortcuts if enabled
    let shortcuts_config = config_manager.get_shortcuts_config();
    if shortcuts_config.enabled {
        let extra_paths = shortcuts_config.extra_paths.unwrap_or_default();
        let exclude = shortcuts_config.exclude.unwrap_or_default();
        let shortcut_entries = shortcuts::collect_shortcuts(&extra_paths, &exclude);

        for entry in shortcut_entries {
            all_items.push(ListItem {
                key: entry.name,
                display_detail: entry.path.display().to_string(),
                kind: ListItemKind::Shortcut {
                    path: entry.path.display().to_string(),
                },
            });
        }
    }

    if all_items.is_empty() {
        println!(
            "{}",
            format!(
                "No openable items found in project '{}' or global commands",
                current_project_name
            )
            .yellow()
        );
        println!(
            "{}",
            "Use \"project-switch add\" to add commands to your project".blue()
        );
        return Ok(());
    }

    let autocomplete = ListAutocomplete {
        items: all_items.clone(),
    };

    let user_input = inquire::Text::new(&format!(
        "Enter command (with optional arguments) for '{}':",
        current_project_name
    ))
    .with_autocomplete(autocomplete)
    .prompt()?;

    // Clean the input
    let cleaned_input = {
        let stripped = strip_ansi_codes(&user_input);
        if let Some(arrow_pos) = stripped.find(" → ") {
            stripped[..arrow_pos].trim().to_string()
        } else if let Some(rest) = stripped.strip_prefix(APP_PREFIX) {
            rest.to_string()
        } else {
            stripped
        }
    };

    // Parse keyword and arguments
    let (keyword, args) = if let Some(space_pos) = cleaned_input.find(' ') {
        let keyword = &cleaned_input[..space_pos];
        let args = cleaned_input[space_pos + 1..].trim();
        (
            keyword.to_string(),
            if args.is_empty() {
                None
            } else {
                Some(args.to_string())
            },
        )
    } else {
        (cleaned_input.clone(), None)
    };

    // Try to find a matching item
    let matched_item = all_items
        .iter()
        .find(|item| item.key.to_lowercase() == keyword.to_lowercase())
        .or_else(|| {
            // Partial match fallback
            let matches: Vec<_> = all_items
                .iter()
                .filter(|item| item.key.to_lowercase().contains(&keyword.to_lowercase()))
                .collect();
            matches.into_iter().next()
        });

    match matched_item {
        Some(item) => match &item.kind {
            ListItemKind::Shortcut { path } => {
                browser::launch_shortcut(path)?;
            }
            ListItemKind::Command => {
                // Find the original command for browser/args resolution
                let selected_command = sorted_commands
                    .iter()
                    .find(|cmd| cmd.key.to_lowercase() == item.key.to_lowercase())
                    .ok_or_else(|| anyhow::anyhow!("Command '{}' not found", item.key))?;

                let url = selected_command.url.as_ref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "Command '{}' does not have a URL configured",
                        selected_command.key
                    )
                })?;

                let browser_name = selected_command
                    .browser
                    .as_deref()
                    .or(project.browser.as_deref())
                    .unwrap_or_else(|| config_manager.get_default_browser());

                let final_args = match (selected_command.args.as_deref(), args.as_deref()) {
                    (Some(cmd_args), Some(user_args)) => {
                        Some(format!("{} {}", cmd_args, user_args))
                    }
                    (Some(cmd_args), None) => Some(cmd_args.to_string()),
                    (None, Some(user_args)) => Some(user_args.to_string()),
                    (None, None) => None,
                };

                browser::open_command_with_args(
                    url,
                    browser_name,
                    final_args.as_deref(),
                    selected_command.url_encode,
                )?;
            }
        },
        None => {
            // No matches — check if the input looks like a URL
            if is_url(&keyword) {
                let url = if keyword.starts_with("http://") || keyword.starts_with("https://") {
                    keyword.to_string()
                } else {
                    format!("https://{}", keyword)
                };
                let browser_name = project
                    .browser
                    .as_deref()
                    .unwrap_or_else(|| config_manager.get_default_browser());
                return browser::open_url_in_browser(&url, browser_name);
            }
            anyhow::bail!("No command found matching '{}'", keyword);
        }
    }

    Ok(())
}
