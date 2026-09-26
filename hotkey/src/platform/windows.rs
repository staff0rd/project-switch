use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

use windows::Wdk::System::Threading::{NtQueryInformationProcess, PROCESSINFOCLASS};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
};

const CREATE_NO_WINDOW: u32 = 0x08000000;

const PROCESS_COMMAND_LINE_INFORMATION: PROCESSINFOCLASS = PROCESSINFOCLASS(60);

/// `UNICODE_STRING` as returned by `NtQueryInformationProcess`: its buffer
/// points back into the output buffer we supplied, not at separate storage.
#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *mut u16,
}

/// The daemon is a GUI-subsystem binary with no console, so stderr alone would
/// discard the message; the log file is the only place a failure survives.
fn warn(message: &str) {
    eprintln!("{message}");
    crate::log::append_error(message);
}

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

fn process_ids_named(name: &str) -> Vec<u32> {
    let mut pids = Vec::new();

    let snapshot = match unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) } {
        Ok(snapshot) => snapshot,
        Err(e) => {
            warn(&format!("Failed to snapshot running processes: {e}"));
            return pids;
        }
    };

    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            let end = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..end]).eq_ignore_ascii_case(name) {
                pids.push(entry.th32ProcessID);
            }
            if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }

    let _ = unsafe { CloseHandle(snapshot) };
    pids
}

fn process_command_line(pid: u32) -> Option<String> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;

    // u64 elements so the buffer is pointer-aligned for the UNICODE_STRING header.
    let mut buf = vec![0u64; 512];
    let mut command_line = None;
    for _ in 0..2 {
        let mut needed = 0u32;
        let status = unsafe {
            NtQueryInformationProcess(
                handle,
                PROCESS_COMMAND_LINE_INFORMATION,
                buf.as_mut_ptr().cast(),
                (buf.len() * 8) as u32,
                &mut needed,
            )
        };
        if status.is_ok() {
            command_line = unsafe {
                let raw = &*buf.as_ptr().cast::<UnicodeString>();
                (!raw.buffer.is_null() && raw.length > 0).then(|| {
                    String::from_utf16_lossy(std::slice::from_raw_parts(
                        raw.buffer,
                        raw.length as usize / 2,
                    ))
                })
            };
            break;
        }
        if needed as usize <= buf.len() * 8 {
            warn(&format!("Failed to read command line of pid {pid}: {status:?}"));
            break;
        }
        buf = vec![0u64; needed.div_ceil(8) as usize];
    }

    let _ = unsafe { CloseHandle(handle) };
    command_line
}

fn terminate_process(pid: u32) {
    let handle = match unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) } {
        Ok(handle) => handle,
        Err(e) => {
            warn(&format!("Failed to open pid {pid} for termination: {e}"));
            return;
        }
    };
    if let Err(e) = unsafe { TerminateProcess(handle, 1) } {
        warn(&format!("Failed to terminate pid {pid}: {e}"));
    }
    let _ = unsafe { CloseHandle(handle) };
}

pub fn kill_existing_hotkey_instances() {
    let our_pid = std::process::id();
    for pid in process_ids_named("project-switch-hotkey.exe") {
        if pid != our_pid {
            terminate_process(pid);
        }
    }
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
            warn(&format!("Failed to launch project-switch: {e}"));
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

    // Kill old instances, excluding the one we just spawned and the long-lived
    // webview window (a 'project-switch.exe webview <url>' process), which must
    // survive so re-triggering summons it rather than spawning a duplicate.
    for pid in process_ids_named(binary_name()) {
        if pid == new_pid {
            continue;
        }
        match process_command_line(pid) {
            Some(command_line) if command_line.contains("webview") => {}
            // Unreadable command line: leave it alone rather than risk the webview.
            None => warn(&format!("Skipping pid {pid}: command line unavailable")),
            Some(_) => terminate_process(pid),
        }
    }
}
