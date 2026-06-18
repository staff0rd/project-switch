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

#[cfg(any(windows, target_os = "macos"))]
use anyhow::Context;
use anyhow::Result;
#[cfg(any(windows, target_os = "macos"))]
use std::path::PathBuf;

/// Title (and identifier) of the single reusable webview window. Used both when
/// creating the window and, later, when locating an existing one to foreground.
#[cfg(any(windows, target_os = "macos"))]
pub const WEBVIEW_WINDOW_TITLE: &str = "project-switch-webview";

/// Default window size in logical pixels, used when no monitor area constraint
/// shrinks it.
#[cfg(any(windows, target_os = "macos"))]
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

/// Injected when handing a link to the default browser fails. Shows a small,
/// self-dismissing toast in the bottom-right of the current page (it removes any
/// prior toast first, so repeated failures don't stack) and leaves the page
/// untouched otherwise. Wrapped in a try so a hostile page that has redefined
/// document globals can't surface a script error.
#[cfg(windows)]
const TOAST_SCRIPT: &str = r#"
;(function () {
  try {
    var ID = '__ps_browser_toast__';
    var prev = document.getElementById(ID);
    if (prev) prev.remove();
    var t = document.createElement('div');
    t.id = ID;
    t.textContent = 'Couldn’t open the link in your browser.';
    t.style.cssText = 'position:fixed;bottom:16px;right:16px;z-index:2147483647;' +
      'max-width:320px;padding:12px 16px;border-radius:8px;cursor:pointer;' +
      'background:#323232;color:#fff;font:14px system-ui,sans-serif;' +
      'box-shadow:0 4px 12px rgba(0,0,0,.3);';
    t.title = 'Click to dismiss';
    t.addEventListener('click', function () { t.remove(); });
    document.body.appendChild(t);
    setTimeout(function () { t.remove(); }, 6000);
  } catch (e) {}
})();
"#;

/// IPC messages from the page, mapped to window operations on the main thread.
#[cfg(windows)]
enum UserEvent {
    Drag,
    Resize(tao::window::ResizeDirection),
    OpenExternal(String),
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
#[cfg(any(windows, target_os = "macos"))]
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
struct SavedGeometry {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[cfg(any(windows, target_os = "macos"))]
fn geometry_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".project-switch-webview.yml"))
}

#[cfg(any(windows, target_os = "macos"))]
fn load_geometry() -> Option<SavedGeometry> {
    let contents = std::fs::read_to_string(geometry_path()?).ok()?;
    serde_yaml::from_str(&contents).ok()
}

#[cfg(any(windows, target_os = "macos"))]
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
#[cfg(any(windows, target_os = "macos"))]
fn geometry_visible<T>(g: &SavedGeometry, event_loop: &tao::event_loop::EventLoop<T>) -> bool {
    let cx = g.x + g.width as i32 / 2;
    let cy = g.y + g.height as i32 / 2;
    event_loop.available_monitors().any(|m| {
        let p = m.position();
        let s = m.size();
        cx >= p.x && cx < p.x + s.width as i32 && cy >= p.y && cy < p.y + s.height as i32
    })
}

/// A computed window size paired with an optional top-left position.
#[cfg(any(windows, target_os = "macos"))]
type WindowPlacement = (
    tao::dpi::PhysicalSize<u32>,
    Option<tao::dpi::PhysicalPosition<i32>>,
);

/// Seed the window placement from the saved rect (when its monitor is still
/// present) or the computed default, returning it alongside the validated saved
/// geometry — Windows reapplies the latter precisely once the HWND exists; other
/// platforms ignore it.
#[cfg(any(windows, target_os = "macos"))]
fn seed_geometry<T>(
    event_loop: &tao::event_loop::EventLoop<T>,
    monitor: Option<u32>,
) -> (WindowPlacement, Option<SavedGeometry>) {
    use tao::dpi::{PhysicalPosition, PhysicalSize};

    // A remembered placement wins over the computed default — it's the spot the
    // user last left the window — unless its monitor is gone.
    let saved = load_geometry().filter(|g| geometry_visible(g, event_loop));
    let placement = match saved {
        Some(g) => (
            PhysicalSize::new(g.width, g.height),
            Some(PhysicalPosition::new(g.x, g.y)),
        ),
        None => window_geometry(event_loop, monitor),
    };
    (placement, saved)
}

/// Build the "is this URL external?" predicate for a webview showing `url`: an
/// http(s) target whose origin differs from the configured origin is external
/// and handed to the system browser; same-origin and non-http(s) targets (None
/// origin, e.g. about:/error pages) stay inside the webview.
#[cfg(any(windows, target_os = "macos"))]
fn external_predicate(url: &str) -> impl Fn(&str) -> bool + Clone {
    let initial_origin = crate::utils::url::origin_of(url);
    move |target: &str| match crate::utils::url::origin_of(target) {
        Some(origin) => initial_origin.as_deref() != Some(origin.as_str()),
        None => false,
    }
}

/// Build the `project-switch webview <url> [--monitor N]` command that spawns a
/// fresh webview process. Callers apply any platform-specific flags before
/// spawning.
#[cfg(any(windows, target_os = "macos"))]
fn webview_command(url: &str, monitor: Option<u32>) -> Result<std::process::Command> {
    let exe = std::env::current_exe().context("Unable to determine current executable path")?;
    let mut command = std::process::Command::new(exe);
    command.args(["webview", url]);
    if let Some(n) = monitor {
        command.args(["--monitor", &n.to_string()]);
    }
    Ok(command)
}

#[cfg(windows)]
pub fn execute(url: &str, monitor: Option<u32>) -> Result<()> {
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::platform::windows::WindowExtWindows;
    use tao::window::WindowBuilder;
    use windows::Win32::Foundation::HWND;
    use wry::WebViewBuilder;

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // Seed the builder with the saved rect (or computed default) so the window
    // is created on roughly the right monitor; the exact placement is forced
    // below with SetWindowPos once the HWND exists.
    let ((size, position), saved) = seed_geometry(&event_loop, monitor);
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

    let is_external = external_predicate(url);

    // The webview navigates asynchronously; an unreachable URL (e.g. the local
    // server isn't up yet) just lands WebView2 on its own error page rather than
    // failing the build, so the window stays alive and reloads cleanly later.
    //
    // `new_as_child` (not `new`) keeps us in control of the child bounds so we
    // can keep it filling the host on resize.
    let nav_proxy = proxy.clone();
    let nav_is_external = is_external.clone();
    let popup_proxy = proxy.clone();
    let popup_is_external = is_external;

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
        // Cross-origin link clicks: cancel the in-webview navigation (return
        // false) and hand the URL to the default browser instead.
        .with_navigation_handler(move |url| {
            if nav_is_external(&url) {
                let _ = nav_proxy.send_event(UserEvent::OpenExternal(url));
                false
            } else {
                true
            }
        })
        // target=_blank / window.open: suppress the popup (return false) for
        // cross-origin URLs and open them in the default browser instead.
        .with_new_window_req_handler(move |url| {
            if popup_is_external(&url) {
                let _ = popup_proxy.send_event(UserEvent::OpenExternal(url));
                false
            } else {
                true
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
            Event::UserEvent(UserEvent::OpenExternal(url)) => {
                if crate::utils::browser::open_url_in_browser(&url, "default", false).is_err() {
                    let _ = webview.evaluate_script(TOAST_SCRIPT);
                }
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
#[cfg(any(windows, target_os = "macos"))]
fn window_geometry<T>(
    event_loop: &tao::event_loop::EventLoop<T>,
    monitor: Option<u32>,
) -> WindowPlacement {
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

    let child = webview_command(url, monitor)?
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .context("Failed to spawn webview process")?;

    // Grant the child permission to foreground its window (see hotkey daemon).
    unsafe {
        let _ = AllowSetForegroundWindow(child.id());
    }

    Ok(())
}

/// macOS webview window. Unlike the chromeless Windows window (which needs
/// injected drag/resize gestures because the WebView2 child swallows mouse
/// messages), this keeps native decorations, so dragging, resizing and closing
/// come from the OS for free.
#[cfg(target_os = "macos")]
pub fn execute(url: &str, monitor: Option<u32>) -> Result<()> {
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoop};
    use tao::window::WindowBuilder;
    use wry::WebViewBuilder;

    let event_loop = EventLoop::new();

    let ((size, position), _saved) = seed_geometry(&event_loop, monitor);
    let mut builder = WindowBuilder::new()
        .with_title(WEBVIEW_WINDOW_TITLE)
        .with_inner_size(size);
    if let Some(pos) = position {
        builder = builder.with_position(pos);
    }
    let window = builder.build(&event_loop)?;

    let is_external = external_predicate(url);
    let nav_is_external = is_external.clone();
    let popup_is_external = is_external;

    let _webview = WebViewBuilder::new(&window)
        .with_url(url)
        // Cross-origin link clicks: cancel the in-webview navigation (return
        // false) and hand the URL to the default browser instead.
        .with_navigation_handler(move |url| {
            if nav_is_external(&url) {
                let _ = crate::utils::browser::open_url_in_browser(&url, "default", false);
                false
            } else {
                true
            }
        })
        // target=_blank / window.open: suppress the popup (return false) for
        // cross-origin URLs and open them in the default browser instead.
        .with_new_window_req_handler(move |url| {
            if popup_is_external(&url) {
                let _ = crate::utils::browser::open_url_in_browser(&url, "default", false);
                false
            } else {
                true
            }
        })
        .build()
        .context("Failed to create webview window")?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CloseRequested => {
                    save_geometry(&window);
                    *control_flow = ControlFlow::Exit;
                }
                // Persist on every move/resize so the latest placement is always
                // on disk — the hotkey can kill this process outright, which
                // never delivers CloseRequested.
                WindowEvent::Moved(_) | WindowEvent::Resized(_) => save_geometry(&window),
                _ => {}
            }
        }
    });
}

/// Bring the single webview window to the front if its process already exists,
/// otherwise spawn a fresh `project-switch webview <url>` process. Guarantees
/// only one webview window ever exists.
#[cfg(target_os = "macos")]
pub fn summon_or_open(url: &str, monitor: Option<u32>) -> Result<()> {
    if activate_existing_webview() {
        return Ok(());
    }
    spawn_window(url, monitor)
}

/// Find a running `project-switch webview` process and bring it to the front.
/// Returns true when one was found and activated.
#[cfg(target_os = "macos")]
fn activate_existing_webview() -> bool {
    use std::process::Command;

    let our_pid = std::process::id().to_string();
    let Ok(output) = Command::new("pgrep")
        .args(["-f", "project-switch webview"])
        .output()
    else {
        return false;
    };

    // A live webview process means a window already exists: report it found so
    // the caller never spawns a duplicate, and raise it best-effort.
    let pids = String::from_utf8_lossy(&output.stdout);
    let mut found = false;
    for pid in pids.lines() {
        let pid = pid.trim();
        if pid.is_empty() || pid == our_pid {
            continue;
        }
        if let Ok(pid) = pid.parse::<i32>() {
            found = true;
            activate_pid(pid);
        }
    }
    found
}

/// Activate the GUI process with the given pid, foregrounding all its windows.
#[cfg(target_os = "macos")]
fn activate_pid(pid: i32) {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};

    if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
    }
}

#[cfg(target_os = "macos")]
fn spawn_window(url: &str, monitor: Option<u32>) -> Result<()> {
    webview_command(url, monitor)?
        .spawn()
        .context("Failed to spawn webview process")?;
    Ok(())
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn execute(_url: &str, _monitor: Option<u32>) -> Result<()> {
    anyhow::bail!("The 'webview' subcommand is not supported on this platform")
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn summon_or_open(_url: &str, _monitor: Option<u32>) -> Result<()> {
    anyhow::bail!("The webview window is not supported on this platform")
}
