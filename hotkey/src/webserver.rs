//! Tray-managed assist web servers. Each configured server has a target:
//!
//! - `wsl` (Windows only) runs inside WSL (`wsl.exe -- bash -lc`).
//! - `native` runs on the host: `pwsh` on Windows, the user's shell (`$SHELL -ilc`)
//!   elsewhere. The macOS shell must be interactive (`-i`) as well as login (`-l`):
//!   version managers like fnm initialise in `.zshrc`, which a non-interactive
//!   shell never sources, so a login-only shell resolves the wrong Node and may not
//!   find `assist` on PATH. On Windows, pwsh 7 loads the profile that runs `fnm env`.
//!
//! Every server is checked and stopped by the port it listens on, never by
//! command line: a `pkill -f` substring match also catches unrelated
//! assist/claude processes whose arguments happen to contain the command.

use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use std::{env, fs, thread};

const STOP_WAIT: Duration = Duration::from_secs(3);
const STOP_POLL: Duration = Duration::from_millis(50);

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Wsl,
    Native,
}

/// Where a server's config lives, so toggling writes back to the same entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Legacy,
    List(usize),
}

#[derive(Clone, Debug)]
pub struct Webserver {
    pub name: String,
    pub source: Source,
    pub enabled: bool,
    pub target: Target,
    pub command: String,
    #[cfg_attr(not(windows), allow(dead_code))]
    pub distro: Option<String>,
    pub port: u16,
}

impl Webserver {
    pub fn menu_label(&self) -> String {
        match self.source {
            Source::Legacy => "Webserver".to_string(),
            Source::List(_) => format!("Webserver ({})", self.name),
        }
    }

    // The legacy single server keeps its original log name so existing tails still work.
    fn log_file_name(&self) -> String {
        match self.source {
            Source::Legacy => "assist.log".to_string(),
            Source::List(_) => format!("assist-{}.log", self.name),
        }
    }
}

/// Directory the webserver logs live in: `%LOCALAPPDATA%\project-switch` on
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

/// Path to a server's log file, falling back to the current directory if the
/// log directory is unavailable.
fn log_path(server: &Webserver) -> PathBuf {
    let name = server.log_file_name();
    match log_dir() {
        Some(dir) => {
            let _ = fs::create_dir_all(&dir);
            dir.join(name)
        }
        None => PathBuf::from(name),
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

#[cfg(windows)]
fn wsl_sh(distro: Option<&str>, script: String) -> Command {
    let mut cmd = wsl_base(distro);
    cmd.arg("sh").arg("-c").arg(script);
    cmd
}

#[cfg(windows)]
fn windowless(program: &str) -> Command {
    use std::os::windows::process::CommandExt;

    let mut cmd = Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Build the command that launches a web server. The `PROJECT_SWITCH_WEBSERVER`
/// marker is set on the process env (not the -c string) so it is present while
/// the login profile is sourced, before `command` runs.
fn launch_command(server: &Webserver) -> io::Result<Command> {
    match server.target {
        Target::Wsl => wsl_launch_command(server),
        Target::Native => Ok(native_launch_command(&server.command)),
    }
}

#[cfg(windows)]
fn wsl_launch_command(server: &Webserver) -> io::Result<Command> {
    let mut cmd = wsl_base(server.distro.as_deref());
    // WSLENV carries the marker into WSL; append to any existing WSLENV so we
    // don't clobber other shared vars.
    cmd.env("PROJECT_SWITCH_WEBSERVER", "1");
    let wslenv = match env::var("WSLENV") {
        Ok(existing) if !existing.is_empty() => format!("{existing}:PROJECT_SWITCH_WEBSERVER"),
        _ => "PROJECT_SWITCH_WEBSERVER".to_string(),
    };
    cmd.env("WSLENV", wslenv);
    cmd.arg("bash")
        .arg("-lc")
        .arg(format!("exec {}", server.command));
    Ok(cmd)
}

#[cfg(not(windows))]
fn wsl_launch_command(_server: &Webserver) -> io::Result<Command> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "the wsl webserver target is only available on Windows",
    ))
}

#[cfg(windows)]
fn native_launch_command(command: &str) -> Command {
    let mut cmd = windowless("pwsh.exe");
    cmd.env("PROJECT_SWITCH_WEBSERVER", "1");
    cmd.args(["-NoLogo", "-NonInteractive", "-Command", command]);
    cmd
}

#[cfg(not(windows))]
fn native_launch_command(command: &str) -> Command {
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let mut cmd = Command::new(shell);
    cmd.env("PROJECT_SWITCH_WEBSERVER", "1");
    cmd.arg("-ilc").arg(format!("exec {command}"));
    cmd
}

/// Spawn a web server, capturing stdout/stderr to its log file.
pub fn spawn_webserver(server: &Webserver) -> io::Result<Child> {
    let mut cmd = launch_command(server)?;
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path(server))?;
    let err_file = log_file.try_clone()?;
    cmd.stdout(Stdio::from(log_file))
        .stderr(Stdio::from(err_file));
    cmd.spawn()
}

/// Local addresses in `netstat -ano` output that listen on `port`, as owning
/// pids. Listening rows are matched by their `:0` foreign address rather than
/// the state column, which Windows localises.
#[cfg(any(windows, test))]
fn parse_netstat_listen_pids(output: &str, port: u16) -> Vec<u32> {
    let suffix = format!(":{port}");
    let mut pids: Vec<u32> = output
        .lines()
        .filter_map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 5 || !cols[0].eq_ignore_ascii_case("TCP") {
                return None;
            }
            let listening = cols[2].ends_with(":0");
            if !listening || !cols[1].ends_with(&suffix) {
                return None;
            }
            cols[cols.len() - 1].parse().ok()
        })
        .collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

#[cfg(windows)]
fn native_listen_pids(port: u16) -> Vec<u32> {
    windowless("netstat")
        .args(["-ano", "-p", "TCP"])
        .stderr(Stdio::null())
        .output()
        .map(|out| parse_netstat_listen_pids(&String::from_utf8_lossy(&out.stdout), port))
        .unwrap_or_default()
}

#[cfg(windows)]
fn native_running(port: u16) -> bool {
    !native_listen_pids(port).is_empty()
}

// No /T: the web server's detached sessions daemon is its child and must survive.
#[cfg(windows)]
fn native_stop(port: u16) {
    for pid in native_listen_pids(port) {
        let _ = windowless("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(not(windows))]
fn native_running(port: u16) -> bool {
    Command::new("lsof")
        .arg("-ti")
        .arg(format!("tcp:{port}"))
        .arg("-sTCP:LISTEN")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn native_stop(port: u16) {
    let _ = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "lsof -ti tcp:{port} -sTCP:LISTEN | while read pid; do kill \"$pid\"; done"
        ))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(windows)]
fn wsl_running(server: &Webserver) -> bool {
    wsl_sh(
        server.distro.as_deref(),
        format!("ss -ltnH 'sport = :{}' | grep -q .", server.port),
    )
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()
    .map(|s| s.success())
    .unwrap_or(false)
}

#[cfg(windows)]
fn wsl_stop(server: &Webserver) {
    let _ = wsl_sh(
        server.distro.as_deref(),
        format!(
            "ss -ltnpH 'sport = :{}' | grep -o 'pid=[0-9]*' | cut -d= -f2 | sort -u | xargs -r kill",
            server.port
        ),
    )
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status();
}

#[cfg(not(windows))]
fn wsl_running(_server: &Webserver) -> bool {
    false
}

#[cfg(not(windows))]
fn wsl_stop(_server: &Webserver) {}

fn webserver_running(server: &Webserver) -> bool {
    match server.target {
        Target::Wsl => wsl_running(server),
        Target::Native => native_running(server.port),
    }
}

fn wait_until_stopped(server: &Webserver) {
    let mut waited = Duration::ZERO;
    while waited < STOP_WAIT {
        if !webserver_running(server) {
            return;
        }
        thread::sleep(STOP_POLL);
        waited += STOP_POLL;
    }
}

/// Stop a web server: kill whatever listens on its port, then reap the spawned
/// Child handle best-effort.
pub fn stop_webserver(child: Option<Child>, server: &Webserver) {
    match server.target {
        Target::Wsl => wsl_stop(server),
        Target::Native => native_stop(server.port),
    }

    if let Some(mut child) = child {
        let _ = child.kill();
        let _ = child.wait();
    }

    wait_until_stopped(server);
}

/// Open a web server's URL in the system's default web browser.
pub fn open_webserver_url(port: u16) {
    #[cfg(windows)]
    {
        let _ = windowless("cmd")
            .args(["/c", "start", "", &format!("http://localhost:{port}")])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("open")
            .arg(format!("http://localhost:{port}"))
            .spawn();
    }
}

/// Open a terminal live-tailing a web server's log file.
pub fn launch_log_tail(server: &Webserver) {
    let log = log_path(server);

    #[cfg(windows)]
    {
        let _ = windowless("wt.exe")
            .arg("powershell")
            .arg("-NoExit")
            .arg("-Command")
            .arg(format!(
                "Get-Content -LiteralPath '{}' -Wait -Tail 50",
                log.display()
            ))
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

#[cfg(test)]
mod tests {
    use super::parse_netstat_listen_pids;

    const NETSTAT: &str = "
Active Connections

  Proto  Local Address          Foreign Address        State           PID
  TCP    0.0.0.0:3101           0.0.0.0:0              LISTENING       4242
  TCP    127.0.0.1:3101         127.0.0.1:50000        ESTABLISHED     4242
  TCP    127.0.0.1:50000        127.0.0.1:3101         ESTABLISHED     999
  TCP    0.0.0.0:31010          0.0.0.0:0              LISTENING       7
  TCP    [::]:3101              [::]:0                 ABHÖREN         4242
";

    #[test]
    fn finds_only_listeners_on_the_exact_port() {
        assert_eq!(parse_netstat_listen_pids(NETSTAT, 3101), vec![4242]);
    }

    #[test]
    fn finds_nothing_when_the_port_is_free() {
        assert!(parse_netstat_listen_pids(NETSTAT, 3100).is_empty());
    }
}
