//! egui/eframe windowed launcher — text input, filtered list, keyboard nav.

pub mod state;
pub mod window;

pub use state::WindowState;
pub use window::LauncherApp;

/// Standard eframe options for the launcher window.
pub fn launcher_options(visible: bool, monitor: Option<u32>) -> eframe::NativeOptions {
    let window_size = [700.0_f32, 500.0];
    let position = resolve_monitor_position(monitor, window_size);

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size(window_size)
        .with_decorations(false)
        .with_always_on_top()
        .with_visible(visible);

    if let Some(pos) = position {
        viewport = viewport.with_position(pos);
    }

    eframe::NativeOptions {
        centered: position.is_none(),
        viewport,
        #[cfg(target_os = "macos")]
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            builder.with_activation_policy(ActivationPolicy::Accessory);
        })),
        ..Default::default()
    }
}

#[cfg(windows)]
fn resolve_monitor_position(
    monitor: Option<u32>,
    window_size: [f32; 2],
) -> Option<eframe::egui::Pos2> {
    use std::mem;
    use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

    // left, top, width, height (physical pixels), dpi
    type MonitorEntry = [i32; 5];

    unsafe extern "system" fn callback(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let monitors = &mut *(data.0 as *mut Vec<MonitorEntry>);
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

    let n = monitor?;
    let mut monitors: Vec<MonitorEntry> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(callback),
            LPARAM(&mut monitors as *mut _ as isize),
        );
    }
    // Sort left-to-right so Monitor 1 = leftmost
    monitors.sort_by_key(|m| m[0]);

    let m = monitors.get((n.saturating_sub(1)) as usize)?;
    let scale = m[4] as f32 / 96.0;

    // GetMonitorInfoW returns physical pixel coordinates; eframe expects logical.
    // Convert physical monitor center to logical, then offset by half the logical window size.
    let center_x = m[0] as f32 + m[2] as f32 / 2.0;
    let center_y = m[1] as f32 + m[3] as f32 / 2.0;
    Some(eframe::egui::Pos2::new(
        center_x / scale - window_size[0] / 2.0,
        center_y / scale - window_size[1] / 2.0,
    ))
}

#[cfg(not(windows))]
fn resolve_monitor_position(
    _monitor: Option<u32>,
    _window_size: [f32; 2],
) -> Option<eframe::egui::Pos2> {
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
