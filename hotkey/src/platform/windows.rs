use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, fs};

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn binary_name() -> &'static str {
    "project-switch.exe"
}

fn local_dir() -> Option<PathBuf> {
    env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("project-switch"))
}

/// If running from the build output (not LOCALAPPDATA), copy both exes to
/// the per-user local directory and relaunch from there. Returns true if
/// the caller should exit (trampoline fired).
pub fn trampoline_if_needed() -> bool {
    let exe_path = match env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let exe_dir = match exe_path.parent() {
        Some(d) => d,
        None => return false,
    };
    let dest = match local_dir() {
        Some(d) => d,
        None => return false,
    };

    // Already running from the local directory — nothing to do
    if exe_dir.starts_with(&dest) {
        return false;
    }

    // Copy all exes (and runtime DLLs, e.g. WebView2Loader.dll which
    // project-switch.exe links at load time) from source to local dir
    let _ = fs::create_dir_all(&dest);
    if let Ok(entries) = fs::read_dir(exe_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str());
            if matches!(ext, Some("exe") | Some("dll")) {
                let target = dest.join(entry.file_name());
                let _ = fs::copy(&path, &target);
            }
        }
    }

    // Relaunch from local copy
    let local_exe = dest.join("project-switch-hotkey.exe");
    if local_exe.exists() {
        let _ = Command::new(&local_exe)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }

    true
}

pub fn kill_existing_hotkey_instances() {
    let our_pid = std::process::id().to_string();
    let _ = Command::new("wmic")
        .args([
            "process",
            "where",
            &format!("Name='project-switch-hotkey.exe' and ProcessId!='{our_pid}'"),
            "call",
            "terminate",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

pub fn launch_project_switch(project_switch: &Path, monitor: u32) {
    use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;

    let monitor_arg = monitor.to_string();

    // Launch the new instance first so the window appears immediately.
    let child = match Command::new(project_switch)
        .args(["list", "--gui", "--monitor", &monitor_arg])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            eprintln!("Failed to launch project-switch: {e}");
            return;
        }
    };

    let new_pid = child.id();

    // Grant the child process permission to call SetForegroundWindow.
    // Without this, Windows silently ignores the request ~50% of the
    // time because only the current foreground process (or one it
    // explicitly authorises) is allowed to steal focus.
    unsafe {
        let _ = AllowSetForegroundWindow(new_pid);
    }

    // Kill old instances (non-blocking), excluding the one we just spawned and
    // the long-lived webview window (a 'project-switch.exe webview <url>'
    // process), which must survive so re-triggering summons it rather than
    // spawning a duplicate.
    let _ = Command::new("wmic")
        .args([
            "process",
            "where",
            &format!(
                "Name='project-switch.exe' and ProcessId!='{new_pid}' and not CommandLine like '%webview%'"
            ),
            "call",
            "terminate",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}
