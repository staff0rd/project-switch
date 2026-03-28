//! Shared launcher data model — filtering, matching, item types.
//! Used by both the CLI `list` command and the windowed GUI launcher.

/// The kind of item in the launcher list.
#[derive(Debug, Clone, PartialEq)]
pub enum ListItemKind {
    Command,
    Shortcut { path: String },
}

/// A single item in the launcher list.
#[derive(Debug, Clone)]
pub struct ListItem {
    pub key: String,
    pub display_detail: String,
    pub kind: ListItemKind,
}

impl ListItem {
    pub fn matches(&self, query: &str) -> bool {
        self.key.to_lowercase().contains(&query.to_lowercase())
    }
}

/// URL-encode user arguments while preserving path separators (slashes).
pub fn encode_url_args(url: &str, user_args: &str) -> String {
    let encoded: String = user_args
        .split('/')
        .map(|segment| urlencoding::encode(segment).into_owned())
        .collect::<Vec<_>>()
        .join("/");
    format!("{}{}", url, encoded)
}

/// Merge command-level args with user-supplied args.
pub fn merge_args(cmd_args: Option<&str>, user_args: Option<&str>) -> Option<String> {
    match (cmd_args, user_args) {
        (Some(c), Some(u)) => Some(format!("{} {}", c, u)),
        (Some(c), None) => Some(c.to_string()),
        (None, Some(u)) => Some(u.to_string()),
        (None, None) => None,
    }
}

/// Strip ANSI escape codes from a string.
pub fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if chars.next() == Some('[') {
                for ch in chars.by_ref() {
                    if ch.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Check if input looks like a file path (drive letter or UNC path).
pub fn is_file_path(input: &str) -> bool {
    let s = input.trim();
    if s.len() >= 2 && s.as_bytes()[0].is_ascii_alphabetic() && s.as_bytes()[1] == b':' {
        return true;
    }
    if s.starts_with("\\\\") {
        return true;
    }
    false
}

/// Evaluate a calculator expression. Returns Ok(display_string) or Err on invalid input.
pub fn eval_calculator(expr: &str) -> Result<String, String> {
    let expr = expr.trim();
    if expr.is_empty() {
        return Err("empty expression".to_string());
    }
    match meval::eval_str(expr) {
        Ok(result) => {
            if result.fract() == 0.0 {
                Ok(format!("{}", result as i64))
            } else {
                Ok(format!("{}", result))
            }
        }
        Err(e) => Err(format!("{}", e)),
    }
}

/// Filter a list of items by query string, returning matching items in order.
/// When the query contains a space (i.e. keyword + args), use exact key match
/// so that "g some text" only matches a "g" key, not everything containing "g".
pub fn filter_items<'a>(items: &'a [ListItem], query: &str) -> Vec<&'a ListItem> {
    if query.is_empty() {
        items.iter().collect()
    } else {
        let keyword = query.split_whitespace().next().unwrap_or(query);
        let has_args = query.contains(' ');
        items
            .iter()
            .filter(|item| {
                if has_args {
                    item.key.to_lowercase() == keyword.to_lowercase()
                } else {
                    item.matches(keyword)
                }
            })
            .collect()
    }
}

/// Find the best matching item for a given input.
/// Returns the matched item and any remaining arguments.
pub fn resolve_item<'a>(
    items: &'a [ListItem],
    input: &str,
) -> Option<(&'a ListItem, Option<String>)> {
    // Try exact match on full input (handles multi-word keys like shortcuts)
    if let Some(item) = items
        .iter()
        .find(|item| item.key.to_lowercase() == input.to_lowercase())
    {
        return Some((item, None));
    }

    // Split into keyword + args
    let (keyword, args) = if let Some(space_pos) = input.find(' ') {
        let kw = &input[..space_pos];
        let rest = input[space_pos + 1..].trim();
        (
            kw.to_string(),
            if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            },
        )
    } else {
        (input.to_string(), None)
    };

    // Try exact match on keyword
    if let Some(item) = items
        .iter()
        .find(|item| item.key.to_lowercase() == keyword.to_lowercase())
    {
        return Some((item, args));
    }

    // Partial match fallback
    if let Some(item) = items
        .iter()
        .find(|item| item.key.to_lowercase().contains(&keyword.to_lowercase()))
    {
        return Some((item, args));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_items() -> Vec<ListItem> {
        vec![
            ListItem {
                key: "github".to_string(),
                display_detail: "https://github.com/".to_string(),
                kind: ListItemKind::Command,
            },
            ListItem {
                key: "jira".to_string(),
                display_detail: "https://jira.example.com/".to_string(),
                kind: ListItemKind::Command,
            },
            ListItem {
                key: "slack".to_string(),
                display_detail: "https://slack.com/".to_string(),
                kind: ListItemKind::Command,
            },
            ListItem {
                key: "Visual Studio Code".to_string(),
                display_detail: "C:\\Program Files\\Code.exe".to_string(),
                kind: ListItemKind::Shortcut {
                    path: "C:\\ProgramData\\Start Menu\\Visual Studio Code.lnk".to_string(),
                },
            },
        ]
    }

    // --- encode_url_args ---

    #[test]
    fn encode_url_args_preserves_slashes() {
        assert_eq!(
            encode_url_args("https://github.com/", "staff0rd/assist"),
            "https://github.com/staff0rd/assist"
        );
    }

    #[test]
    fn encode_url_args_encodes_spaces() {
        assert_eq!(
            encode_url_args("https://www.google.com/search?q=", "hello world"),
            "https://www.google.com/search?q=hello%20world"
        );
    }

    #[test]
    fn encode_url_args_empty_args() {
        assert_eq!(
            encode_url_args("https://example.com/", ""),
            "https://example.com/"
        );
    }

    // --- merge_args ---

    #[test]
    fn merge_args_both() {
        assert_eq!(
            merge_args(Some("--flag"), Some("value")),
            Some("--flag value".to_string())
        );
    }

    #[test]
    fn merge_args_cmd_only() {
        assert_eq!(merge_args(Some("--flag"), None), Some("--flag".to_string()));
    }

    #[test]
    fn merge_args_user_only() {
        assert_eq!(merge_args(None, Some("value")), Some("value".to_string()));
    }

    #[test]
    fn merge_args_neither() {
        assert_eq!(merge_args(None, None), None);
    }

    // --- strip_ansi_codes ---

    #[test]
    fn strip_ansi_removes_color_codes() {
        assert_eq!(strip_ansi_codes("\x1b[32mhello\x1b[0m"), "hello");
    }

    #[test]
    fn strip_ansi_preserves_plain_text() {
        assert_eq!(strip_ansi_codes("hello world"), "hello world");
    }

    // --- is_file_path ---

    #[test]
    fn is_file_path_drive_letter() {
        assert!(is_file_path("C:\\Users"));
        assert!(is_file_path("d:"));
    }

    #[test]
    fn is_file_path_unc() {
        assert!(is_file_path("\\\\server\\share"));
    }

    #[test]
    fn is_file_path_not_a_path() {
        assert!(!is_file_path("github"));
        assert!(!is_file_path("https://example.com"));
        assert!(!is_file_path(""));
    }

    // --- eval_calculator ---

    #[test]
    fn eval_calculator_integer_result() {
        assert_eq!(eval_calculator("5+3"), Ok("8".to_string()));
    }

    #[test]
    fn eval_calculator_float_result() {
        assert_eq!(
            eval_calculator("10/3"),
            Ok("3.3333333333333335".to_string())
        );
    }

    #[test]
    fn eval_calculator_empty_expression() {
        assert!(eval_calculator("").is_err());
    }

    #[test]
    fn eval_calculator_invalid_expression() {
        assert!(eval_calculator("abc").is_err());
    }

    #[test]
    fn eval_calculator_complex_expression() {
        assert_eq!(eval_calculator("(2+3)*4"), Ok("20".to_string()));
    }

    // --- filter_items ---

    #[test]
    fn filter_items_empty_query_returns_all() {
        let items = sample_items();
        let filtered = filter_items(&items, "");
        assert_eq!(filtered.len(), 4);
    }

    #[test]
    fn filter_items_exact_match() {
        let items = sample_items();
        let filtered = filter_items(&items, "github");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].key, "github");
    }

    #[test]
    fn filter_items_partial_match() {
        let items = sample_items();
        let filtered = filter_items(&items, "git");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].key, "github");
    }

    #[test]
    fn filter_items_case_insensitive() {
        let items = sample_items();
        let filtered = filter_items(&items, "JIRA");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].key, "jira");
    }

    #[test]
    fn filter_items_no_match() {
        let items = sample_items();
        let filtered = filter_items(&items, "nonexistent");
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn filter_items_with_args_filters_by_keyword_only() {
        let items = sample_items();
        let filtered = filter_items(&items, "github staff0rd/repo");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].key, "github");
    }

    #[test]
    fn filter_items_with_args_exact_match_when_space() {
        // "g some text" should only match an item with key "g", not everything containing "g"
        let mut items = sample_items();
        items.push(ListItem {
            key: "g".to_string(),
            display_detail: "https://google.com/search?q=".to_string(),
            kind: ListItemKind::Command,
        });
        let filtered = filter_items(&items, "g some text");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].key, "g");
    }

    // --- resolve_item ---

    #[test]
    fn resolve_item_exact_match() {
        let items = sample_items();
        let (item, args) = resolve_item(&items, "github").unwrap();
        assert_eq!(item.key, "github");
        assert_eq!(args, None);
    }

    #[test]
    fn resolve_item_with_args() {
        let items = sample_items();
        let (item, args) = resolve_item(&items, "github staff0rd/repo").unwrap();
        assert_eq!(item.key, "github");
        assert_eq!(args, Some("staff0rd/repo".to_string()));
    }

    #[test]
    fn resolve_item_multi_word_key() {
        let items = sample_items();
        let (item, args) = resolve_item(&items, "Visual Studio Code").unwrap();
        assert_eq!(item.key, "Visual Studio Code");
        assert_eq!(args, None);
    }

    #[test]
    fn resolve_item_partial_match() {
        let items = sample_items();
        let (item, _) = resolve_item(&items, "git").unwrap();
        assert_eq!(item.key, "github");
    }

    #[test]
    fn resolve_item_case_insensitive() {
        let items = sample_items();
        let (item, _) = resolve_item(&items, "GITHUB").unwrap();
        assert_eq!(item.key, "github");
    }

    #[test]
    fn resolve_item_no_match() {
        let items = sample_items();
        assert!(resolve_item(&items, "nonexistent").is_none());
    }

    // --- ListItem::matches ---

    #[test]
    fn list_item_matches_case_insensitive() {
        let item = ListItem {
            key: "GitHub".to_string(),
            display_detail: String::new(),
            kind: ListItemKind::Command,
        };
        assert!(item.matches("github"));
        assert!(item.matches("Git"));
        assert!(item.matches("GITHUB"));
        assert!(!item.matches("jira"));
    }
}
