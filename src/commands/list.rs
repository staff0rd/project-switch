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

const PATH_PREFIX: &str = "[path] ";

fn is_file_path(input: &str) -> bool {
    let s = input.trim();
    // Drive letter pattern: c: or D:\ etc.
    if s.len() >= 2 && s.as_bytes()[0].is_ascii_alphabetic() && s.as_bytes()[1] == b':' {
        return true;
    }
    // UNC paths: \\server\...
    if s.starts_with("\\\\") {
        return true;
    }
    false
}

fn get_file_suggestions(input: &str) -> Vec<String> {
    // Normalize forward slashes to backslashes
    let normalized = input.replace('/', "\\");

    // Handle bare drive letter like "c:" -> "c:\"
    let working = if normalized.len() == 2
        && normalized.as_bytes()[0].is_ascii_alphabetic()
        && normalized.as_bytes()[1] == b':'
    {
        format!("{}\\", normalized)
    } else {
        normalized
    };

    // Split into directory portion and partial name filter
    let (initial_dir, initial_filter) = match working.rfind('\\') {
        Some(pos) => (working[..=pos].to_string(), working[pos + 1..].to_string()),
        None => return Vec::new(),
    };

    let mut result = Vec::new();
    let mut dir_part = initial_dir;
    let mut filter = initial_filter;
    let mut show_self = true;

    // Loop to auto-expand when a filter matches exactly one directory
    for _ in 0..10 {
        // Show directory itself when browsing contents (empty filter), first level only
        if filter.is_empty() && show_self {
            result.push(format!("{}{}", PATH_PREFIX.cyan(), dir_part.bold().cyan()));
        }

        let entries = match std::fs::read_dir(&dir_part) {
            Ok(e) => e,
            Err(_) => break,
        };

        let filter_lower = filter.to_lowercase();
        let mut dirs: Vec<(String, String)> = Vec::new();
        let mut files: Vec<(String, String)> = Vec::new();

        for entry in entries.flatten() {
            let name = match entry.file_name().into_string() {
                Ok(n) => n,
                Err(_) => continue,
            };

            if !filter.is_empty() && !name.to_lowercase().starts_with(&filter_lower) {
                continue;
            }

            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            if is_dir {
                dirs.push((format!("{}{}\\", dir_part, name), name));
            } else {
                files.push((format!("{}{}", dir_part, name), name));
            }
        }

        dirs.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
        files.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

        for (full, _) in &dirs {
            result.push(format!("{}{}", PATH_PREFIX.cyan(), full.bold().cyan()));
        }

        // Single directory match, no files — auto-expand into it
        if dirs.len() == 1 && files.is_empty() {
            dir_part = dirs[0].0.clone();
            filter = String::new();
            show_self = false;
            continue;
        }

        for (full, _) in &files {
            result.push(format!("{}{}", PATH_PREFIX.cyan(), full));
        }

        break;
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
        if is_file_path(input) {
            return Ok(get_file_suggestions(input));
        }

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
            // File path format: "[path] full\path\" or "[path] full\path\file"
            if let Some(full_path) = clean.strip_prefix(PATH_PREFIX) {
                if is_file_path(full_path) {
                    return Ok(Some(full_path.to_string()));
                }
            }
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

    let resolved = config_manager.resolve_current_project();
    let display_name = resolved
        .as_ref()
        .map(|(name, _)| name.as_str())
        .unwrap_or("global");

    // Collect commands from both project and global
    let mut all_commands = Vec::new();

    if let Some((_, project)) = &resolved {
        if let Some(project_commands) = &project.commands {
            all_commands.extend(project_commands.iter().cloned());
        }
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
                display_name
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
        display_name
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
        } else if let Some(rest) = stripped.strip_prefix(PATH_PREFIX) {
            rest.to_string()
        } else {
            stripped
        }
    };

    // Handle file paths early (before keyword split, since paths can contain spaces)
    if is_file_path(&cleaned_input) {
        let path = std::path::Path::new(&cleaned_input);
        if path.exists() {
            browser::launch_shortcut(&cleaned_input)?;
            return Ok(());
        } else {
            anyhow::bail!("Path does not exist: '{}'", cleaned_input);
        }
    }

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
                    .or_else(|| resolved.as_ref().and_then(|(_, p)| p.browser.as_deref()))
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
                let browser_name = resolved
                    .as_ref()
                    .and_then(|(_, p)| p.browser.as_deref())
                    .unwrap_or_else(|| config_manager.get_default_browser());
                return browser::open_url_in_browser(&url, browser_name);
            }
            anyhow::bail!("No command found matching '{}'", keyword);
        }
    }

    Ok(())
}
