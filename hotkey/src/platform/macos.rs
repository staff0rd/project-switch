use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn binary_name() -> &'static str {
    "project-switch"
}

pub fn kill_existing_hotkey_instances() {
    let our_pid = std::process::id().to_string();
    let output = Command::new("pgrep")
        .args(["-f", "project-switch-hotkey"])
        .output();

    if let Ok(output) = output {
        let pids = String::from_utf8_lossy(&output.stdout);
        for pid in pids.lines() {
            let pid = pid.trim();
            if !pid.is_empty() && pid != our_pid {
                let _ = Command::new("kill").arg(pid).status();
            }
        }
    }
}

pub fn launch_project_switch(project_switch: &Path) {
    // Kill any existing project-switch instances
    let _ = Command::new("pkill")
        .args(["-f", "project-switch list"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    // Write a wrapper script; iTerm's `command` parameter doesn't go through a shell
    let wrapper = project_switch.with_file_name("launch-list.sh");
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let rc = if shell.ends_with("zsh") {
        "[ -f ~/.zshrc ] && source ~/.zshrc 2>/dev/null"
    } else {
        "[ -f ~/.bashrc ] && source ~/.bashrc 2>/dev/null"
    };
    let script_body = format!(
        "#!{shell} -l\n{rc}\n{exe} list\n",
        exe = project_switch.display()
    );
    if fs::write(&wrapper, &script_body).is_err() {
        return;
    }
    let _ = fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755));

    let cmd = wrapper.display().to_string();

    if is_app_installed("iTerm") {
        let script = format!(
            r#"tell application "iTerm2"
    activate
    create window with default profile command "{cmd}"
end tell"#
        );
        let _ = Command::new("osascript").args(["-e", &script]).spawn();
    } else {
        let script = format!(
            r#"tell application "Terminal"
    activate
    do script "{cmd}"
end tell"#
        );
        let _ = Command::new("osascript").args(["-e", &script]).spawn();
    }
}

fn is_app_installed(name: &str) -> bool {
    Path::new(&format!("/Applications/{name}.app")).exists()
}
