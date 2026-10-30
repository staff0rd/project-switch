use super::CalendarContent;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarCache {
    pub fetched_at: SystemTime,
    pub content: CalendarContent,
}

fn cache_path() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".project-switch-calendar.yml"))
}

fn fetched_today(cache: &CalendarCache) -> bool {
    DateTime::<Local>::from(cache.fetched_at).date_naive() == Local::now().date_naive()
}

fn load_from(path: &Path) -> Option<CalendarCache> {
    let contents = fs::read_to_string(path).ok()?;
    serde_yaml::from_str::<CalendarCache>(&contents)
        .ok()
        .filter(fetched_today)
}

fn save_to(path: &Path, content: &CalendarContent) {
    if !matches!(content, CalendarContent::Day(_)) {
        let _ = fs::remove_file(path);
        return;
    }
    let cache = CalendarCache {
        fetched_at: SystemTime::now(),
        content: content.clone(),
    };
    if let Ok(yaml) = serde_yaml::to_string(&cache) {
        let _ = fs::write(path, yaml);
    }
}

pub fn load() -> Option<CalendarCache> {
    load_from(&cache_path()?)
}

pub fn save(content: &CalendarContent) {
    if let Some(path) = cache_path() {
        save_to(&path, content);
    }
}

pub fn clear() {
    if let Some(path) = cache_path() {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::{CalendarDay, CalendarEvent, EventColour};
    use std::time::Duration;

    fn temp_path(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ps-calendar-cache-{}-{name}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        dir.join(".project-switch-calendar.yml")
    }

    fn day() -> CalendarContent {
        let start = crate::calendar::today_start() + chrono::Duration::hours(9);
        CalendarContent::Day(CalendarDay {
            events: vec![CalendarEvent {
                title: "Standup".to_string(),
                start,
                end: start + chrono::Duration::minutes(15),
                all_day: false,
                colour: EventColour {
                    background: [1, 2, 3],
                    foreground: [4, 5, 6],
                },
            }],
            errors: vec!["Calendar 'work': boom".to_string()],
        })
    }

    #[test]
    fn saved_day_loads_back() {
        let path = temp_path("roundtrip");
        save_to(&path, &day());
        let cache = load_from(&path).unwrap();
        assert_eq!(cache.content, day());
        assert!(cache.fetched_at.elapsed().unwrap() < Duration::from_secs(60));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn not_configured_is_not_cached() {
        let path = temp_path("not-configured");
        save_to(&path, &day());
        save_to(&path, &CalendarContent::NotConfigured);
        assert!(load_from(&path).is_none());
    }

    #[test]
    fn cache_from_an_earlier_day_is_ignored() {
        let path = temp_path("yesterday");
        let cache = CalendarCache {
            fetched_at: SystemTime::now() - Duration::from_secs(48 * 60 * 60),
            content: day(),
        };
        fs::write(&path, serde_yaml::to_string(&cache).unwrap()).unwrap();
        assert!(load_from(&path).is_none());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_or_corrupt_cache_loads_nothing() {
        let path = temp_path("corrupt");
        let _ = fs::remove_file(&path);
        assert!(load_from(&path).is_none());
        fs::write(&path, "not: [a cache").unwrap();
        assert!(load_from(&path).is_none());
        let _ = fs::remove_file(path);
    }
}
