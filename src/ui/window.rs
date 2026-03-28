//! egui launcher window — renders the text input and filtered list.

use crate::launcher::ListItemKind;
use crate::ui::state::{Visibility, WindowState};
use eframe::egui;

#[allow(dead_code)]
pub struct LauncherApp {
    state: WindowState,
    project_name: String,
}

#[allow(dead_code)]
impl LauncherApp {
    pub fn new(state: WindowState, project_name: String) -> Self {
        Self {
            state,
            project_name,
        }
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle visibility
        if self.state.visibility == Visibility::Hidden {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));

        egui::CentralPanel::default().show(ctx, |ui| {
            // Project context label
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&self.project_name)
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });

            // Text input
            let input_response = ui.add(
                egui::TextEdit::singleline(&mut self.state.input)
                    .hint_text("Type to filter...")
                    .desired_width(f32::INFINITY),
            );

            // Auto-focus input on first frame
            if input_response.gained_focus() || self.state.input.is_empty() {
                input_response.request_focus();
            }

            // Handle input change (must detect manually since we mutate input directly)
            let prev_input = self.state.input.clone();

            // Keyboard navigation
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

            // Update filtered count if input changed
            if self.state.input != prev_input {
                let new_input = self.state.input.clone();
                self.state.set_input(new_input);
            }

            // Get filtered items for display
            let filtered = self.state.filtered_items();
            let selected = self.state.selected;

            if key_enter && !filtered.is_empty() && selected < filtered.len() {
                // TODO: Phase 3 will implement action execution
                // For now, just hide the window
                self.state.hide();
                return;
            }

            ui.separator();

            // Scrollable list
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
                            format!("{} → {}", item.key, detail)
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

                    // Scroll selected item into view
                    if is_selected {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                }
            });
        });
    }
}
