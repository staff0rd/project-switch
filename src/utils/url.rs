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
