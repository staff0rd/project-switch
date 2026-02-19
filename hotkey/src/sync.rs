use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::config;
use crate::CREATE_NO_WINDOW;

/// Walk up from the file's parent directory looking for a `.git` directory.
/// Returns the repo root if found.
fn find_git_repo(file_path: &Path) -> Option<PathBuf> {
    let mut dir = file_path.parent()?;
    loop {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

/// Run `git pull` in the given repo directory. Errors are silently ignored
/// (network may be unavailable).
fn git_pull(repo: &Path) {
    let _ = Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "pull"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

/// Check for uncommitted changes and, if any, stage, commit, and push them.
fn git_sync(repo: &Path) {
    let repo_str = repo.to_string_lossy();

    // Check if there are any changes
    let output = Command::new("git")
        .args(["-C", &repo_str, "status", "--porcelain"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let has_changes = match output {
        Ok(o) => !o.stdout.is_empty(),
        Err(_) => return,
    };

    if !has_changes {
        return;
    }

    let _ = Command::new("git")
        .args(["-C", &repo_str, "add", "-A"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let _ = Command::new("git")
        .args([
            "-C",
            &repo_str,
            "commit",
            "-m",
            "auto-sync project-switch config",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let _ = Command::new("git")
        .args(["-C", &repo_str, "push"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

/// Spawn a background thread that periodically pulls and syncs the git repo
/// containing the included config file. Returns immediately. If there is no
/// include path or it isn't inside a git repo, no thread is spawned.
pub fn start_sync_thread() {
    let include_path = match config::read_include_path() {
        Some(p) => p,
        None => return,
    };

    let repo = match find_git_repo(&include_path) {
        Some(r) => r,
        None => return,
    };

    thread::spawn(move || {
        // Initial pull
        git_pull(&repo);

        loop {
            thread::sleep(Duration::from_secs(30));
            git_pull(&repo);
            git_sync(&repo);
        }
    });
}
