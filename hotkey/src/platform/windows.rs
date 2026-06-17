use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::{env, fs};

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// URL the tray-managed assist webserver listens on (assist --no-open, port 3100).
const WEBSERVER_URL: &str = "http://localhost:3100";

pub fn binary_name() -> &'static str {
    "project-switch.exe"
}

fn local_dir() -> Option<PathBuf> {
    env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("project-switch"))
}

/// Path to the webserver log file under `%LOCALAPPDATA%\project-switch\`,
/// falling back to the current directory if the local dir is unavailable.
fn log_path() -> PathBuf {
    match local_dir() {
        Some(dir) => {
            let _ = fs::create_dir_all(&dir);
            dir.join("assist.log")
        }
        None => PathBuf::from("assist.log"),
    }
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

/// Spawn the assist webserver inside WSL, capturing output to
/// `%LOCALAPPDATA%\project-switch\assist.log`. The command always runs via a
/// WSL login shell so `assist` resolves on the WSL PATH; a Windows binary is
/// never used. Returns the `wsl.exe` Child handle.
pub fn spawn_webserver(command: &str, distro: Option<&str>) -> std::io::Result<Child> {
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())?;
    let err_file = log_file.try_clone()?;

    let mut cmd = Command::new("wsl.exe");
    if let Some(distro) = distro {
        cmd.args(["-d", distro]);
    }
    cmd.arg("--")
        .arg("bash")
        .arg("-lc")
        .arg(format!("exec {command}"))
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(err_file))
        .creation_flags(CREATE_NO_WINDOW);

    cmd.spawn()
}

/// Stop the assist webserver. The Linux-side process is killed via a WSL-side
/// `pkill -f` (the Windows `wsl.exe` handle alone does not reliably terminate
/// the Linux process across the WSL boundary). The Child handle is then killed
/// best-effort to clean up the Windows-side process.
pub fn stop_webserver(child: Option<Child>, command: &str, distro: Option<&str>) {
    let mut cmd = Command::new("wsl.exe");
    if let Some(distro) = distro {
        cmd.args(["-d", distro]);
    }
    let _ = cmd
        .arg("--")
        .arg("pkill")
        .arg("-f")
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();

    if let Some(mut child) = child {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Open the webserver URL in the system's default web browser.
pub fn open_webserver_url() {
    let _ = Command::new("cmd")
        .args(["/c", "start", "", WEBSERVER_URL])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

/// Open Windows Terminal live-tailing the webserver log file.
pub fn launch_log_tail() {
    let log = log_path();
    let _ = Command::new("wt.exe")
        .arg("powershell")
        .arg("-NoExit")
        .arg("-Command")
        .arg(format!(
            "Get-Content -LiteralPath '{}' -Wait -Tail 50",
            log.display()
        ))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
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
                "Name='project-switch.exe' and ProcessId!='{new_pid}' and CommandLine not like '%webview%'"
            ),
            "call",
            "terminate",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}
