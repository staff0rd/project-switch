//! egui launcher window — renders the text input and filtered list.

use crate::commands::list::{execute_action, BrowserPick};
use crate::config::BrowserOption;
use crate::launcher::browser_picker::BrowserPicker;
use crate::launcher::grocery_picker::GroceryRow;
use crate::launcher::key_combo::KeyCombo;
use crate::launcher::picker::{PickerEntry, PickerOutcome};
use crate::launcher::{get_path_entries, CalcResult, ListItem, ListItemKind};
use crate::ui::state::{FilteredEntry, InputMode, Visibility, WindowState};
use eframe::egui;

const ERROR_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 110, 110);
const CURRENT_COLOR: egui::Color32 = egui::Color32::from_rgb(100, 200, 100);

/// Keep the highlighted row on screen when the keyboard moved the selection.
fn keep_selected_visible(response: &egui::Response, is_selected: bool, key_moved: bool) {
    if is_selected && key_moved {
        response.scroll_to_me(Some(egui::Align::Center));
    }
}

fn empty_list_label(ui: &mut egui::Ui, message: &str) {
    ui.label(egui::RichText::new(message).color(egui::Color32::GRAY));
}

fn move_caret_to_end(ctx: &egui::Context, id: egui::Id, text: &str) {
    if let Some(mut edit_state) = egui::TextEdit::load_state(ctx, id) {
        let end = egui::text::CCursor::new(text.chars().count());
        edit_state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(end)));
        edit_state.store(ctx, id);
    }
}

fn set_path_input(state: &mut WindowState, prev_input: &mut String, path: &str) {
    state.input = path.to_string();
    *prev_input = state.input.clone();
    let new_input = state.input.clone();
    state.set_input(new_input);
}

fn open_path_and_hide(state: &mut WindowState, path: String) {
    state.hide();
    state.spawn_action(move || execute_action(&path, BrowserPick::First));
}

fn execute_and_hide(state: &mut WindowState, action_input: &str) {
    if action_input.starts_with('=') {
        return;
    }
    let input = action_input.to_string();
    state.hide();
    state.spawn_action(move || execute_action(&input, BrowserPick::First));
}

fn activate_selected_item(state: &mut WindowState, prev_input: &mut String) {
    let (Some(item), Some(action)) = (state.selected_item(), state.selected_action_input()) else {
        return;
    };
    activate_item(state, prev_input, item, action);
}

fn activate_item(state: &mut WindowState, prev_input: &mut String, item: ListItem, action: String) {
    match item.kind {
        ListItemKind::Switch => open_switch_picker(state, prev_input, &item.key),
        ListItemKind::Command => match crate::commands::list::browser_choices(&item.key) {
            Ok(Some(options)) => {
                state.open_browser_picker(BrowserPicker::new(item.key, action, options));
                *prev_input = state.input.clone();
            }
            Ok(None) => execute_and_hide(state, &action),
            Err(e) => state.set_error(format!("{e:#}")),
        },
        ListItemKind::Grocery => activate_grocery(state, prev_input, &item.key, &action),
        ListItemKind::Shortcut { .. } => execute_and_hide(state, &action),
    }
}

fn pressed_combo(event: &egui::Event) -> Option<KeyCombo> {
    let egui::Event::Key {
        key,
        pressed: true,
        modifiers,
        ..
    } = event
    else {
        return None;
    };
    let mut name = key.name().chars();
    let key = match (name.next(), name.next()) {
        (Some(c), None) if c.is_ascii_alphanumeric() => c.to_ascii_uppercase(),
        _ => return None,
    };
    Some(KeyCombo {
        alt: modifiers.alt,
        ctrl: modifiers.ctrl,
        shift: modifiers.shift,
        key,
    })
}

fn consume_hotkey_and_its_text(ui: &egui::Ui, state: &WindowState) -> Option<ListItem> {
    ui.input_mut(|input| {
        let (index, item) = input.events.iter().enumerate().find_map(|(i, event)| {
            pressed_combo(event)
                .and_then(|combo| state.hotkey_item(combo))
                .map(|item| (i, item))
        })?;
        input.events.remove(index);
        input
            .events
            .retain(|event| !matches!(event, egui::Event::Text(_)));
        Some(item)
    })
}

fn activate_grocery(
    state: &mut WindowState,
    prev_input: &mut String,
    key: &str,
    action_input: &str,
) {
    let args = action_input
        .split_once(char::is_whitespace)
        .map(|(_, rest)| rest);
    crate::history::record(key).ok();
    let Some(command) = crate::commands::grocery::launcher_add(args) else {
        state.open_grocery_picker();
        *prev_input = state.input.clone();
        state.spawn_grocery(|| crate::commands::grocery::apply_and_list(None));
        return;
    };
    state.hide();
    state
        .spawn_action(move || crate::commands::grocery::run_with_loaded_config(&command).map(drop));
}

fn render_grocery_picker(
    ui: &mut egui::Ui,
    state: &mut WindowState,
    prev_input: &mut String,
    key_down: bool,
    key_up: bool,
    key_enter: bool,
) {
    if state.grocery_picker.as_ref().is_some_and(|p| p.loading) {
        empty_list_label(ui, "Loading grocery list...");
        return;
    }
    let rows = state.grocery_rows();
    let selected = navigate_rows(state, rows.len(), key_down, key_up);

    let activate = key_enter || {
        let labels = rows
            .iter()
            .map(|row| match row {
                GroceryRow::Item(item) => egui::RichText::new(format!("   {item}")),
                GroceryRow::Add(item) => {
                    egui::RichText::new(format!("+  Add \"{item}\"")).color(CURRENT_COLOR)
                }
            })
            .collect();
        let clicked = render_rows(
            ui,
            labels,
            selected,
            key_down || key_up,
            "The default grocery list is empty: type an item to add it",
        );
        if let Some(i) = clicked {
            state.selected = i;
        }
        clicked.is_some()
    };
    if !activate {
        return;
    }
    if let Some(command) = state.grocery_command() {
        state.spawn_grocery(move || crate::commands::grocery::apply_and_list(Some(&command)));
        *prev_input = state.input.clone();
    }
}

fn launch_with_browser(state: &mut WindowState, prev_input: &mut String, option: BrowserOption) {
    let Some(picker) = state.browser_picker.take() else {
        return;
    };
    state.close_picker();
    *prev_input = state.input.clone();
    state.hide();
    crate::browser_choice::record(&picker.command_key, &option.label).ok();
    let input = picker.action_input;
    state.spawn_action(move || execute_action(&input, BrowserPick::Chosen(option.value)));
}

fn navigate_rows(state: &mut WindowState, count: usize, key_down: bool, key_up: bool) -> usize {
    if key_down {
        state.navigate_down_bounded(count);
    }
    if key_up {
        state.navigate_up();
    }
    state.selected.min(count.saturating_sub(1))
}

fn render_rows(
    ui: &mut egui::Ui,
    rows: Vec<egui::RichText>,
    selected: usize,
    key_moved: bool,
    empty_message: &str,
) -> Option<usize> {
    let mut clicked = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        if rows.is_empty() {
            empty_list_label(ui, empty_message);
        }
        for (i, text) in rows.into_iter().enumerate() {
            let is_selected = i == selected;
            let text = if is_selected { text.strong() } else { text };
            let response = ui.selectable_label(is_selected, text);
            if response.clicked() {
                clicked = Some(i);
            }
            keep_selected_visible(&response, is_selected, key_moved);
        }
    });
    clicked
}

fn render_browser_picker(
    ui: &mut egui::Ui,
    state: &mut WindowState,
    prev_input: &mut String,
    key_down: bool,
    key_up: bool,
    key_enter: bool,
) {
    let entries = state.browser_entries();
    let selected = navigate_rows(state, entries.len(), key_down, key_up);

    let chosen = if key_enter {
        state.selected_browser()
    } else {
        let rows = entries
            .iter()
            .map(|option| egui::RichText::new(format!("   {}", option.label)))
            .collect();
        render_rows(
            ui,
            rows,
            selected,
            key_down || key_up,
            "No matching browsers",
        )
        .and_then(|i| entries.get(i).cloned())
    };
    if let Some(option) = chosen {
        launch_with_browser(state, prev_input, option);
    }
}

/// Enter the in-window client/project picker. The terminal `switch` prompt
/// can't run here — the launcher window has no console attached.
fn open_switch_picker(state: &mut WindowState, prev_input: &mut String, item_key: &str) {
    match crate::commands::switch::picker_state() {
        Ok(picker) => {
            crate::history::record(item_key).ok();
            state.open_picker(picker);
            *prev_input = state.input.clone();
        }
        Err(e) => state.set_error(format!("{e:#}")),
    }
}

fn activate_picker_entry(state: &mut WindowState, prev_input: &mut String, entry: &PickerEntry) {
    let Some(outcome) = state.picker.as_mut().map(|picker| picker.activate(entry)) else {
        return;
    };
    match outcome {
        PickerOutcome::Descended => {
            state.reset_picker_cursor();
            *prev_input = state.input.clone();
        }
        PickerOutcome::Chosen { client, project } => {
            state.close_picker();
            *prev_input = state.input.clone();
            state.hide();
            state.spawn_action(move || {
                crate::commands::switch::apply_selection(&client, project.as_deref())
            });
        }
    }
}

fn render_picker(
    ui: &mut egui::Ui,
    state: &mut WindowState,
    prev_input: &mut String,
    key_down: bool,
    key_up: bool,
    key_enter: bool,
) {
    let entries = match &state.picker {
        Some(picker) => picker.entries(&state.input),
        None => return,
    };

    let selected = navigate_rows(state, entries.len(), key_down, key_up);

    let chosen = if key_enter {
        entries.get(selected)
    } else {
        let rows = entries
            .iter()
            .map(|entry| {
                if entry.is_current {
                    egui::RichText::new(format!("▶ {} (current)", entry.label)).color(CURRENT_COLOR)
                } else {
                    egui::RichText::new(format!("   {}", entry.label))
                }
            })
            .collect();
        render_rows(ui, rows, selected, key_down || key_up, "No clients found")
            .and_then(|i| entries.get(i))
    };
    if let Some(entry) = chosen {
        activate_picker_entry(state, prev_input, entry);
    }
}

/// Hide on focus loss, then show or hide the window to match the state. Returns whether it is visible.
pub fn sync_visibility(ctx: &egui::Context, state: &mut WindowState) -> bool {
    let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
    state.hide_on_focus_loss(focused);

    let visible = state.visibility == Visibility::Visible;
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(visible));
    visible
}

/// Render the launcher UI inside a CentralPanel. Shared by both standalone and daemon modes.
pub fn render_launcher(
    ui: &mut egui::Ui,
    state: &mut WindowState,
    project_name: &str,
    prev_input: &mut String,
) {
    egui::CentralPanel::default().show(ui, |ui| {
        // Draggable title bar
        let (title_rect, response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), egui::Sense::drag());
        if response.dragged() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        let title = match (&state.picker, &state.browser_picker, &state.grocery_picker) {
            (Some(picker), _, _) => picker.title(),
            (None, Some(picker), _) => picker.title(),
            (None, None, Some(picker)) => picker.title(),
            (None, None, None) => project_name.to_string(),
        };
        ui.painter().text(
            title_rect.left_center() + egui::vec2(4.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &title,
            egui::FontId::proportional(16.0),
            egui::Color32::GRAY,
        );

        let main_list_open = state.picker.is_none()
            && state.browser_picker.is_none()
            && state.input_mode() == InputMode::Normal;
        if main_list_open {
            if let Some(item) = consume_hotkey_and_its_text(ui, state) {
                let action = item.key.clone();
                activate_item(state, prev_input, item, action);
                ui.ctx().request_repaint();
                return;
            }
        }

        // Text input
        let input_response = ui.add(
            egui::TextEdit::singleline(&mut state.input)
                .hint_text("Type to filter...")
                .desired_width(f32::INFINITY)
                .event_filter(egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                }),
        );
        if !input_response.has_focus() {
            input_response.request_focus();
        }

        // Detect input changes
        if state.input != *prev_input {
            *prev_input = state.input.clone();
            let new_input = state.input.clone();
            state.set_input(new_input);
        }

        // Keyboard
        let key_down = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));
        let key_up = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
        let key_escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let key_enter = ui.input(|i| i.key_pressed(egui::Key::Enter));

        if key_escape {
            if state.picker.is_some() {
                if !state.picker_back() {
                    state.close_picker();
                }
                *prev_input = state.input.clone();
            } else if state.browser_picker.is_some() || state.grocery_picker.is_some() {
                state.close_picker();
                *prev_input = state.input.clone();
            } else {
                state.hide();
            }
            return;
        }

        if let Some(error) = state.error.clone() {
            ui.label(egui::RichText::new(error).color(ERROR_COLOR));
        }

        ui.separator();

        if state.picker.is_some() {
            render_picker(ui, state, prev_input, key_down, key_up, key_enter);
            return;
        }
        if state.browser_picker.is_some() {
            render_browser_picker(ui, state, prev_input, key_down, key_up, key_enter);
            return;
        }
        if state.grocery_picker.is_some() {
            render_grocery_picker(ui, state, prev_input, key_down, key_up, key_enter);
            return;
        }

        match state.input_mode() {
            InputMode::Calculator { result } => {
                if key_enter {
                    if let CalcResult::Ok(_) = &result {
                        crate::history::record(&state.input).ok();
                    }
                    return;
                }
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    match result {
                        CalcResult::Ok(value) => {
                            ui.label(
                                egui::RichText::new(format!("= {}", value))
                                    .size(28.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(100, 200, 100)),
                            );
                        }
                        CalcResult::Incomplete(partial) => {
                            ui.label(
                                egui::RichText::new(format!("= {}...", partial))
                                    .size(28.0)
                                    .strong()
                                    .color(egui::Color32::GRAY),
                            );
                        }
                        CalcResult::Invalid => {
                            ui.label(
                                egui::RichText::new("invalid expression")
                                    .size(16.0)
                                    .color(egui::Color32::GRAY),
                            );
                        }
                    }
                });
            }
            InputMode::FilePath => {
                let entries = get_path_entries(&state.input);
                let selected = navigate_rows(state, entries.len(), key_down, key_up);

                if key_enter && !entries.is_empty() {
                    let path = entries[selected].full_path.clone();
                    if path.ends_with('\\') {
                        set_path_input(state, prev_input, &path);
                    } else {
                        open_path_and_hide(state, path);
                    }
                    return;
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, entry) in entries.iter().enumerate() {
                        let is_selected = i == selected;
                        let label = if entry.is_dir {
                            egui::RichText::new(&entry.full_path)
                                .strong()
                                .color(egui::Color32::from_rgb(100, 180, 255))
                        } else {
                            egui::RichText::new(&entry.full_path)
                        };
                        let response = ui.selectable_label(is_selected, label);
                        if response.clicked() {
                            set_path_input(state, prev_input, &entry.full_path);
                        }
                        keep_selected_visible(&response, is_selected, key_down || key_up);
                    }
                    if entries.is_empty() {
                        empty_list_label(ui, "No entries found");
                    }
                });
            }
            InputMode::Normal => {
                let key_tab =
                    ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab));
                if key_tab {
                    if let Some(completion) = state.selected_completion() {
                        *prev_input = completion.clone();
                        state.set_input(completion);
                        move_caret_to_end(ui.ctx(), input_response.id, &state.input);
                        ui.ctx().request_repaint();
                    }
                }
                if key_down {
                    state.navigate_down();
                }
                if key_up {
                    state.navigate_up();
                }

                // Handle Enter on non-item recents (expression/path) first,
                // extracting owned data before taking mutable borrows.
                if key_enter {
                    enum RecentAction {
                        Expression(String),
                        Path(String),
                        None,
                    }
                    let action = {
                        let entries = state.filtered_entries();
                        let sel = state.selected;
                        if !entries.is_empty() && sel < entries.len() {
                            match &entries[sel] {
                                FilteredEntry::Expression { input, .. } => {
                                    RecentAction::Expression(input.clone())
                                }
                                FilteredEntry::Path(path) => RecentAction::Path(path.clone()),
                                FilteredEntry::Item(_) => RecentAction::None,
                            }
                        } else {
                            RecentAction::None
                        }
                    };
                    match action {
                        RecentAction::Expression(expr_input) => {
                            *prev_input = expr_input.clone();
                            state.set_input(expr_input);
                            return;
                        }
                        RecentAction::Path(path) => {
                            open_path_and_hide(state, path);
                            return;
                        }
                        RecentAction::None => {}
                    }
                }

                let entries = state.filtered_entries();
                let selected = state.selected;

                if key_enter && !entries.is_empty() && selected < entries.len() {
                    activate_selected_item(state, prev_input);
                    return;
                }
                if key_enter && entries.is_empty() && crate::utils::url::is_url(&state.input) {
                    execute_and_hide(state, &state.input.clone());
                    return;
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, entry) in entries.iter().enumerate() {
                        let is_selected = i == selected;

                        let label = match entry {
                            FilteredEntry::Item(item) => match &item.kind {
                                ListItemKind::Command
                                | ListItemKind::Switch
                                | ListItemKind::Grocery => {
                                    let text = item.row_text();
                                    if is_selected {
                                        egui::RichText::new(text).strong()
                                    } else {
                                        egui::RichText::new(text)
                                    }
                                }
                                ListItemKind::Shortcut { .. } => {
                                    let text = format!("[app] {}", item.key);
                                    let rt = egui::RichText::new(text)
                                        .color(egui::Color32::from_rgb(100, 200, 200));
                                    if is_selected {
                                        rt.strong()
                                    } else {
                                        rt
                                    }
                                }
                            },
                            FilteredEntry::Expression { display, .. } => {
                                let rt = egui::RichText::new(display)
                                    .color(egui::Color32::from_rgb(100, 200, 100));
                                if is_selected {
                                    rt.strong()
                                } else {
                                    rt
                                }
                            }
                            FilteredEntry::Path(path) => {
                                let text = format!("[path] {}", path);
                                let rt = egui::RichText::new(text)
                                    .color(egui::Color32::from_rgb(100, 180, 255));
                                if is_selected {
                                    rt.strong()
                                } else {
                                    rt
                                }
                            }
                        };

                        let response = ui.selectable_label(is_selected, label);

                        keep_selected_visible(&response, is_selected, key_down || key_up);
                    }
                });
            }
        }
    });
}

pub struct LauncherApp {
    state: WindowState,
    project_name: String,
    prev_input: String,
    /// Counts frames since creation; used to request OS focus during startup.
    startup_frames: u32,
    /// Receives shortcuts collected on a background thread.
    shortcut_rx: Option<std::sync::mpsc::Receiver<Vec<crate::launcher::ListItem>>>,
}

impl LauncherApp {
    pub fn new(
        state: WindowState,
        project_name: String,
        shortcut_rx: Option<std::sync::mpsc::Receiver<Vec<crate::launcher::ListItem>>>,
    ) -> Self {
        Self {
            state,
            project_name,
            prev_input: String::new(),
            startup_frames: 0,
            shortcut_rx,
        }
    }
}

impl eframe::App for LauncherApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Merge async-loaded shortcuts when ready
        if let Some(rx) = self.shortcut_rx.take() {
            match rx.try_recv() {
                Ok(items) => self.state.append_items(items),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.shortcut_rx = Some(rx);
                    ctx.request_repaint();
                }
            }
        }

        // Keep polling while a dispatched action is still running so its
        // failure can reach the window.
        self.state.poll_actions();
        if self.state.has_pending_actions() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }

        if !sync_visibility(ctx, &mut self.state) {
            return;
        }

        // A failed action re-shows the window after the startup focus window
        // has passed, so ask for focus again while the error is on screen.
        if self.state.error.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        // Request OS-level window focus during the first few frames.
        // The hotkey service grants us foreground permission via
        // AllowSetForegroundWindow; this triggers SetForegroundWindow
        // through eframe so the window reliably receives input focus.
        if self.startup_frames < 10 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            self.startup_frames += 1;
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.state.visibility == Visibility::Hidden {
            return;
        }
        render_launcher(
            ui,
            &mut self.state,
            &self.project_name,
            &mut self.prev_input,
        );
    }
}
