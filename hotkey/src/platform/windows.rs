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

    // Copy all exes from source to local dir
    let _ = fs::create_dir_all(&dest);
    if let Ok(entries) = fs::read_dir(exe_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("exe") {
                let target = dest.join(entry.file_name());
                let _ = fs::copy(&path, &target);
            }
        }
    }

    // Relaunch from local copy
    let local_exe = dest.join("project-switch-hotkey.exe");
    if local_exe.exists() {
        let _ = Command::new(&local_exe).spawn();
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

pub fn launch_project_switch(project_switch: &Path) {
    // Kill any existing project-switch.exe instances first
    let _ = Command::new("taskkill")
        .args(["/IM", "project-switch.exe", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();

    // Launch the windowed GUI launcher directly
    match Command::new(project_switch)
        .args(["list", "--gui"])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        Ok(_) => {}
        Err(e) => eprintln!("Failed to launch project-switch: {e}"),
    }
}
