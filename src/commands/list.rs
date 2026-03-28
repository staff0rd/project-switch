use crate::config::ConfigManager;
use crate::launcher::{
    encode_url_args, eval_calculator, filter_items, get_path_entries, is_file_path, merge_args,
    resolve_item, strip_ansi_codes, ListItem, ListItemKind,
};
use crate::utils::browser;
use crate::utils::shortcuts;
use crate::utils::url::is_url;
use anyhow::Result;
use colored::*;
use inquire::Autocomplete;

const PATH_PREFIX: &str = "[path] ";

fn get_file_suggestions(input: &str) -> Vec<String> {
    let entries = get_path_entries(input);
    entries
        .into_iter()
        .map(|entry| {
            if entry.is_dir {
                format!("{}{}", PATH_PREFIX.cyan(), entry.full_path.bold().cyan())
            } else {
                format!("{}{}", PATH_PREFIX.cyan(), entry.full_path)
            }
        })
        .collect()
}

const APP_PREFIX: &str = "[app] ";

fn format_suggestion(item: &ListItem) -> String {
    match &item.kind {
        ListItemKind::Command => {
            let truncated = if item.display_detail.len() > 60 {
                format!("{}...", &item.display_detail[..57])
            } else {
                item.display_detail.clone()
            };
            format!("{} → {}", item.key.green().bold(), truncated.bright_blue())
        }
        ListItemKind::Shortcut { .. } => {
            format!("{}{}", APP_PREFIX.cyan(), item.key.yellow())
        }
    }
}

#[derive(Clone)]
struct ListAutocomplete {
    items: Vec<ListItem>,
}

impl ListAutocomplete {
    fn matching_suggestions(&self, keyword: &str) -> Vec<String> {
        filter_items(&self.items, keyword)
            .into_iter()
            .map(format_suggestion)
            .collect()
    }
}

impl Autocomplete for ListAutocomplete {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, inquire::CustomUserError> {
        // Calculator mode: show result as a suggestion
        if let Some(expr) = input.strip_prefix('=') {
            if expr.trim().is_empty() {
                return Ok(vec![format!(
                    "{}",
                    "Type a math expression (e.g. =5+1)".dimmed()
                )]);
            }
            return Ok(match eval_calculator(expr) {
                Ok(display) => {
                    vec![format!("{}", format!("= {}", display).bold().green())]
                }
                Err(_) => vec![format!("{}", "Invalid expression".red())],
            });
        }

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
                vec![format_suggestion(matched)]
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

fn load_items(
    config_manager: &ConfigManager,
) -> (Vec<crate::config::ProjectCommand>, Vec<ListItem>) {
    let resolved = config_manager.resolve_current_project();

    let mut all_commands = Vec::new();
    if let Some((_, project)) = &resolved {
        if let Some(project_commands) = &project.commands {
            all_commands.extend(project_commands.iter().cloned());
        }
    }
    if let Some(global_commands) = config_manager.get_global_commands() {
        all_commands.extend(global_commands.iter().cloned());
    }
    all_commands.sort_by(|a, b| a.key.cmp(&b.key));
    all_commands.dedup_by(|a, b| a.key == b.key);

    let mut all_items: Vec<ListItem> = all_commands
        .iter()
        .map(|cmd| ListItem {
            key: cmd.key.clone(),
            display_detail: cmd
                .url
                .clone()
                .or_else(|| cmd.command.clone())
                .unwrap_or_default(),
            kind: ListItemKind::Command,
        })
        .collect();

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

    (all_commands, all_items)
}

/// Execute an action from the GUI launcher. Called when user presses Enter.
/// Takes the raw input text from the GUI and dispatches the appropriate action.
pub fn execute_action(input: &str) -> Result<()> {
    let config_manager = ConfigManager::new()?;
    let resolved = config_manager.resolve_current_project();

    // File path mode
    if is_file_path(input) {
        let path = std::path::Path::new(input);
        if path.exists() {
            browser::launch_shortcut(input, false)?;
            return Ok(());
        } else {
            anyhow::bail!("Path does not exist: '{}'", input);
        }
    }

    let (sorted_commands, all_items) = load_items(&config_manager);

    let keyword = input.split_whitespace().next().unwrap_or(input);

    match resolve_item(&all_items, input) {
        Some((item, args)) => match &item.kind {
            ListItemKind::Shortcut { path } => {
                browser::launch_shortcut(path, false)?;
            }
            ListItemKind::Command => {
                let selected_command = sorted_commands
                    .iter()
                    .find(|cmd| cmd.key.to_lowercase() == item.key.to_lowercase())
                    .ok_or_else(|| anyhow::anyhow!("Command '{}' not found", item.key))?;

                if let Some(ref cmd_str) = selected_command.command {
                    let final_args = merge_args(selected_command.args.as_deref(), args.as_deref());
                    browser::open_command_with_args(cmd_str, None, final_args.as_deref(), false)?;
                } else {
                    let url = selected_command.url.as_ref().ok_or_else(|| {
                        anyhow::anyhow!(
                            "Command '{}' has neither 'url' nor 'command' configured",
                            selected_command.key
                        )
                    })?;

                    let resolved_browser = selected_command
                        .browser
                        .as_deref()
                        .or_else(|| resolved.as_ref().and_then(|(_, p)| p.browser.as_deref()))
                        .or_else(|| Some(config_manager.get_default_browser()));

                    let effective_browser;
                    let browser_arg = match resolved_browser {
                        Some(b) => {
                            let b = match selected_command.args.as_deref() {
                                Some(a) => {
                                    effective_browser = format!("{} {}", b, a);
                                    effective_browser.as_str()
                                }
                                None => b,
                            };
                            Some(b)
                        }
                        None => None,
                    };

                    let final_url;
                    let effective_url = if browser_arg.is_some() {
                        if let Some(ref user_args) = args {
                            final_url = encode_url_args(url, user_args);
                            &final_url
                        } else {
                            url
                        }
                    } else {
                        url
                    };

                    let final_args = if browser_arg.is_some() {
                        None
                    } else {
                        merge_args(selected_command.args.as_deref(), args.as_deref())
                    };

                    browser::open_command_with_args(
                        effective_url,
                        browser_arg,
                        final_args.as_deref(),
                        false,
                    )?;
                }
            }
        },
        None => {
            if is_url(keyword) {
                let url = if keyword.starts_with("http://") || keyword.starts_with("https://") {
                    keyword.to_string()
                } else {
                    format!("https://{}", keyword)
                };
                let browser_name = resolved
                    .as_ref()
                    .and_then(|(_, p)| p.browser.as_deref())
                    .unwrap_or_else(|| config_manager.get_default_browser());
                return browser::open_url_in_browser(&url, browser_name, false);
            }
            anyhow::bail!("No command found matching '{}'", keyword);
        }
    }

    Ok(())
}

pub fn execute_gui() -> Result<()> {
    let config_manager = ConfigManager::new()?;
    let display_name = config_manager
        .resolve_current_project()
        .map(|(name, _)| name.clone())
        .unwrap_or_else(|| "global".to_string());

    let (_, all_items) = load_items(&config_manager);

    let mut state = crate::ui::WindowState::new(all_items);
    state.show();

    let options = eframe::NativeOptions {
        centered: true,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([700.0, 500.0])
            .with_decorations(false)
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native(
        "project-switch",
        options,
        Box::new(move |cc| {
            let mut style = (*cc.egui_ctx.style()).clone();
            style.text_styles.insert(
                eframe::egui::TextStyle::Body,
                eframe::egui::FontId::proportional(18.0),
            );
            style.text_styles.insert(
                eframe::egui::TextStyle::Button,
                eframe::egui::FontId::proportional(18.0),
            );
            style.text_styles.insert(
                eframe::egui::TextStyle::Monospace,
                eframe::egui::FontId::monospace(16.0),
            );
            cc.egui_ctx.set_style(style);
            Ok(Box::new(crate::ui::LauncherApp::new(state, display_name)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("GUI error: {}", e))
}

pub fn execute(_debug: bool) -> Result<()> {
    let config_manager = ConfigManager::new()?;

    let display_name = config_manager
        .resolve_current_project()
        .map(|(name, _)| name.as_str())
        .unwrap_or("global");

    let (_, all_items) = load_items(&config_manager);

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

    // Calculator mode: input starting with '=' evaluates a math expression
    if let Some(expr) = user_input.strip_prefix('=') {
        match eval_calculator(expr) {
            Ok(display) => {
                println!("{}", format!("= {}", display).bold().green());
                return Ok(());
            }
            Err(e) => anyhow::bail!("Math error: {}", e),
        }
    }

    // Clean the input (strip ANSI codes from inquire's colored output)
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

    execute_action(&cleaned_input)
}
