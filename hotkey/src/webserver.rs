//! Tray-managed assist webserver: one cross-platform implementation, with only
//! the platform-specific command construction selected by `cfg`.
//!
//! On Windows the server runs inside WSL (`wsl.exe -- bash -lc`); macOS has no
//! WSL, so it runs natively through the user's shell (`$SHELL -ilc`). The shell
//! must be interactive (`-i`) as well as login (`-l`): version managers like fnm
//! initialise in `.zshrc`, which a non-interactive shell never sources, so a
//! login-only shell resolves the wrong Node and may not find `assist` on PATH.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use std::{env, fs, thread};

const STOP_WAIT: Duration = Duration::from_secs(3);
const STOP_POLL: Duration = Duration::from_millis(50);

/// Port the tray-managed assist webserver listens on (assist --no-open).
const WEBSERVER_PORT: u16 = 3100;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Directory the webserver log lives in: `%LOCALAPPDATA%\project-switch` on
/// Windows, `~/Library/Logs/project-switch` elsewhere.
fn log_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("project-switch"))
    }
    #[cfg(not(windows))]
    {
        dirs::home_dir().map(|h| h.join("Library/Logs/project-switch"))
    }
}

/// Path to the webserver log file, falling back to the current directory if the
/// log directory is unavailable.
fn log_path() -> PathBuf {
    match log_dir() {
        Some(dir) => {
            let _ = fs::create_dir_all(&dir);
            dir.join("assist.log")
        }
        None => PathBuf::from("assist.log"),
    }
}

/// Base `wsl.exe [-d <distro>] --` command (windowless), ready for the Linux
/// program and its arguments to be appended.
#[cfg(windows)]
fn wsl_base(distro: Option<&str>) -> Command {
    use std::os::windows::process::CommandExt;

    let mut cmd = Command::new("wsl.exe");
    if let Some(distro) = distro {
        cmd.args(["-d", distro]);
    }
    cmd.arg("--").creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Build the command that launches the assist webserver, capturing output to the
/// log file. On Windows this runs via a WSL login shell so `assist` resolves on
/// the WSL PATH; elsewhere it runs through the user's login shell.
#[cfg(windows)]
fn launch_command(command: &str, distro: Option<&str>) -> Command {
    let mut cmd = wsl_base(distro);
    cmd.arg("bash").arg("-lc").arg(format!("exec {command}"));
    cmd
}

#[cfg(not(windows))]
fn launch_command(command: &str, _distro: Option<&str>) -> Command {
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let mut cmd = Command::new(shell);
    cmd.arg("-ilc").arg(format!("exec {command}"));
    cmd
}

/// Spawn the assist webserver, capturing stdout/stderr to the log file. Returns
/// the spawned Child handle.
pub fn spawn_webserver(command: &str, distro: Option<&str>) -> std::io::Result<Child> {
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())?;
    let err_file = log_file.try_clone()?;

    let mut cmd = launch_command(command, distro);
    cmd.stdout(Stdio::from(log_file))
        .stderr(Stdio::from(err_file));
    cmd.spawn()
}

/// Build the command that stops the running webserver. On Windows the Linux-side
/// process is killed via a WSL-side `pkill -f` (the Windows handle alone does not
/// reliably terminate the Linux process); elsewhere the process listening on the
/// webserver port is killed.
#[cfg(windows)]
fn stop_command(command: &str, distro: Option<&str>) -> Command {
    let mut cmd = wsl_base(distro);
    cmd.arg("pkill").arg("-f").arg(command);
    cmd
}

// Match the webserver by its listening port, not its command line: a `pkill -f`
// substring match also catches unrelated assist/claude processes whose arguments
// happen to contain the webserver command.
#[cfg(not(windows))]
fn stop_command(_command: &str, _distro: Option<&str>) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(format!(
        "lsof -ti tcp:{WEBSERVER_PORT} -sTCP:LISTEN | while read pid; do kill \"$pid\"; done"
    ));
    cmd
}

#[cfg(windows)]
fn running_command(command: &str, distro: Option<&str>) -> Command {
    let mut cmd = wsl_base(distro);
    cmd.arg("pgrep").arg("-f").arg(command);
    cmd
}

#[cfg(not(windows))]
fn running_command(_command: &str, _distro: Option<&str>) -> Command {
    let mut cmd = Command::new("lsof");
    cmd.arg("-ti")
        .arg(format!("tcp:{WEBSERVER_PORT}"))
        .arg("-sTCP:LISTEN");
    cmd
}

fn webserver_running(command: &str, distro: Option<&str>) -> bool {
    running_command(command, distro)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn wait_until_stopped(command: &str, distro: Option<&str>) {
    let mut waited = Duration::ZERO;
    while waited < STOP_WAIT {
        if !webserver_running(command, distro) {
            return;
        }
        thread::sleep(STOP_POLL);
        waited += STOP_POLL;
    }
}

/// Stop the assist webserver: kill the server process by command match, then reap
/// the spawned Child handle best-effort.
pub fn stop_webserver(child: Option<Child>, command: &str, distro: Option<&str>) {
    let _ = stop_command(command, distro)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if let Some(mut child) = child {
        let _ = child.kill();
        let _ = child.wait();
    }

    wait_until_stopped(command, distro);
}

/// Open the webserver URL in the system's default web browser.
pub fn open_webserver_url() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("cmd")
            .args(["/c", "start", "", &format!("http://localhost:{WEBSERVER_PORT}")])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("open")
            .arg(format!("http://localhost:{WEBSERVER_PORT}"))
            .spawn();
    }
}

/// Open a terminal live-tailing the webserver log file.
pub fn launch_log_tail() {
    let log = log_path();

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
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
    #[cfg(not(windows))]
    {
        let script = format!(
            "tell application \"Terminal\"\nactivate\ndo script \"tail -n 50 -f '{}'\"\nend tell",
            log.display()
        );
        let _ = Command::new("osascript").arg("-e").arg(script).spawn();
    }
}
