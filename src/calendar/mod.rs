pub mod cache;
pub mod colours;
pub mod google;
pub mod oauth;

use crate::config::{CalendarConfig, ConfigManager};
use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDate};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum CalendarCommand {
    #[command(about = "Log in to a Google account and save its token to ~/.project-switch.yml")]
    Auth {
        #[arg(help = "Name for this Google account, e.g. work")]
        label: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EventColour {
    pub background: [u8; 3],
    pub foreground: [u8; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub title: String,
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
    pub all_day: bool,
    pub colour: EventColour,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CalendarDays {
    pub days: BTreeMap<NaiveDate, Vec<CalendarEvent>>,
    pub errors: Vec<String>,
}

impl CalendarDays {
    pub fn events_on(&self, date: NaiveDate) -> &[CalendarEvent] {
        self.days.get(&date).map_or(&[], Vec::as_slice)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CalendarContent {
    Loading,
    NotConfigured,
    Days(CalendarDays),
}

const WINDOW_DAYS_BEFORE: u64 = 1;
const WINDOW_DAYS_AFTER: u64 = 5;

pub fn auth_hint(label: &str) -> String {
    format!("Google rejected the saved login: run `project-switch calendar auth {label}`")
}

pub fn local_midnight(date: NaiveDate) -> Option<DateTime<Local>> {
    date.and_hms_opt(0, 0, 0)?
        .and_local_timezone(Local)
        .earliest()
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn day_bounds(date: NaiveDate) -> Option<(DateTime<Local>, DateTime<Local>)> {
    Some((local_midnight(date)?, local_midnight(date.succ_opt()?)?))
}

pub fn window_range(today: NaiveDate) -> (NaiveDate, NaiveDate) {
    (
        today - chrono::Days::new(WINDOW_DAYS_BEFORE),
        today + chrono::Days::new(WINDOW_DAYS_AFTER),
    )
}

fn split_by_day(
    events: &[CalendarEvent],
    first: NaiveDate,
    last: NaiveDate,
) -> BTreeMap<NaiveDate, Vec<CalendarEvent>> {
    first
        .iter_days()
        .take_while(|date| *date <= last)
        .filter_map(|date| {
            let (start, end) = day_bounds(date)?;
            let overlapping = events
                .iter()
                .filter(|event| event.start < end && event.end > start)
                .cloned()
                .collect();
            Some((date, overlapping))
        })
        .collect()
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

fn merge_accounts(
    results: Vec<(&str, Result<Vec<CalendarEvent>>)>,
) -> (Vec<CalendarEvent>, Vec<String>) {
    let mut events: Vec<CalendarEvent> = Vec::new();
    let mut errors = Vec::new();
    for (label, result) in results {
        let account = match result {
            Ok(account) => account,
            Err(e) => {
                errors.push(format!("{:#}", e.context(format!("Calendar '{label}'"))));
                continue;
            }
        };
        for event in account {
            let duplicate = events.iter().any(|known| {
                (&known.title, known.start, known.end) == (&event.title, event.start, event.end)
            });
            if !duplicate {
                events.push(event);
            }
        }
    }
    events.sort_by_key(|event| event.start);
    (events, errors)
}

pub fn load_window() -> Result<CalendarContent> {
    let (first, last) = window_range(today());
    load_range(first, last)
}

pub fn load_day(date: NaiveDate) -> Result<CalendarContent> {
    load_range(date, date)
}

fn load_range(first: NaiveDate, last: NaiveDate) -> Result<CalendarContent> {
    let config_manager = ConfigManager::new()?;
    let Some(config) = config_manager.get_calendar_config() else {
        return Ok(CalendarContent::NotConfigured);
    };
    let accounts = authorised_accounts(config);
    let Some(client) = client(config).filter(|_| !accounts.is_empty()) else {
        return Ok(CalendarContent::NotConfigured);
    };
    let (start, _) = day_bounds(first).context("Couldn't work out the calendar window")?;
    let (_, end) = day_bounds(last).context("Couldn't work out the calendar window")?;
    let results = std::thread::scope(|scope| {
        let fetches: Vec<_> = accounts
            .iter()
            .map(|&(label, refresh_token)| {
                let client = &client;
                let fetch = scope
                    .spawn(move || google::fetch_events(client, label, refresh_token, start, end));
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
    let (events, errors) = merge_accounts(results);
    Ok(CalendarContent::Days(CalendarDays {
        days: split_by_day(&events, first, last),
        errors,
    }))
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

    fn date(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
    }

    fn at(hour: u32) -> DateTime<Local> {
        local_midnight(date(2)).unwrap() + chrono::Duration::hours(hour.into())
    }

    fn spanning(title: &str, start: u32, end: u32) -> CalendarEvent {
        CalendarEvent {
            title: title.to_string(),
            start: at(start),
            end: at(end),
            all_day: false,
            colour: BLUE,
        }
    }

    fn event(title: &str, hour: u32, colour: EventColour) -> CalendarEvent {
        CalendarEvent {
            colour,
            ..spanning(title, hour, hour + 1)
        }
    }

    fn account(events: Vec<CalendarEvent>) -> Result<Vec<CalendarEvent>> {
        Ok(events)
    }

    #[test]
    fn accounts_merge_into_one_sorted_list() {
        let (events, errors) = merge_accounts(vec![
            ("work", account(vec![event("Retro", 14, BLUE)])),
            ("personal", account(vec![event("Gym", 7, GREEN)])),
        ]);
        let titles: Vec<&str> = events.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["Gym", "Retro"]);
        assert!(errors.is_empty());
    }

    #[test]
    fn failed_account_is_reported_while_others_render() {
        let (events, errors) = merge_accounts(vec![
            ("work", Err(anyhow::anyhow!(auth_hint("work")))),
            ("personal", account(vec![event("Gym", 7, GREEN)])),
        ]);
        assert_eq!(events.len(), 1);
        assert_eq!(errors, [format!("Calendar 'work': {}", auth_hint("work"))]);
    }

    #[test]
    fn shared_meetings_appear_once() {
        let (events, _) = merge_accounts(vec![
            ("work", account(vec![event("Offsite", 9, BLUE)])),
            ("personal", account(vec![event("Offsite", 9, GREEN)])),
        ]);
        assert_eq!(events.len(), 1);
    }

    fn titles_on(days: &BTreeMap<NaiveDate, Vec<CalendarEvent>>, day: u32) -> Vec<&str> {
        days[&date(day)].iter().map(|e| e.title.as_str()).collect()
    }

    #[test]
    fn window_splits_into_every_day_including_empty_ones() {
        let events = [spanning("Mon", 9, 10), spanning("Wed", 57, 58)];
        let days = split_by_day(&events, date(1), date(5));
        assert_eq!(
            days.keys().copied().collect::<Vec<_>>(),
            (1..=5).map(date).collect::<Vec<_>>()
        );
        assert!(titles_on(&days, 1).is_empty());
        assert_eq!(titles_on(&days, 2), ["Mon"]);
        assert_eq!(titles_on(&days, 4), ["Wed"]);
    }

    #[test]
    fn event_crossing_midnight_shows_on_both_days() {
        let events = [
            spanning("Late deploy", 22, 26),
            spanning("Ends at midnight", 23, 24),
        ];
        let days = split_by_day(&events, date(1), date(4));
        assert_eq!(titles_on(&days, 2), ["Late deploy", "Ends at midnight"]);
        assert_eq!(titles_on(&days, 3), ["Late deploy"]);
    }

    #[test]
    fn all_day_event_stays_on_its_own_days() {
        let mut sprint = spanning("Sprint", 0, 48);
        sprint.all_day = true;
        let days = split_by_day(&[sprint], date(1), date(5));
        assert!(titles_on(&days, 1).is_empty());
        assert_eq!(titles_on(&days, 2), ["Sprint"]);
        assert_eq!(titles_on(&days, 3), ["Sprint"]);
        assert!(titles_on(&days, 4).is_empty());
    }

    #[test]
    fn window_runs_from_yesterday_to_five_days_ahead() {
        assert_eq!(window_range(date(10)), (date(9), date(15)));
    }

    #[test]
    fn missing_day_has_no_events() {
        assert!(CalendarDays::default().events_on(date(1)).is_empty());
    }

    #[test]
    fn hex_colours_parse() {
        assert_eq!(parse_hex("#ffb878"), Some([0xff, 0xb8, 0x78]));
        assert_eq!(parse_hex("ffb878"), None);
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("#gggggg"), None);
    }

    #[test]
    fn day_bounds_span_one_local_day() {
        let (start, end) = day_bounds(date(2)).unwrap();
        assert_eq!(start.time(), chrono::NaiveTime::MIN);
        assert_eq!(end.date_naive(), date(3));
    }
}
