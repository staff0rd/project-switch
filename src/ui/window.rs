//! egui launcher window — renders the text input and filtered list.

use crate::launcher::{get_path_entries, CalcResult, ListItemKind};
use crate::ui::state::{FilteredEntry, InputMode, WindowState};
use eframe::egui;

fn set_path_input(state: &mut WindowState, prev_input: &mut String, path: &str) {
    state.input = path.to_string();
    *prev_input = state.input.clone();
    let new_input = state.input.clone();
    state.set_input(new_input);
}

fn open_path_and_hide(state: &mut WindowState, path: String) {
    state.hide();
    std::thread::spawn(move || {
        if let Err(e) = crate::commands::list::execute_action(&path) {
            eprintln!("Action error: {e:#}");
        }
    });
}

fn execute_and_hide(state: &mut WindowState, action_input: &str) {
    if action_input.starts_with('=') {
        return;
    }
    let input = action_input.to_string();
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
                    execute_and_hide(state, &state.input.clone());
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
    /// Counts frames since creation; used to request OS focus during startup.
    startup_frames: u32,
    /// Receives shortcuts collected on a background thread.
    shortcut_rx: Option<std::sync::mpsc::Receiver<Vec<crate::launcher::ListItem>>>,
    /// Target monitor (1-based) to reposition the window onto after creation.
    monitor: Option<u32>,
}

impl LauncherApp {
    pub fn new(
        state: WindowState,
        project_name: String,
        shortcut_rx: Option<std::sync::mpsc::Receiver<Vec<crate::launcher::ListItem>>>,
        monitor: Option<u32>,
    ) -> Self {
        Self {
            state,
            project_name,
            prev_input: String::new(),
            startup_frames: 0,
            shortcut_rx,
            monitor,
        }
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        use crate::ui::state::Visibility;

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

        // Hide on focus loss (focused → unfocused transition only).
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        self.state.hide_on_focus_loss(focused);

        if self.state.visibility == Visibility::Hidden {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));

        // On the first rendered frame, reposition the window onto the
        // target monitor.  The window starts off-screen (see launcher_options)
        // so this runs before the user ever sees it.
        if self.startup_frames == 1 {
            if let Some(n) = self.monitor {
                if let Some([ml, mt, mw, mh, dpi]) = crate::ui::monitor_physical_rect(n) {
                    let ppp = ctx.pixels_per_point();
                    let target_scale = dpi as f32 / 96.0;
                    let phys_w = crate::ui::WINDOW_SIZE[0] * target_scale;
                    let phys_h = crate::ui::WINDOW_SIZE[1] * target_scale;
                    let tx = ml as f32 + (mw as f32 - phys_w) / 2.0;
                    let ty = mt as f32 + (mh as f32 - phys_h) / 2.0;
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                        tx / ppp,
                        ty / ppp,
                    )));
                }
            }
        }

        // Request OS-level window focus during the first few frames.
        // The hotkey service grants us foreground permission via
        // AllowSetForegroundWindow; this triggers SetForegroundWindow
        // through eframe so the window reliably receives input focus.
        if self.startup_frames < 10 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            self.startup_frames += 1;
        }

        render_launcher(
            ctx,
            &mut self.state,
            &self.project_name,
            &mut self.prev_input,
        );
    }
}
