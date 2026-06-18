/// Extract the origin (scheme + host[:port]) of an http(s) URL, e.g.
/// `https://example.com:8080/path?q=1` -> `https://example.com:8080`.
/// Returns `None` for non-http(s) URLs (about:, error pages, etc.) so callers
/// can leave those navigations untouched.
// Only consumed by the webview window; unused on other targets.
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
pub fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = if let Some(rest) = url.strip_prefix("https://") {
        ("https", rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        ("http", rest)
    } else {
        return None;
    };

    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        // Drop any userinfo (user:pass@host) so only host[:port] remains.
        .rsplit('@')
        .next()
        .unwrap_or("");

    if authority.is_empty() {
        return None;
    }

    Some(format!("{}://{}", scheme, authority))
}

/// Check if a string looks like a URL
pub fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://") ||
    s.starts_with("www.") ||
    // Check for common URL patterns like domain.tld
    (s.contains('.') && !s.contains(' ') && {
        let parts: Vec<&str> = s.split('.').collect();
        parts.len() >= 2 && !parts.last().unwrap_or(&"").is_empty()
    })
}
