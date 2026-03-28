//! egui/eframe windowed launcher — text input, filtered list, keyboard nav.

pub mod state;
pub mod window;

pub use state::WindowState;
pub use window::LauncherApp;

/// Standard eframe options for the launcher window.
pub fn launcher_options(visible: bool) -> eframe::NativeOptions {
    eframe::NativeOptions {
        centered: true,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([700.0, 500.0])
            .with_decorations(false)
            .with_always_on_top()
            .with_visible(visible),
        ..Default::default()
    }
}

/// Apply the standard launcher font styles to an egui context.
pub fn apply_launcher_style(ctx: &eframe::egui::Context) {
    let mut style = (*ctx.style()).clone();
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
    ctx.set_style(style);
}
