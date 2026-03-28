//! Background git sync for included config files.

use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn git_cmd(dir: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

fn git_pull(dir: &Path) {
    let _ = git_cmd(dir)
        .args(["pull", "--ff-only"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn git_sync(dir: &Path) {
    git_pull(dir);

    let status = git_cmd(dir).args(["status", "--porcelain"]).output();

    if let Ok(output) = status {
        if !output.stdout.is_empty() {
            let _ = git_cmd(dir).args(["add", "-A"]).status();
            let _ = git_cmd(dir)
                .args(["commit", "-m", "auto-sync project-switch config"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            let _ = git_cmd(dir)
                .args(["push"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    }
}

/// Start the background sync thread for the given config directory.
/// Returns immediately; the thread runs until the process exits.
pub fn start(include_path: Option<String>) {
    let dir = match include_path {
        Some(p) => {
            let path = crate::config::expand_include_path(&p);
            match path.parent() {
                Some(d) if d.join(".git").exists() => d.to_path_buf(),
                _ => return,
            }
        }
        None => return,
    };

    thread::spawn(move || {
        git_pull(&dir);
        loop {
            thread::sleep(Duration::from_secs(30));
            git_sync(&dir);
        }
    });
}
