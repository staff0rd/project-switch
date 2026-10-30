pub mod colours;
pub mod google;
pub mod oauth;

use crate::config::{CalendarConfig, ConfigManager};
use anyhow::Result;
use chrono::{DateTime, Local, NaiveDate};
use clap::Subcommand;

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum CalendarCommand {
    #[command(about = "Log in to a Google account and save its token to ~/.project-switch.yml")]
    Auth {
        #[arg(help = "Name for this Google account, e.g. work")]
        label: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EventColour {
    pub background: [u8; 3],
    pub foreground: [u8; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalendarEvent {
    pub title: String,
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
    pub all_day: bool,
    pub colour: EventColour,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CalendarDay {
    pub events: Vec<CalendarEvent>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CalendarContent {
    Loading,
    NotConfigured,
    Day(CalendarDay),
}

pub fn auth_hint(label: &str) -> String {
    format!("Google rejected the saved login: run `project-switch calendar auth {label}`")
}

pub fn local_midnight(date: NaiveDate) -> Option<DateTime<Local>> {
    date.and_hms_opt(0, 0, 0)?
        .and_local_timezone(Local)
        .earliest()
}

pub fn today_start() -> DateTime<Local> {
    let now = Local::now();
    local_midnight(now.date_naive()).unwrap_or(now)
}

fn today_bounds() -> (DateTime<Local>, DateTime<Local>) {
    let start = today_start();
    let end = start
        .date_naive()
        .succ_opt()
        .and_then(local_midnight)
        .unwrap_or(start + chrono::Duration::days(1));
    (start, end)
}

pub fn parse_hex(hex: &str) -> Option<[u8; 3]> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

pub struct Client<'a> {
    pub id: &'a str,
    pub secret: &'a str,
}

pub fn client(config: &CalendarConfig) -> Option<Client<'_>> {
    Some(Client {
        id: non_empty(&config.client_id)?,
        secret: non_empty(&config.client_secret)?,
    })
}

fn authorised_accounts(config: &CalendarConfig) -> Vec<(&str, &str)> {
    config
        .accounts
        .iter()
        .filter_map(|account| Some((account.label.as_str(), non_empty(&account.refresh_token)?)))
        .collect()
}

fn merge_accounts(results: Vec<(&str, Result<CalendarDay>)>) -> CalendarDay {
    let mut day = CalendarDay::default();
    for (label, result) in results {
        let account = match result {
            Ok(account) => account,
            Err(e) => {
                day.errors
                    .push(format!("{:#}", e.context(format!("Calendar '{label}'"))));
                continue;
            }
        };
        for event in account.events {
            let duplicate = day.events.iter().any(|known| {
                (&known.title, known.start, known.end) == (&event.title, event.start, event.end)
            });
            if !duplicate {
                day.events.push(event);
            }
        }
    }
    day.events.sort_by_key(|event| event.start);
    day
}

pub fn load_today() -> Result<CalendarContent> {
    let config_manager = ConfigManager::new()?;
    let Some(config) = config_manager.get_calendar_config() else {
        return Ok(CalendarContent::NotConfigured);
    };
    let accounts = authorised_accounts(config);
    let Some(client) = client(config).filter(|_| !accounts.is_empty()) else {
        return Ok(CalendarContent::NotConfigured);
    };
    let (start, end) = today_bounds();
    let results = std::thread::scope(|scope| {
        let fetches: Vec<_> = accounts
            .iter()
            .map(|&(label, refresh_token)| {
                let client = &client;
                let fetch = scope
                    .spawn(move || google::fetch_day(client, label, refresh_token, start, end));
                (label, fetch)
            })
            .collect();
        fetches
            .into_iter()
            .map(|(label, fetch)| {
                let result = fetch
                    .join()
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("fetch crashed")));
                (label, result)
            })
            .collect()
    });
    Ok(CalendarContent::Day(merge_accounts(results)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CalendarAccount;

    fn calendar_config(id: Option<&str>, accounts: &[(&str, Option<&str>)]) -> CalendarConfig {
        CalendarConfig {
            client_id: id.map(str::to_string),
            client_secret: Some("secret".to_string()),
            accounts: accounts
                .iter()
                .map(|(label, token)| CalendarAccount {
                    label: label.to_string(),
                    refresh_token: token.map(str::to_string),
                })
                .collect(),
        }
    }

    #[test]
    fn client_needs_id_and_secret() {
        assert!(client(&calendar_config(None, &[])).is_none());
        assert!(client(&calendar_config(Some(" "), &[])).is_none());
        let config = calendar_config(Some("id"), &[]);
        let client = client(&config).unwrap();
        assert_eq!((client.id, client.secret), ("id", "secret"));
    }

    #[test]
    fn every_account_with_a_token_is_used() {
        let config = calendar_config(
            Some("id"),
            &[
                ("empty", Some("")),
                ("work", Some("rt")),
                ("personal", None),
                ("family", Some("rt2")),
            ],
        );
        assert_eq!(
            authorised_accounts(&config),
            [("work", "rt"), ("family", "rt2")]
        );
        assert!(authorised_accounts(&calendar_config(Some("id"), &[("a", None)])).is_empty());
    }

    const BLUE: EventColour = EventColour {
        background: [0, 0, 255],
        foreground: [255, 255, 255],
    };
    const GREEN: EventColour = EventColour {
        background: [0, 255, 0],
        foreground: [0, 0, 0],
    };

    fn at(hour: u32) -> DateTime<Local> {
        today_start() + chrono::Duration::hours(hour.into())
    }

    fn event(title: &str, hour: u32, colour: EventColour) -> CalendarEvent {
        CalendarEvent {
            title: title.to_string(),
            start: at(hour),
            end: at(hour + 1),
            all_day: false,
            colour,
        }
    }

    fn account(events: Vec<CalendarEvent>) -> Result<CalendarDay> {
        Ok(CalendarDay {
            events,
            errors: Vec::new(),
        })
    }

    #[test]
    fn accounts_merge_into_one_sorted_day() {
        let day = merge_accounts(vec![
            ("work", account(vec![event("Retro", 14, BLUE)])),
            ("personal", account(vec![event("Gym", 7, GREEN)])),
        ]);
        let titles: Vec<&str> = day.events.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["Gym", "Retro"]);
        assert!(day.errors.is_empty());
    }

    #[test]
    fn failed_account_is_reported_while_others_render() {
        let day = merge_accounts(vec![
            ("work", Err(anyhow::anyhow!(auth_hint("work")))),
            ("personal", account(vec![event("Gym", 7, GREEN)])),
        ]);
        assert_eq!(day.events.len(), 1);
        assert_eq!(
            day.errors,
            [format!("Calendar 'work': {}", auth_hint("work"))]
        );
    }

    #[test]
    fn shared_meetings_appear_once() {
        let day = merge_accounts(vec![
            ("work", account(vec![event("Offsite", 9, BLUE)])),
            ("personal", account(vec![event("Offsite", 9, GREEN)])),
        ]);
        assert_eq!(day.events.len(), 1);
    }

    #[test]
    fn hex_colours_parse() {
        assert_eq!(parse_hex("#ffb878"), Some([0xff, 0xb8, 0x78]));
        assert_eq!(parse_hex("ffb878"), None);
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("#gggggg"), None);
    }

    #[test]
    fn today_bounds_span_one_local_day() {
        let (start, end) = today_bounds();
        assert_eq!(start.time(), chrono::NaiveTime::MIN);
        assert_eq!(end.date_naive(), start.date_naive().succ_opt().unwrap());
    }
}
