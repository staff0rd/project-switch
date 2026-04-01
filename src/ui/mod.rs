//! egui/eframe windowed launcher — text input, filtered list, keyboard nav.

pub mod state;
pub mod window;

pub use state::WindowState;
pub use window::LauncherApp;

pub const WINDOW_SIZE: [f32; 2] = [700.0, 500.0];

/// Standard eframe options for the launcher window.
pub fn launcher_options(visible: bool, monitor: Option<u32>) -> eframe::NativeOptions {
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size(WINDOW_SIZE)
        .with_decorations(false)
        .with_always_on_top()
        .with_visible(visible);

    // When targeting a specific monitor, start the window off-screen so it
    // doesn't flash on the primary display; LauncherApp repositions on frame 1.
    if monitor.is_some() {
        viewport = viewport.with_position(eframe::egui::pos2(-32000.0, -32000.0));
    }

    eframe::NativeOptions {
        centered: visible && monitor.is_none(),
        viewport,
        #[cfg(target_os = "macos")]
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            builder.with_activation_policy(ActivationPolicy::Accessory);
        })),
        ..Default::default()
    }
}

/// Return the physical-pixel rect `[left, top, width, height, dpi]` for the
/// Nth monitor (1-based, sorted left-to-right).  Must be called **after**
/// eframe has initialised (i.e. inside `App::update`) so the process is
/// DPI-aware and coordinates are in true physical pixels.
#[cfg(windows)]
pub fn monitor_physical_rect(n: u32) -> Option<[i32; 5]> {
    use std::mem;
    use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

    type Entry = [i32; 5];

    unsafe extern "system" fn callback(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let monitors = &mut *(data.0 as *mut Vec<Entry>);
        let mut info = MONITORINFO {
            cbSize: mem::size_of::<MONITORINFO>() as u32,
            ..mem::zeroed()
        };
        if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
            let rc = info.rcMonitor;
            let mut dpi_x: u32 = 96;
            let mut dpi_y: u32 = 96;
            let _ = GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
            monitors.push([
                rc.left,
                rc.top,
                rc.right - rc.left,
                rc.bottom - rc.top,
                dpi_x as i32,
            ]);
        }
        BOOL(1)
    }

    let mut monitors: Vec<Entry> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(callback),
            LPARAM(&mut monitors as *mut _ as isize),
        );
    }
    monitors.sort_by_key(|m| m[0]);
    monitors.get((n.saturating_sub(1)) as usize).copied()
}

#[cfg(not(windows))]
pub fn monitor_physical_rect(_n: u32) -> Option<[i32; 5]> {
    None
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
