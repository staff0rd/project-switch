use super::colours::{calendar_colour, event_colour};
use super::{auth_hint, local_midnight, CalendarEvent, Client, EventColour};
use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDate};
use serde::Deserialize;
use std::time::Duration;

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const API_URL: &str = "https://www.googleapis.com/calendar/v3";

type HttpResult = std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into()
}

fn read_body(response: HttpResult) -> Result<(ureq::http::StatusCode, String)> {
    let mut response = response.context("Couldn't reach Google")?;
    let body = response
        .body_mut()
        .read_to_string()
        .context("Couldn't read Google's response")?;
    Ok((response.status(), body))
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

pub fn parse_tokens(body: &str, label: &str) -> Result<Tokens> {
    let response: TokenResponse =
        serde_json::from_str(body).context("Google sent an unreadable token response")?;
    match response.error.as_deref() {
        Some("invalid_grant") => anyhow::bail!(auth_hint(label)),
        Some(error) => anyhow::bail!(
            "Google rejected the login: {}",
            response.error_description.as_deref().unwrap_or(error)
        ),
        None => {}
    }
    Ok(Tokens {
        access_token: response
            .access_token
            .context("Google sent no access token")?,
        refresh_token: response.refresh_token,
    })
}

fn request_tokens(agent: &ureq::Agent, form: &[(&str, &str)], label: &str) -> Result<Tokens> {
    let (_, body) = read_body(agent.post(TOKEN_URL).send_form(form.iter().copied()))?;
    parse_tokens(&body, label)
}

pub fn exchange_code(
    client: &Client,
    label: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<Tokens> {
    request_tokens(
        &agent(),
        &[
            ("client_id", client.id),
            ("client_secret", client.secret),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ],
        label,
    )
}

fn refresh_access_token(
    agent: &ureq::Agent,
    client: &Client,
    label: &str,
    refresh_token: &str,
) -> Result<String> {
    let tokens = request_tokens(
        agent,
        &[
            ("client_id", client.id),
            ("client_secret", client.secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ],
        label,
    )?;
    Ok(tokens.access_token)
}

#[derive(Deserialize)]
struct ApiErrorResponse {
    error: ApiError,
}

#[derive(Deserialize)]
struct ApiError {
    message: String,
}

fn api_error(status: u16, body: &str) -> String {
    match serde_json::from_str::<ApiErrorResponse>(body) {
        Ok(response) => format!(
            "Google Calendar API error {status}: {}",
            response.error.message
        ),
        Err(_) => format!("Google Calendar API error {status}"),
    }
}

fn get(
    agent: &ureq::Agent,
    access_token: &str,
    url: &str,
    query: &[(&str, &str)],
) -> Result<String> {
    let mut request = agent
        .get(url)
        .header("Authorization", format!("Bearer {access_token}"));
    for (key, value) in query {
        request = request.query(*key, *value);
    }
    let (status, body) = read_body(request.call())?;
    if !status.is_success() {
        anyhow::bail!(api_error(status.as_u16(), &body));
    }
    Ok(body)
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalendarInfo {
    pub id: String,
    pub colour: EventColour,
}

#[derive(Deserialize)]
struct CalendarListResponse {
    #[serde(default)]
    items: Vec<CalendarListEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CalendarListEntry {
    id: String,
    background_color: Option<String>,
    #[serde(default)]
    selected: bool,
}

pub fn parse_selected_calendars(body: &str) -> Result<Vec<CalendarInfo>> {
    let response: CalendarListResponse =
        serde_json::from_str(body).context("Google sent an unreadable calendar list")?;
    Ok(response
        .items
        .into_iter()
        .filter(|entry| entry.selected)
        .map(|entry| CalendarInfo {
            colour: calendar_colour(entry.background_color.as_deref()),
            id: entry.id,
        })
        .collect())
}

#[derive(Deserialize)]
struct EventsResponse {
    #[serde(default)]
    items: Vec<EventEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventEntry {
    status: Option<String>,
    summary: Option<String>,
    color_id: Option<String>,
    start: Option<EventTime>,
    end: Option<EventTime>,
    event_type: Option<String>,
    #[serde(default)]
    attendees: Vec<Attendee>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventTime {
    date_time: Option<String>,
    date: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Attendee {
    #[serde(rename = "self", default)]
    is_self: bool,
    response_status: Option<String>,
}

impl EventTime {
    fn parse(&self) -> Option<(DateTime<Local>, bool)> {
        if let Some(date_time) = &self.date_time {
            let parsed = DateTime::parse_from_rfc3339(date_time).ok()?;
            return Some((parsed.with_timezone(&Local), false));
        }
        let date = NaiveDate::parse_from_str(self.date.as_deref()?, "%Y-%m-%d").ok()?;
        Some((local_midnight(date)?, true))
    }
}

impl EventEntry {
    fn is_shown(&self) -> bool {
        let declined = self.attendees.iter().any(|attendee| {
            attendee.is_self && attendee.response_status.as_deref() == Some("declined")
        });
        self.status.as_deref() != Some("cancelled")
            && self.event_type.as_deref() != Some("workingLocation")
            && !declined
    }
}

pub fn parse_events(body: &str, calendar: &CalendarInfo) -> Result<Vec<CalendarEvent>> {
    let response: EventsResponse = serde_json::from_str(body)
        .with_context(|| format!("Google sent unreadable events for '{}'", calendar.id))?;
    Ok(response
        .items
        .into_iter()
        .filter(EventEntry::is_shown)
        .filter_map(|entry| {
            let (start, all_day) = entry.start.as_ref()?.parse()?;
            let (end, _) = entry.end.as_ref()?.parse()?;
            Some(CalendarEvent {
                title: entry.summary.unwrap_or_else(|| "(No title)".to_string()),
                start,
                end,
                all_day,
                colour: entry
                    .color_id
                    .as_deref()
                    .and_then(event_colour)
                    .unwrap_or(calendar.colour),
            })
        })
        .collect())
}

pub fn fetch_events(
    client: &Client,
    label: &str,
    refresh_token: &str,
    start: DateTime<Local>,
    end: DateTime<Local>,
) -> Result<Vec<CalendarEvent>> {
    let agent = agent();
    let access_token = refresh_access_token(&agent, client, label, refresh_token)?;
    let calendars = parse_selected_calendars(&get(
        &agent,
        &access_token,
        &format!("{API_URL}/users/me/calendarList"),
        &[("maxResults", "250")],
    )?)?;
    let (time_min, time_max) = (start.to_rfc3339(), end.to_rfc3339());
    let mut events = Vec::new();
    for calendar in &calendars {
        let body = get(
            &agent,
            &access_token,
            &format!(
                "{API_URL}/calendars/{}/events",
                urlencoding::encode(&calendar.id)
            ),
            &[
                ("singleEvents", "true"),
                ("orderBy", "startTime"),
                ("timeMin", &time_min),
                ("timeMax", &time_max),
                ("maxResults", "250"),
            ],
        )?;
        events.extend(parse_events(&body, calendar)?);
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CALENDAR_LIST: &str = r##"{
      "kind": "calendar#calendarList",
      "items": [
        {"id": "me@makerx.com.au", "summary": "me@makerx.com.au", "backgroundColor": "#9fe1e7", "foregroundColor": "#000000", "selected": true, "primary": true},
        {"id": "team@group.calendar.google.com", "summary": "Team", "summaryOverride": "Team leave", "backgroundColor": "#f83a22", "foregroundColor": "#ffffff", "selected": true},
        {"id": "hidden@group.calendar.google.com", "summary": "Hidden", "backgroundColor": "#000000"}
      ]
    }"##;

    const EVENTS: &str = r##"{
      "kind": "calendar#events",
      "items": [
        {"status": "confirmed", "summary": "Standup", "start": {"dateTime": "2026-10-30T09:15:00+11:00", "timeZone": "Australia/Sydney"}, "end": {"dateTime": "2026-10-30T09:30:00+11:00"}, "recurringEventId": "abc"},
        {"status": "confirmed", "summary": "Client demo", "colorId": "6", "start": {"dateTime": "2026-10-30T02:30:00Z"}, "end": {"dateTime": "2026-10-30T03:30:00Z"}},
        {"status": "confirmed", "summary": "Sprint 42 ends", "start": {"date": "2026-10-30"}, "end": {"date": "2026-10-31"}},
        {"status": "cancelled", "summary": "Moved", "start": {"dateTime": "2026-10-30T10:00:00+11:00"}, "end": {"dateTime": "2026-10-30T11:00:00+11:00"}},
        {"status": "confirmed", "eventType": "workingLocation", "summary": "Home", "start": {"date": "2026-10-30"}, "end": {"date": "2026-10-31"}},
        {"status": "confirmed", "summary": "Declined sync", "start": {"dateTime": "2026-10-30T12:00:00+11:00"}, "end": {"dateTime": "2026-10-30T12:30:00+11:00"}, "attendees": [{"email": "me@makerx.com.au", "self": true, "responseStatus": "declined"}]},
        {"status": "confirmed", "start": {"dateTime": "2026-10-30T16:00:00+11:00"}, "end": {"dateTime": "2026-10-30T16:30:00+11:00"}, "attendees": [{"email": "me@makerx.com.au", "self": true, "responseStatus": "accepted"}]}
      ]
    }"##;

    fn instant(rfc3339: &str) -> DateTime<Local> {
        DateTime::parse_from_rfc3339(rfc3339)
            .unwrap()
            .with_timezone(&Local)
    }

    fn primary() -> CalendarInfo {
        parse_selected_calendars(CALENDAR_LIST).unwrap().remove(0)
    }

    #[test]
    fn only_selected_calendars_are_kept_with_their_colours() {
        let calendars = parse_selected_calendars(CALENDAR_LIST).unwrap();
        assert_eq!(
            calendars,
            vec![
                CalendarInfo {
                    id: "me@makerx.com.au".to_string(),
                    colour: EventColour {
                        background: [0x03, 0x9b, 0xe5],
                        foreground: [0x1d, 0x1d, 0x1d],
                    },
                },
                CalendarInfo {
                    id: "team@group.calendar.google.com".to_string(),
                    colour: EventColour {
                        background: [0xd5, 0x00, 0x00],
                        foreground: [0x1d, 0x1d, 0x1d],
                    },
                },
            ]
        );
    }

    #[test]
    fn events_parse_times_colours_and_skip_hidden_entries() {
        let events = parse_events(EVENTS, &primary()).unwrap();
        let titles: Vec<&str> = events.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Standup", "Client demo", "Sprint 42 ends", "(No title)"]
        );

        assert_eq!(events[0].start, instant("2026-10-29T22:15:00Z"));
        assert_eq!(events[0].end, instant("2026-10-29T22:30:00Z"));
        assert!(!events[0].all_day);
        assert_eq!(events[0].colour, primary().colour);

        assert_eq!(events[1].start, instant("2026-10-30T13:30:00+11:00"));
        assert_eq!(events[1].colour, event_colour("6").unwrap());

        assert!(events[2].all_day);
        assert_eq!(
            events[2].start.date_naive(),
            NaiveDate::from_ymd_opt(2026, 10, 30).unwrap()
        );
        assert_eq!(events[2].start.time(), chrono::NaiveTime::MIN);
    }

    #[test]
    fn unknown_colour_id_falls_back_to_the_calendar() {
        let body = r#"{"items": [{"summary": "x", "colorId": "99", "start": {"dateTime": "2026-10-30T09:00:00+11:00"}, "end": {"dateTime": "2026-10-30T10:00:00+11:00"}}]}"#;
        let events = parse_events(body, &primary()).unwrap();
        assert_eq!(events[0].colour, primary().colour);
    }

    #[test]
    fn empty_event_list_parses() {
        let body = r#"{"kind": "calendar#events"}"#;
        assert!(parse_events(body, &primary()).unwrap().is_empty());
    }

    #[test]
    fn token_response_yields_tokens() {
        let body = r#"{"access_token": "ya29.x", "expires_in": 3599, "refresh_token": "1//r", "scope": "https://www.googleapis.com/auth/calendar.readonly", "token_type": "Bearer"}"#;
        assert_eq!(
            parse_tokens(body, "work").unwrap(),
            Tokens {
                access_token: "ya29.x".to_string(),
                refresh_token: Some("1//r".to_string()),
            }
        );
    }

    #[test]
    fn revoked_refresh_token_asks_for_auth() {
        let body = r#"{"error": "invalid_grant", "error_description": "Token has been expired or revoked."}"#;
        let err = parse_tokens(body, "work").unwrap_err().to_string();
        assert_eq!(err, auth_hint("work"));
        assert!(err.contains("project-switch calendar auth work"));
    }

    #[test]
    fn other_token_errors_use_googles_description() {
        let body = r#"{"error": "invalid_client", "error_description": "The OAuth client was not found."}"#;
        assert_eq!(
            parse_tokens(body, "work").unwrap_err().to_string(),
            "Google rejected the login: The OAuth client was not found."
        );
    }

    #[test]
    fn api_errors_show_googles_message() {
        let body = r#"{"error": {"code": 403, "message": "Google Calendar API has not been used in project 1 before or it is disabled."}}"#;
        assert_eq!(
            api_error(403, body),
            "Google Calendar API error 403: Google Calendar API has not been used in project 1 before or it is disabled."
        );
        assert_eq!(api_error(502, "<html>"), "Google Calendar API error 502");
    }
}
