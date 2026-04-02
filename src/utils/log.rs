use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::SystemTime;

const MAX_LOG_SIZE: u64 = 64 * 1024;

/// Appends a timestamped error line to ~/.project-switch.log.
/// Truncates the file first if it exceeds 64 KB.
pub fn append_error(message: &str) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let log_path = home.join(".project-switch.log");

    // Truncate if over size limit
    if let Ok(meta) = fs::metadata(&log_path) {
        if meta.len() > MAX_LOG_SIZE {
            let _ = fs::write(&log_path, "");
        }
    }

    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) else {
        return;
    };

    let timestamp = format_timestamp();
    let _ = writeln!(file, "[{timestamp}] {message}");
}

fn format_timestamp() -> String {
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Convert days since epoch to date (simplified Gregorian)
    let (year, month, day) = days_to_date(days);

    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}:{seconds:02} UTC")
}

fn days_to_date(days_since_epoch: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days_since_epoch + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
