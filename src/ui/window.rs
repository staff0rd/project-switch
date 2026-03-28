//! egui launcher window — renders the text input and filtered list.

use crate::launcher::ListItemKind;
use crate::ui::state::{Visibility, WindowState};
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
            // Draggable title bar area
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

            // Auto-focus input on first frame only
            if !input_response.has_focus() {
                input_response.request_focus();
            }

            // Detect input changes (egui mutates input directly via TextEdit)
            if self.state.input != self.prev_input {
                self.prev_input = self.state.input.clone();
                let new_input = self.state.input.clone();
                self.state.set_input(new_input);
            }

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

            // Get filtered items for display
            let filtered = self.state.filtered_items();
            let selected = self.state.selected;

            if key_enter && !filtered.is_empty() && selected < filtered.len() {
                // TODO: Phase 3 will implement action execution
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

                    // Scroll selected item into view only when navigating
                    if is_selected && (key_down || key_up) {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                }
            });
        });
    }
}
