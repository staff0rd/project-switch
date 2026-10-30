use super::{google, Client};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const SCOPE: &str = "https://www.googleapis.com/auth/calendar.readonly";

fn random_state() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default(),
    );
    format!("{:016x}", hasher.finish())
}

fn consent_url(client_id: &str, redirect_uri: &str, state: &str) -> String {
    let params = [
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("scope", SCOPE),
        ("access_type", "offline"),
        ("prompt", "consent"),
        ("state", state),
    ];
    let query: Vec<String> = params
        .iter()
        .map(|(key, value)| format!("{key}={}", urlencoding::encode(value)))
        .collect();
    format!("{AUTH_URL}?{}", query.join("&"))
}

fn decode(value: &str) -> String {
    let value = value.replace('+', " ");
    urlencoding::decode(&value)
        .map(|decoded| decoded.into_owned())
        .unwrap_or(value)
}

fn parse_callback(request_line: &str) -> Option<HashMap<String, String>> {
    let target = request_line
        .strip_prefix("GET ")?
        .split_whitespace()
        .next()?;
    let (path, query) = target.split_once('?')?;
    if path != "/" {
        return None;
    }
    let params: HashMap<String, String> = query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (decode(key), decode(value)))
        .collect();
    (params.contains_key("code") || params.contains_key("error")).then_some(params)
}

fn callback_code(params: &HashMap<String, String>, state: &str) -> Result<String> {
    if let Some(error) = params.get("error") {
        anyhow::bail!("Google login failed: {error}");
    }
    if params.get("state").map(String::as_str) != Some(state) {
        anyhow::bail!("Google login returned an unexpected state: try again");
    }
    params
        .get("code")
        .cloned()
        .context("Google login returned no code")
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

fn wait_for_code(listener: &TcpListener, state: &str) -> Result<String> {
    for stream in listener.incoming() {
        let mut stream = stream.context("Failed to accept the Google login redirect")?;
        let mut request_line = String::new();
        BufReader::new(&stream)
            .read_line(&mut request_line)
            .context("Failed to read the Google login redirect")?;
        let Some(params) = parse_callback(&request_line) else {
            respond(&mut stream, "404 Not Found", "Not found");
            continue;
        };
        let code = callback_code(&params, state);
        match &code {
            Ok(_) => respond(
                &mut stream,
                "200 OK",
                "Signed in to Google Calendar. You can close this tab and return to the terminal.",
            ),
            Err(e) => respond(&mut stream, "400 Bad Request", &e.to_string()),
        }
        return code;
    }
    anyhow::bail!("The Google login listener stopped before a redirect arrived")
}

pub fn authorize(client: &Client, label: &str) -> Result<String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .context("Failed to open a local port for the Google login")?;
    let redirect_uri = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
    let state = random_state();
    let url = consent_url(client.id, &redirect_uri, &state);

    println!(
        "Opening Google login for '{label}' in your browser. If it doesn't open, visit:\n\n{url}\n"
    );
    if let Err(e) = crate::utils::browser::open_url_in_browser(&url, "default", false) {
        eprintln!("Couldn't open a browser: {e:#}");
    }

    let code = wait_for_code(&listener, &state)?;
    google::exchange_code(client, label, &code, &redirect_uri)?
        .refresh_token
        .context("Google sent no refresh token: remove project-switch's access at https://myaccount.google.com/permissions and try again")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consent_url_requests_offline_read_only_access() {
        let url = consent_url(
            "id.apps.googleusercontent.com",
            "http://127.0.0.1:5000",
            "s1",
        );
        assert_eq!(
            url,
            "https://accounts.google.com/o/oauth2/v2/auth?client_id=id.apps.googleusercontent.com\
             &redirect_uri=http%3A%2F%2F127.0.0.1%3A5000&response_type=code\
             &scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcalendar.readonly\
             &access_type=offline&prompt=consent&state=s1"
        );
    }

    #[test]
    fn callback_with_code_is_parsed() {
        let params = parse_callback(
            "GET /?state=s1&code=4%2F0Ab_x&scope=https://www.googleapis.com/auth/calendar.readonly HTTP/1.1\r\n",
        )
        .unwrap();
        assert_eq!(callback_code(&params, "s1").unwrap(), "4/0Ab_x");
    }

    #[test]
    fn unrelated_requests_are_ignored() {
        assert!(parse_callback("GET /favicon.ico HTTP/1.1\r\n").is_none());
        assert!(parse_callback("GET / HTTP/1.1\r\n").is_none());
        assert!(parse_callback("GET /?foo=bar HTTP/1.1\r\n").is_none());
        assert!(parse_callback("POST /?code=x HTTP/1.1\r\n").is_none());
    }

    #[test]
    fn denied_consent_and_wrong_state_fail() {
        let denied = parse_callback("GET /?error=access_denied&state=s1 HTTP/1.1").unwrap();
        assert_eq!(
            callback_code(&denied, "s1").unwrap_err().to_string(),
            "Google login failed: access_denied"
        );
        let forged = parse_callback("GET /?code=x&state=other HTTP/1.1").unwrap();
        assert!(callback_code(&forged, "s1").is_err());
    }

    #[test]
    fn random_states_differ() {
        assert_ne!(random_state(), random_state());
    }
}
