//! egui launcher window — renders the text input and filtered list.

use crate::launcher::{get_path_entries, ListItemKind};
use crate::ui::state::{InputMode, WindowState};
use eframe::egui;

fn set_path_input(state: &mut WindowState, prev_input: &mut String, path: &str) {
    state.input = path.to_string();
    *prev_input = state.input.clone();
    let new_input = state.input.clone();
    state.set_input(new_input);
}

fn execute_and_hide(state: &mut WindowState) {
    let input = state.input.clone();
    if input.starts_with('=') {
        return;
    }
    state.hide();
    std::thread::spawn(move || {
        if let Err(e) = crate::commands::list::execute_action(&input) {
            eprintln!("Action error: {e:#}");
        }
    });
}

/// Render the launcher UI inside a CentralPanel. Shared by both standalone and daemon modes.
pub fn render_launcher(
    ctx: &egui::Context,
    state: &mut WindowState,
    project_name: &str,
    prev_input: &mut String,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        // Draggable title bar
        let (title_rect, response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), egui::Sense::drag());
        if response.dragged() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        ui.painter().text(
            title_rect.left_center() + egui::vec2(4.0, 0.0),
            egui::Align2::LEFT_CENTER,
            project_name,
            egui::FontId::proportional(16.0),
            egui::Color32::GRAY,
        );

        // Text input
        let input_response = ui.add(
            egui::TextEdit::singleline(&mut state.input)
                .hint_text("Type to filter...")
                .desired_width(f32::INFINITY),
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
            state.hide();
            return;
        }

        ui.separator();

        match state.input_mode() {
            InputMode::Calculator { result } => {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    match result {
                        Ok(value) => {
                            ui.label(
                                egui::RichText::new(format!("= {}", value))
                                    .size(28.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(100, 200, 100)),
                            );
                        }
                        Err(msg) => {
                            ui.label(
                                egui::RichText::new(msg)
                                    .size(16.0)
                                    .color(egui::Color32::GRAY),
                            );
                        }
                    }
                });
            }
            InputMode::FilePath => {
                let entries = get_path_entries(&state.input);
                if key_down {
                    state.navigate_down_bounded(entries.len());
                }
                if key_up {
                    state.navigate_up();
                }
                let selected = state.selected.min(entries.len().saturating_sub(1));

                if key_enter && !entries.is_empty() {
                    let path = entries[selected].full_path.clone();
                    if path.ends_with('\\') {
                        set_path_input(state, prev_input, &path);
                    } else {
                        state.hide();
                        std::thread::spawn(move || {
                            if let Err(e) = crate::commands::list::execute_action(&path) {
                                eprintln!("Action error: {e:#}");
                            }
                        });
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
                        if is_selected && (key_down || key_up) {
                            response.scroll_to_me(Some(egui::Align::Center));
                        }
                    }
                    if entries.is_empty() {
                        ui.label(
                            egui::RichText::new("No entries found").color(egui::Color32::GRAY),
                        );
                    }
                });
            }
            InputMode::Normal => {
                if key_down {
                    state.navigate_down();
                }
                if key_up {
                    state.navigate_up();
                }
                let filtered = state.filtered_items();
                let selected = state.selected;

                if key_enter && !filtered.is_empty() && selected < filtered.len() {
                    execute_and_hide(state);
                    return;
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, item) in filtered.iter().enumerate() {
                        let is_selected = i == selected;

                        let label = match &item.kind {
                            ListItemKind::Command => {
                                let detail = if item.display_detail.len() > 50 {
                                    format!("{}...", &item.display_detail[..47])
                                } else {
                                    item.display_detail.clone()
                                };
                                let text = format!("{}  -  {}", item.key, detail);
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
                        };

                        let response = ui.selectable_label(is_selected, label);

                        if is_selected && (key_down || key_up) {
                            response.scroll_to_me(Some(egui::Align::Center));
                        }
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
}

impl LauncherApp {
    pub fn new(state: WindowState, project_name: String) -> Self {
        Self {
            state,
            project_name,
            prev_input: String::new(),
        }
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        use crate::ui::state::Visibility;

        // Hide on focus loss (focused → unfocused transition only).
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        self.state.hide_on_focus_loss(focused);

        if self.state.visibility == Visibility::Hidden {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));

        render_launcher(
            ctx,
            &mut self.state,
            &self.project_name,
            &mut self.prev_input,
        );
    }
}
