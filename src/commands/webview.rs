//! Borderless WebView2 window for displaying a local web app.
//!
//! Runs as its own process (the hidden `webview <url>` subcommand) so its event
//! loop never collides with the egui launcher.
//!
//! The window has no OS decorations and no host-drawn chrome — the WebView2
//! child fills it edge to edge. A child window swallows every mouse message over
//! the pixels it covers, so the host can never receive `WM_NCHITTEST` there.
//! Instead, drag and resize gestures are detected in JavaScript inside the page
//! (an injected script) and forwarded over wry's IPC channel; the host then
//! drives the move/resize through tao's `drag_window` / `drag_resize_window`.

#[cfg(windows)]
use anyhow::Context;
use anyhow::Result;
#[cfg(windows)]
use std::path::PathBuf;

/// Title (and identifier) of the single reusable webview window. Used both when
/// creating the window and, later, when locating an existing one to foreground.
#[cfg(windows)]
pub const WEBVIEW_WINDOW_TITLE: &str = "project-switch-webview";

/// Default window size in logical pixels, used when no monitor area constraint
/// shrinks it.
#[cfg(windows)]
const DEFAULT_LOGICAL_SIZE: [f64; 2] = [1280.0, 832.0];

/// Injected into the page to turn the top strip into a drag handle and the
/// window edges into resize handles, since the WebView2 child swallows the
/// mouse messages the host would otherwise hit-test. Gestures are forwarded to
/// the host over IPC (`drag` / `resize:<Direction>`).
///
/// `DRAG_H` / `EDGE` are CSS pixels (matching `clientX`/`clientY`), so they need
/// no DPI scaling. The drag handle is skipped over interactive elements so the
/// app's own header controls still work.
#[cfg(windows)]
const GESTURE_SCRIPT: &str = r#"
;(function () {
  const DRAG_H = 64;
  const EDGE = 6;
  const interactive = (el) => {
    for (; el && el !== document.documentElement; el = el.parentElement) {
      const t = el.tagName;
      if (t === 'A' || t === 'BUTTON' || t === 'INPUT' || t === 'TEXTAREA' ||
          t === 'SELECT' || el.isContentEditable) return true;
    }
    return false;
  };
  const edgeDir = (e) => {
    const w = window.innerWidth, h = window.innerHeight;
    const l = e.clientX <= EDGE, r = e.clientX >= w - EDGE;
    const t = e.clientY <= EDGE, b = e.clientY >= h - EDGE;
    if (t && l) return 'NorthWest';
    if (t && r) return 'NorthEast';
    if (b && l) return 'SouthWest';
    if (b && r) return 'SouthEast';
    if (l) return 'West';
    if (r) return 'East';
    if (t) return 'North';
    if (b) return 'South';
    return null;
  };
  const cursors = {
    North: 'ns-resize', South: 'ns-resize', East: 'ew-resize', West: 'ew-resize',
    NorthEast: 'nesw-resize', SouthWest: 'nesw-resize',
    NorthWest: 'nwse-resize', SouthEast: 'nwse-resize',
  };
  window.addEventListener('mousemove', (e) => {
    const d = edgeDir(e);
    document.documentElement.style.cursor = d ? cursors[d] : '';
  }, true);
  window.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    const d = edgeDir(e);
    if (d) { e.preventDefault(); window.ipc.postMessage('resize:' + d); return; }
    if (e.clientY <= DRAG_H && !interactive(e.target)) window.ipc.postMessage('drag');
  }, true);
})();
"#;

/// IPC messages from the page, mapped to window operations on the main thread.
#[cfg(windows)]
enum UserEvent {
    Drag,
    Resize(tao::window::ResizeDirection),
}

#[cfg(windows)]
fn parse_resize(dir: &str) -> Option<tao::window::ResizeDirection> {
    use tao::window::ResizeDirection::*;
    Some(match dir {
        "North" => North,
        "South" => South,
        "East" => East,
        "West" => West,
        "NorthEast" => NorthEast,
        "NorthWest" => NorthWest,
        "SouthEast" => SouthEast,
        "SouthWest" => SouthWest,
        _ => return None,
    })
}

/// Last known window placement, persisted across runs so the window reopens
/// where it was closed. Stored as the physical-pixel outer rect (position and
/// outer size) so it can be reapplied verbatim with `SetWindowPos`.
#[cfg(windows)]
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
struct SavedGeometry {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[cfg(windows)]
fn geometry_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".project-switch-webview.yml"))
}

#[cfg(windows)]
fn load_geometry() -> Option<SavedGeometry> {
    let contents = std::fs::read_to_string(geometry_path()?).ok()?;
    serde_yaml::from_str(&contents).ok()
}

#[cfg(windows)]
fn save_geometry(window: &tao::window::Window) {
    let Some(path) = geometry_path() else { return };
    let Ok(pos) = window.outer_position() else {
        return;
    };
    let size = window.outer_size();
    let geo = SavedGeometry {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
    };
    if let Ok(yaml) = serde_yaml::to_string(&geo) {
        let _ = std::fs::write(path, yaml);
    }
}

/// Force the window to an exact physical-pixel rect. Used instead of tao's
/// builder position/size, which mis-place the window across monitors with
/// differing DPI (it builds on the primary monitor, then a DPI change shifts
/// it). `SetWindowPos` takes physical screen coordinates directly.
#[cfg(windows)]
fn apply_geometry(hwnd: windows::Win32::Foundation::HWND, g: SavedGeometry) {
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER};

    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            g.x,
            g.y,
            g.width as i32,
            g.height as i32,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// True when the saved window centre falls inside some connected monitor, so a
/// since-disconnected display doesn't reopen the window off-screen.
#[cfg(windows)]
fn geometry_visible<T>(g: &SavedGeometry, event_loop: &tao::event_loop::EventLoop<T>) -> bool {
    let cx = g.x + g.width as i32 / 2;
    let cy = g.y + g.height as i32 / 2;
    event_loop.available_monitors().any(|m| {
        let p = m.position();
        let s = m.size();
        cx >= p.x && cx < p.x + s.width as i32 && cy >= p.y && cy < p.y + s.height as i32
    })
}

#[cfg(windows)]
pub fn execute(url: &str, monitor: Option<u32>) -> Result<()> {
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::platform::windows::WindowExtWindows;
    use tao::window::WindowBuilder;
    use windows::Win32::Foundation::HWND;
    use wry::WebViewBuilder;

    use tao::dpi::{PhysicalPosition, PhysicalSize};

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // A remembered placement wins over the computed default — it's the spot the
    // user last left the window — unless its monitor is gone.
    let saved = load_geometry().filter(|g| geometry_visible(g, &event_loop));

    // Seed the builder with the saved rect (or computed default) so the window
    // is created on roughly the right monitor; the exact placement is forced
    // below with SetWindowPos once the HWND exists.
    let (size, position) = match saved {
        Some(g) => (
            PhysicalSize::new(g.width, g.height),
            Some(PhysicalPosition::new(g.x, g.y)),
        ),
        None => window_geometry(&event_loop, monitor),
    };
    let mut builder = WindowBuilder::new()
        .with_title(WEBVIEW_WINDOW_TITLE)
        .with_decorations(false)
        .with_resizable(true)
        .with_inner_size(size);
    if let Some(pos) = position {
        builder = builder.with_position(pos);
    }
    let window = builder.build(&event_loop)?;

    let hwnd = HWND(window.hwnd() as *mut core::ffi::c_void);
    enable_resize_frame(hwnd);
    if let Some(g) = saved {
        apply_geometry(hwnd, g);
    }

    let proxy = event_loop.create_proxy();

    // The webview navigates asynchronously; an unreachable URL (e.g. the local
    // server isn't up yet) just lands WebView2 on its own error page rather than
    // failing the build, so the window stays alive and reloads cleanly later.
    //
    // `new_as_child` (not `new`) keeps us in control of the child bounds so we
    // can keep it filling the host on resize.
    let webview = WebViewBuilder::new_as_child(&window)
        .with_url(url)
        .with_bounds(fill_bounds(window.inner_size()))
        .with_initialization_script(GESTURE_SCRIPT)
        .with_ipc_handler(move |req| {
            let body = req.body().as_str();
            let event = if body == "drag" {
                Some(UserEvent::Drag)
            } else if let Some(dir) = body.strip_prefix("resize:") {
                parse_resize(dir).map(UserEvent::Resize)
            } else {
                None
            };
            if let Some(event) = event {
                let _ = proxy.send_event(event);
            }
        })
        .build()
        .context("Failed to create WebView2 window")?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(UserEvent::Drag) => {
                let _ = window.drag_window();
            }
            Event::UserEvent(UserEvent::Resize(dir)) => {
                let _ = window.drag_resize_window(dir);
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    save_geometry(&window);
                    *control_flow = ControlFlow::Exit;
                }
                // Persist on every move/resize so the latest placement is always
                // on disk — the ALT+SPACE hotkey can kill this process outright,
                // which never delivers CloseRequested.
                WindowEvent::Moved(_) => save_geometry(&window),
                WindowEvent::Resized(new_size) => {
                    let _ = webview.set_bounds(fill_bounds(new_size));
                    save_geometry(&window);
                }
                _ => {}
            },
            _ => {}
        }
    });
}

/// Bounds that make the WebView2 child fill the host's entire client area.
#[cfg(windows)]
fn fill_bounds(size: tao::dpi::PhysicalSize<u32>) -> wry::Rect {
    use wry::dpi::{PhysicalPosition, PhysicalSize, Position, Size};

    wry::Rect {
        position: Position::Physical(PhysicalPosition::new(0, 0)),
        size: Size::Physical(PhysicalSize::new(size.width, size.height)),
    }
}

/// Compute a centered window size and position for the target monitor (1-based,
/// ordered left-to-right to match the launcher). Falls back to the primary
/// monitor, and finally to a bare size with OS-chosen placement.
#[cfg(windows)]
fn window_geometry<T>(
    event_loop: &tao::event_loop::EventLoop<T>,
    monitor: Option<u32>,
) -> (
    tao::dpi::PhysicalSize<u32>,
    Option<tao::dpi::PhysicalPosition<i32>>,
) {
    use tao::dpi::{PhysicalPosition, PhysicalSize};

    let handle = match monitor {
        Some(n) => {
            let mut monitors: Vec<_> = event_loop.available_monitors().collect();
            monitors.sort_by_key(|m| m.position().x);
            monitors
                .into_iter()
                .nth(n.saturating_sub(1) as usize)
                .or_else(|| event_loop.primary_monitor())
        }
        None => event_loop.primary_monitor(),
    };

    match handle {
        Some(m) => {
            let scale = m.scale_factor();
            let pos = m.position();
            let area = m.size();
            let w = ((DEFAULT_LOGICAL_SIZE[0] * scale) as u32).min(area.width);
            let h = ((DEFAULT_LOGICAL_SIZE[1] * scale) as u32).min(area.height);
            let x = pos.x + (area.width.saturating_sub(w) / 2) as i32;
            let y = pos.y + (area.height.saturating_sub(h) / 2) as i32;
            (PhysicalSize::new(w, h), Some(PhysicalPosition::new(x, y)))
        }
        None => (
            PhysicalSize::new(
                DEFAULT_LOGICAL_SIZE[0] as u32,
                DEFAULT_LOGICAL_SIZE[1] as u32,
            ),
            None,
        ),
    }
}

/// Restore the resize-capable window styles that `decorations(false)` drops, so
/// tao's `drag_resize_window` can drive a native edge resize. No caption is
/// added, so the window stays chromeless.
#[cfg(windows)]
fn enable_resize_frame(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED,
        SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_MAXIMIZEBOX, WS_THICKFRAME,
    };

    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_STYLE,
            style | WS_THICKFRAME.0 as isize | WS_MAXIMIZEBOX.0 as isize,
        );
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        );
    }
}

/// Bring the single webview window to the front if it already exists, otherwise
/// spawn a fresh `project-switch webview <url>` process. Guarantees only one
/// webview window ever exists.
#[cfg(windows)]
pub fn summon_or_open(url: &str, monitor: Option<u32>) -> Result<()> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
    };

    let title = HSTRING::from(WEBVIEW_WINDOW_TITLE);
    let hwnd = unsafe { FindWindowW(PCWSTR::null(), &title) };

    if let Ok(hwnd) = hwnd {
        if !hwnd.is_invalid() {
            unsafe {
                let restore = if IsIconic(hwnd).as_bool() {
                    SW_RESTORE
                } else {
                    SW_SHOW
                };
                let _ = ShowWindow(hwnd, restore);
                let _ = SetForegroundWindow(hwnd);
            }
            return Ok(());
        }
    }

    spawn_window(url, monitor)
}

#[cfg(windows)]
fn spawn_window(url: &str, monitor: Option<u32>) -> Result<()> {
    use std::os::windows::process::CommandExt;
    use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let exe = std::env::current_exe().context("Unable to determine current executable path")?;
    let mut command = std::process::Command::new(exe);
    command.args(["webview", url]);
    if let Some(n) = monitor {
        command.args(["--monitor", &n.to_string()]);
    }
    let child = command
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .context("Failed to spawn webview process")?;

    // Grant the child permission to foreground its window (see hotkey daemon).
    unsafe {
        let _ = AllowSetForegroundWindow(child.id());
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn execute(_url: &str, _monitor: Option<u32>) -> Result<()> {
    anyhow::bail!("The 'webview' subcommand is only supported on Windows")
}

#[cfg(not(windows))]
pub fn summon_or_open(_url: &str, _monitor: Option<u32>) -> Result<()> {
    anyhow::bail!("The webview window is only supported on Windows")
}
