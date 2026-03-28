//! egui launcher window — renders the text input and filtered list.

use crate::launcher::{get_path_entries, ListItemKind};
use crate::ui::state::{InputMode, Visibility, WindowState};
use eframe::egui;

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

    fn execute_current(&mut self) {
        let input = self.state.input.clone();

        // Calculator mode — nothing to execute
        if input.starts_with('=') {
            return;
        }

        self.state.hide();

        // Run action in background so the window closes immediately
        std::thread::spawn(move || {
            if let Err(e) = crate::commands::list::execute_action(&input) {
                eprintln!("Action error: {e:#}");
            }
        });
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.state.visibility == Visibility::Hidden {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));

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
                &self.project_name,
                egui::FontId::proportional(16.0),
                egui::Color32::GRAY,
            );

            // Text input
            let input_response = ui.add(
                egui::TextEdit::singleline(&mut self.state.input)
                    .hint_text("Type to filter...")
                    .desired_width(f32::INFINITY),
            );
            if !input_response.has_focus() {
                input_response.request_focus();
            }

            // Detect input changes
            if self.state.input != self.prev_input {
                self.prev_input = self.state.input.clone();
                let new_input = self.state.input.clone();
                self.state.set_input(new_input);
            }

            // Keyboard
            let key_down = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));
            let key_up = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
            let key_escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
            let key_enter = ui.input(|i| i.key_pressed(egui::Key::Enter));

            if key_down {
                self.state.navigate_down();
            }
            if key_up {
                self.state.navigate_up();
            }
            if key_escape {
                self.state.hide();
                return;
            }

            ui.separator();

            // Render based on input mode
            match self.state.input_mode() {
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
                    let entries = get_path_entries(&self.state.input);

                    if key_enter {
                        self.execute_current();
                        return;
                    }

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for entry in &entries {
                            let label = if entry.is_dir {
                                egui::RichText::new(&entry.full_path)
                                    .strong()
                                    .color(egui::Color32::from_rgb(100, 180, 255))
                            } else {
                                egui::RichText::new(&entry.full_path)
                            };
                            if ui.selectable_label(false, label).clicked() {
                                self.state.input = entry.full_path.clone();
                                self.prev_input = self.state.input.clone();
                                let new_input = self.state.input.clone();
                                self.state.set_input(new_input);
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
                    let filtered = self.state.filtered_items();
                    let selected = self.state.selected;

                    if key_enter && !filtered.is_empty() && selected < filtered.len() {
                        self.execute_current();
                        return;
                    }

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (i, item) in filtered.iter().enumerate() {
                            let is_selected = i == selected;

                            let text = match &item.kind {
                                ListItemKind::Command => {
                                    let detail = if item.display_detail.len() > 50 {
                                        format!("{}...", &item.display_detail[..47])
                                    } else {
                                        item.display_detail.clone()
                                    };
                                    format!("{}  -  {}", item.key, detail)
                                }
                                ListItemKind::Shortcut { .. } => {
                                    format!("[app] {}", item.key)
                                }
                            };

                            let label = if is_selected {
                                egui::RichText::new(&text).strong()
                            } else {
                                egui::RichText::new(&text)
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
}
