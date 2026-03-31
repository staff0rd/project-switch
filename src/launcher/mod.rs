//! Shared launcher data model — filtering, matching, item types.
//! Used by both the CLI `list` command and the windowed GUI launcher.

/// The kind of item in the launcher list.
#[derive(Debug, Clone, PartialEq)]
pub enum ListItemKind {
    Command,
    Shortcut { path: String },
}

/// A single item in the launcher list.
#[derive(Debug, Clone, PartialEq)]
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

/// Three-state result for calculator input classification.
#[derive(Debug, Clone, PartialEq)]
pub enum CalcResult {
    /// Expression evaluated successfully.
    Ok(String),
    /// Expression is incomplete (trailing operator or unclosed parens).
    /// Contains the original expression text for echoing back.
    Incomplete(String),
    /// Expression is genuinely invalid.
    Invalid,
}

/// Returns `true` when the expression looks incomplete rather than invalid:
/// it ends with an operator or has unclosed parentheses.
pub fn is_incomplete_expr(expr: &str) -> bool {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return false;
    }

    let open = trimmed.chars().filter(|&c| c == '(').count();
    let close = trimmed.chars().filter(|&c| c == ')').count();
    if open > close {
        return true;
    }

    matches!(
        trimmed.as_bytes().last(),
        Some(b'+' | b'-' | b'*' | b'/' | b'^')
    )
}

/// Normalize bare decimal points (e.g. `.5` → `0.5`) so meval can parse them.
fn normalize_decimals(expr: &str) -> String {
    let bytes = expr.as_bytes();
    let mut out = String::with_capacity(expr.len() + 4);
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'.'
            && i + 1 < bytes.len()
            && bytes[i + 1].is_ascii_digit()
            && (i == 0 || !bytes[i - 1].is_ascii_digit())
        {
            out.push('0');
        }
        out.push(b as char);
    }
    out
}

/// Evaluate a calculator expression. Returns Ok(display_string) or Err on invalid input.
pub fn eval_calculator(expr: &str) -> Result<String, String> {
    let expr = expr.trim();
    if expr.is_empty() {
        return Err("empty expression".to_string());
    }
    let expr = &normalize_decimals(expr);
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

/// Classify a calculator expression as ok, incomplete, or invalid.
pub fn eval_calc_input(expr: &str) -> CalcResult {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return CalcResult::Invalid;
    }
    match eval_calculator(trimmed) {
        Ok(value) => CalcResult::Ok(value),
        Err(_) if is_incomplete_expr(trimmed) => CalcResult::Incomplete(trimmed.to_string()),
        Err(_) => CalcResult::Invalid,
    }
}

/// A file/directory entry returned by path browsing.
#[derive(Debug, Clone, PartialEq)]
pub struct PathEntry {
    pub full_path: String,
    pub is_dir: bool,
}

/// List filesystem entries matching a path input. Auto-expands single directory matches.
pub fn get_path_entries(input: &str) -> Vec<PathEntry> {
    let normalized = input.replace('/', "\\");

    let working = if normalized.len() == 2
        && normalized.as_bytes()[0].is_ascii_alphabetic()
        && normalized.as_bytes()[1] == b':'
    {
        format!("{}\\", normalized)
    } else {
        normalized
    };

    let (initial_dir, initial_filter) = match working.rfind('\\') {
        Some(pos) => (working[..=pos].to_string(), working[pos + 1..].to_string()),
        None => return Vec::new(),
    };

    let mut result = Vec::new();
    let mut dir_part = initial_dir;
    let mut filter = initial_filter;

    for _ in 0..10 {
        let entries = match std::fs::read_dir(&dir_part) {
            Ok(e) => e,
            Err(_) => break,
        };

        let filter_lower = filter.to_lowercase();
        let mut dirs: Vec<(String, String)> = Vec::new();
        let mut files: Vec<(String, String)> = Vec::new();

        for entry in entries.flatten() {
            let name = match entry.file_name().into_string() {
                Ok(n) => n,
                Err(_) => continue,
            };

            if !filter.is_empty() && !name.to_lowercase().starts_with(&filter_lower) {
                continue;
            }

            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            if is_dir {
                dirs.push((format!("{}{}\\", dir_part, name), name));
            } else {
                files.push((format!("{}{}", dir_part, name), name));
            }
        }

        dirs.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
        files.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

        for (full, _) in &dirs {
            // Without trailing \ = open the directory
            let open_path = full.trim_end_matches('\\').to_string();
            result.push(PathEntry {
                full_path: open_path,
                is_dir: true,
            });
            // With trailing \ = browse into it
            result.push(PathEntry {
                full_path: full.clone(),
                is_dir: true,
            });
        }

        // Single directory match, no files — auto-expand
        if dirs.len() == 1 && files.is_empty() {
            dir_part = dirs[0].0.clone();
            filter = String::new();
            continue;
        }

        for (full, _) in &files {
            result.push(PathEntry {
                full_path: full.clone(),
                is_dir: false,
            });
        }

        break;
    }

    result
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
    fn eval_calculator_bare_decimal() {
        assert_eq!(eval_calculator("5+4+.01"), Ok("9.01".to_string()));
    }

    #[test]
    fn eval_calculator_leading_bare_decimal() {
        assert_eq!(eval_calculator(".5+.5"), Ok("1".to_string()));
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

    // --- is_incomplete_expr ---

    #[test]
    fn incomplete_trailing_plus() {
        assert!(is_incomplete_expr("5+"));
    }

    #[test]
    fn incomplete_trailing_minus() {
        assert!(is_incomplete_expr("5-"));
    }

    #[test]
    fn incomplete_trailing_multiply() {
        assert!(is_incomplete_expr("5*"));
    }

    #[test]
    fn incomplete_trailing_divide() {
        assert!(is_incomplete_expr("5/"));
    }

    #[test]
    fn incomplete_trailing_power() {
        assert!(is_incomplete_expr("2^"));
    }

    #[test]
    fn incomplete_unclosed_paren() {
        assert!(is_incomplete_expr("5*(3+2"));
    }

    #[test]
    fn incomplete_unclosed_paren_trailing_op() {
        assert!(is_incomplete_expr("5*(3+"));
    }

    #[test]
    fn incomplete_not_for_valid_expr() {
        assert!(!is_incomplete_expr("5+3"));
    }

    #[test]
    fn incomplete_not_for_nonsense() {
        assert!(!is_incomplete_expr("abc"));
    }

    #[test]
    fn incomplete_not_for_empty() {
        assert!(!is_incomplete_expr(""));
    }

    // --- eval_calc_input ---

    #[test]
    fn calc_input_valid_result() {
        assert_eq!(eval_calc_input("5+3"), CalcResult::Ok("8".to_string()));
    }

    #[test]
    fn calc_input_trailing_operator() {
        assert_eq!(
            eval_calc_input("5+"),
            CalcResult::Incomplete("5+".to_string())
        );
    }

    #[test]
    fn calc_input_unclosed_paren_trailing_op() {
        assert_eq!(
            eval_calc_input("5*(3+"),
            CalcResult::Incomplete("5*(3+".to_string())
        );
    }

    #[test]
    fn calc_input_genuinely_invalid() {
        assert_eq!(eval_calc_input("abc"), CalcResult::Invalid);
    }

    #[test]
    fn calc_input_empty() {
        assert_eq!(eval_calc_input(""), CalcResult::Invalid);
    }

    #[test]
    fn calc_input_complex_valid() {
        assert_eq!(eval_calc_input("(2+3)*4"), CalcResult::Ok("20".to_string()));
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
