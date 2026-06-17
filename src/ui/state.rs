//! Launcher window state machine.
//!
//! Manages visibility, input, filtering, selection, and transitions
//! independently of the GUI framework for testability.

use crate::launcher::{
    eval_calc_input, filter_items, is_file_path, order_recent_keys, CalcResult, ListItem,
};

/// The current input mode, derived from the input text.
#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    /// Normal item filtering mode.
    Normal,
    /// Calculator mode (input starts with `=`).
    Calculator { result: CalcResult },
    /// File path browsing mode.
    FilePath,
}

/// An entry in the filtered display list, including both regular items
/// and non-item history entries (expressions, file paths).
#[derive(Debug, Clone, PartialEq)]
pub enum FilteredEntry {
    /// A regular list item (command or shortcut).
    Item(ListItem),
    /// A recent calculator expression with its evaluated result.
    Expression { input: String, display: String },
    /// A recent file path.
    Path(String),
}

/// Whether the launcher window is visible.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visibility {
    Hidden,
    Visible,
}

/// Frames to wait before allowing focus-loss hiding, giving the OS
/// time to process our foreground/focus request.
const FOCUS_GRACE_FRAMES: u32 = 5;

/// State machine for the launcher window.
pub struct WindowState {
    pub input: String,
    pub selected: usize,
    pub visibility: Visibility,
    /// Whether the window had focus on the previous frame.
    /// Used to detect focus *loss* (focused → unfocused) without
    /// false-triggering on the first frame before the OS grants focus.
    pub had_focus: bool,
    /// Frames elapsed since the window became visible.
    visible_frames: u32,
    items: Vec<ListItem>,
    filtered_count: usize,
    /// Recently executed item keys (most recent first), used to show
    /// recents when input is empty.
    recent_keys: Vec<String>,
}

impl WindowState {
    pub fn new(items: Vec<ListItem>, recent_keys: Vec<String>) -> Self {
        let mut s = Self {
            input: String::new(),
            selected: 0,
            visibility: Visibility::Hidden,
            had_focus: false,
            visible_frames: 0,
            items,
            filtered_count: 0,
            recent_keys,
        };
        s.update_filtered_count();
        s
    }

    /// Show the window: clear input, reset selection, set visible.
    pub fn show(&mut self) {
        self.input.clear();
        self.selected = 0;
        self.visibility = Visibility::Visible;
        self.had_focus = false;
        self.visible_frames = 0;
        self.update_filtered_count();
    }

    /// Hide the window.
    pub fn hide(&mut self) {
        self.visibility = Visibility::Hidden;
    }

    /// Track focus and hide on loss. Call once per frame with the current
    /// viewport focus state. Only hides when transitioning focused → unfocused
    /// (avoids hiding on the first frame before the OS grants focus).
    pub fn hide_on_focus_loss(&mut self, focused: bool) {
        if self.visibility != Visibility::Visible {
            return;
        }
        self.visible_frames = self.visible_frames.saturating_add(1);
        // During the grace period, track focus but don't hide — the OS may
        // still be processing our SetForegroundWindow / Focus request.
        if self.visible_frames <= FOCUS_GRACE_FRAMES {
            self.had_focus = focused;
            return;
        }
        if self.had_focus && !focused {
            self.hide();
            return;
        }
        self.had_focus = focused;
    }

    #[allow(dead_code)]
    /// Toggle visibility.
    pub fn toggle(&mut self) {
        match self.visibility {
            Visibility::Hidden => self.show(),
            Visibility::Visible => self.hide(),
        }
    }

    /// Update the input text and reset selection.
    pub fn set_input(&mut self, input: String) {
        self.input = input;
        self.selected = 0;
        self.update_filtered_count();
    }

    /// Move selection down, clamped to the given count (or filtered_count if None).
    pub fn navigate_down_bounded(&mut self, count: usize) {
        if self.selected < count.saturating_sub(1) {
            self.selected += 1;
        }
    }

    /// Move selection down within the filtered item list.
    pub fn navigate_down(&mut self) {
        self.navigate_down_bounded(self.filtered_count);
    }

    /// Move selection up.
    pub fn navigate_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Determine the current input mode.
    pub fn input_mode(&self) -> InputMode {
        if let Some(expr) = self.input.strip_prefix('=') {
            let expr = expr.trim();
            if expr.is_empty() {
                InputMode::Calculator {
                    result: CalcResult::Invalid,
                }
            } else {
                InputMode::Calculator {
                    result: eval_calc_input(expr),
                }
            }
        } else if is_file_path(&self.input) {
            InputMode::FilePath
        } else {
            InputMode::Normal
        }
    }

    #[allow(dead_code)]
    /// Get the currently filtered items (item-only, excludes non-item recents).
    /// When input is empty and recent history exists, returns only recent
    /// items (validated against the current item list). Otherwise uses
    /// standard full-list filtering.
    pub fn filtered_items(&self) -> Vec<&ListItem> {
        if self.input.is_empty() && !self.recent_keys.is_empty() {
            let recent: Vec<&ListItem> = self
                .recent_keys
                .iter()
                .filter_map(|key| self.items.iter().find(|item| item.key == *key))
                .collect();
            if !recent.is_empty() {
                return recent;
            }
        }
        filter_items(&self.items, &self.input)
    }

    /// Get the full filtered entry list including non-item recents
    /// (expressions, file paths). Used for GUI display and navigation.
    pub fn filtered_entries(&self) -> Vec<FilteredEntry> {
        if self.input.is_empty() && !self.recent_keys.is_empty() {
            let ordered = order_recent_keys(&self.recent_keys, &self.items);
            let entries: Vec<FilteredEntry> = ordered
                .iter()
                .filter_map(|key| {
                    // Known list item
                    if let Some(item) = self.items.iter().find(|item| item.key == *key) {
                        return Some(FilteredEntry::Item(item.clone()));
                    }
                    // Calculator expression (only valid ones)
                    if let Some(expr) = key.strip_prefix('=') {
                        if let CalcResult::Ok(value) = eval_calc_input(expr.trim()) {
                            return Some(FilteredEntry::Expression {
                                input: key.clone(),
                                display: format!("{} -> {}", key, value),
                            });
                        }
                        return None;
                    }
                    // File path
                    if is_file_path(key) {
                        return Some(FilteredEntry::Path(key.clone()));
                    }
                    None
                })
                .collect();
            if !entries.is_empty() {
                return entries;
            }
        }
        filter_items(&self.items, &self.input)
            .into_iter()
            .map(|item| FilteredEntry::Item(item.clone()))
            .collect()
    }

    #[allow(dead_code)]
    /// Get the number of filtered items.
    pub fn filtered_count(&self) -> usize {
        self.filtered_count
    }

    #[allow(dead_code)]
    /// Replace the items list (e.g., when config changes).
    pub fn set_items(&mut self, items: Vec<ListItem>) {
        self.items = items;
        self.selected = 0;
        self.update_filtered_count();
    }

    #[allow(dead_code)]
    /// Update the recent-history keys (e.g., after the daemon executes an action).
    pub fn set_recent_keys(&mut self, keys: Vec<String>) {
        self.recent_keys = keys;
        self.update_filtered_count();
    }

    /// Return the action input string for the currently selected Item entry.
    /// Preserves user-typed args when the first word matches the item key exactly.
    /// Returns `None` when the selection is empty, out-of-bounds, or a non-Item entry
    /// (Expression/Path are handled separately by the caller).
    pub fn selected_action_input(&self) -> Option<String> {
        let entries = self.filtered_entries();
        if entries.is_empty() || self.selected >= entries.len() {
            return None;
        }
        match &entries[self.selected] {
            FilteredEntry::Item(item) => {
                let keyword = self.input.split_whitespace().next().unwrap_or("");
                if keyword.eq_ignore_ascii_case(&item.key) {
                    Some(self.input.clone())
                } else {
                    Some(item.key.clone())
                }
            }
            _ => None,
        }
    }

    /// Append additional items (e.g., shortcuts loaded asynchronously).
    /// Preserves the current input and selection.
    pub fn append_items(&mut self, new_items: Vec<ListItem>) {
        self.items.extend(new_items);
        self.update_filtered_count();
    }

    fn update_filtered_count(&mut self) {
        self.filtered_count = self.filtered_entries().len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::{ListItem, ListItemKind};

    fn make_item(key: &str) -> ListItem {
        ListItem {
            key: key.to_string(),
            display_detail: format!("https://{}.com/", key),
            kind: ListItemKind::Command,
            pinned: false,
        }
    }

    fn make_pinned_item(key: &str) -> ListItem {
        ListItem {
            key: key.to_string(),
            display_detail: format!("https://{}.com/", key),
            kind: ListItemKind::Command,
            pinned: true,
        }
    }

    fn sample_items() -> Vec<ListItem> {
        vec![make_item("github"), make_item("jira"), make_item("slack")]
    }

    /// Show state built from `recents` and assert all three sample items survive.
    fn assert_all_items_shown(recents: Vec<String>) {
        let mut state = WindowState::new(sample_items(), recents);
        state.show();
        assert_eq!(state.filtered_items().len(), 3);
    }

    /// Show state where pinned "github" should lead the accessed "jira",
    /// regardless of which entries appear in `recents`.
    fn assert_pinned_github_leads(recents: Vec<String>) {
        let items = vec![make_pinned_item("github"), make_item("jira")];
        let mut state = WindowState::new(items, recents);
        state.show();
        let entries = state.filtered_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], FilteredEntry::Item(make_pinned_item("github")));
        assert_eq!(entries[1], FilteredEntry::Item(make_item("jira")));
    }

    /// Sample items plus a single-letter "g" command (used by args tests).
    fn sample_items_with_g() -> Vec<ListItem> {
        let mut items = sample_items();
        items.push(ListItem {
            key: "g".to_string(),
            display_detail: "https://google.com/search?q=".to_string(),
            kind: ListItemKind::Command,
            pinned: false,
        });
        items
    }

    // --- Visibility states ---

    #[test]
    fn initial_state_is_hidden() {
        let state = WindowState::new(sample_items(), vec![]);
        assert_eq!(state.visibility, Visibility::Hidden);
    }

    #[test]
    fn show_sets_visible() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        assert_eq!(state.visibility, Visibility::Visible);
    }

    #[test]
    fn hide_sets_hidden() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.hide();
        assert_eq!(state.visibility, Visibility::Hidden);
    }

    #[test]
    fn toggle_flips_visibility() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.toggle();
        assert_eq!(state.visibility, Visibility::Visible);
        state.toggle();
        assert_eq!(state.visibility, Visibility::Hidden);
    }

    // --- Show clears state ---

    #[test]
    fn show_clears_input() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("github".to_string());
        state.show();
        assert_eq!(state.input, "");
    }

    #[test]
    fn show_resets_selection() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.navigate_down();
        state.navigate_down();
        assert_eq!(state.selected, 2);
        state.show();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn show_populates_all_items() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        assert_eq!(state.filtered_count(), 3);
    }

    // --- Filtering ---

    #[test]
    fn empty_input_shows_all() {
        assert_all_items_shown(vec![]);
    }

    #[test]
    fn typing_filters_items() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("git".to_string());
        assert_eq!(state.filtered_count(), 1);
        assert_eq!(state.filtered_items()[0].key, "github");
    }

    #[test]
    fn typing_resets_selection() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.navigate_down();
        assert_eq!(state.selected, 1);
        state.set_input("j".to_string());
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn no_matches_returns_empty() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("nonexistent".to_string());
        assert_eq!(state.filtered_count(), 0);
        assert!(state.filtered_items().is_empty());
    }

    // --- Navigation ---

    #[test]
    fn navigate_down_increments_selection() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.navigate_down();
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn navigate_down_clamps_at_bottom() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.navigate_down();
        state.navigate_down();
        state.navigate_down();
        state.navigate_down(); // past end
        assert_eq!(state.selected, 2); // clamped to last item
    }

    #[test]
    fn navigate_up_decrements_selection() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.navigate_down();
        state.navigate_down();
        state.navigate_up();
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn navigate_up_clamps_at_top() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.navigate_up(); // already at 0
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn navigate_within_filtered_list() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("sl".to_string()); // matches "slack" only
        assert_eq!(state.filtered_count(), 1);
        state.navigate_down(); // can't go past the single item
        assert_eq!(state.selected, 0);
    }

    // --- Selected item ---

    #[test]
    fn selected_item_from_filtered() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.navigate_down();
        let filtered = state.filtered_items();
        assert_eq!(filtered[state.selected].key, "jira");
    }

    // --- Input modes ---

    #[test]
    fn input_mode_normal_by_default() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        assert_eq!(state.input_mode(), InputMode::Normal);
    }

    #[test]
    fn input_mode_normal_with_text() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("github".to_string());
        assert_eq!(state.input_mode(), InputMode::Normal);
    }

    #[test]
    fn input_mode_calculator_empty_expr() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("=".to_string());
        assert_eq!(
            state.input_mode(),
            InputMode::Calculator {
                result: CalcResult::Invalid
            }
        );
    }

    #[test]
    fn input_mode_calculator_valid_expr() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("=5+3".to_string());
        assert_eq!(
            state.input_mode(),
            InputMode::Calculator {
                result: CalcResult::Ok("8".to_string())
            }
        );
    }

    #[test]
    fn input_mode_calculator_invalid_expr() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("=abc".to_string());
        assert_eq!(
            state.input_mode(),
            InputMode::Calculator {
                result: CalcResult::Invalid
            }
        );
    }

    #[test]
    fn input_mode_calculator_incomplete_expr() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("=5+".to_string());
        assert_eq!(
            state.input_mode(),
            InputMode::Calculator {
                result: CalcResult::Incomplete("5+".to_string())
            }
        );
    }

    #[test]
    fn input_mode_calculator_incomplete_unclosed_paren() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("=5*(3+".to_string());
        assert_eq!(
            state.input_mode(),
            InputMode::Calculator {
                result: CalcResult::Incomplete("5*(3+".to_string())
            }
        );
    }

    #[test]
    fn input_mode_file_path_drive_letter() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("C:\\Users".to_string());
        assert_eq!(state.input_mode(), InputMode::FilePath);
    }

    #[test]
    fn input_mode_file_path_unc() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("\\\\server\\share".to_string());
        assert_eq!(state.input_mode(), InputMode::FilePath);
    }

    // --- set_items ---

    #[test]
    fn set_items_replaces_and_resets() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.navigate_down();
        state.set_items(vec![ListItem {
            key: "new".to_string(),
            display_detail: String::new(),
            kind: ListItemKind::Command,
            pinned: false,
        }]);
        assert_eq!(state.selected, 0);
        assert_eq!(state.filtered_count(), 1);
    }

    // --- Recent items ---

    #[test]
    fn empty_input_with_recents_shows_only_recents() {
        let recents = vec!["jira".to_string(), "slack".to_string()];
        let mut state = WindowState::new(sample_items(), recents);
        state.show();
        let filtered = state.filtered_items();
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].key, "jira");
        assert_eq!(filtered[1].key, "slack");
    }

    #[test]
    fn typing_with_recents_filters_full_list() {
        let recents = vec!["jira".to_string()];
        let mut state = WindowState::new(sample_items(), recents);
        state.show();
        state.set_input("git".to_string());
        let filtered = state.filtered_items();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].key, "github");
    }

    #[test]
    fn recents_excludes_missing_items() {
        let recents = vec!["jira".to_string(), "deleted".to_string()];
        let mut state = WindowState::new(sample_items(), recents);
        state.show();
        let filtered = state.filtered_items();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].key, "jira");
    }

    #[test]
    fn empty_recents_shows_all_items() {
        assert_all_items_shown(vec![]);
    }

    #[test]
    fn all_recents_invalid_falls_back_to_full_list() {
        assert_all_items_shown(vec!["deleted1".to_string(), "deleted2".to_string()]);
    }

    // --- Filtered entries (expressions & paths in recents) ---

    /// Build state from recents, show it, and return entries.
    fn entries_for(recents: Vec<String>) -> Vec<FilteredEntry> {
        let mut state = WindowState::new(sample_items(), recents);
        state.show();
        state.filtered_entries()
    }

    fn mixed_recents() -> Vec<String> {
        vec![
            "jira".to_string(),
            "=5+3".to_string(),
            "C:\\path".to_string(),
        ]
    }

    #[test]
    fn entries_expression_recent_appears() {
        let entries = entries_for(vec!["=5+3".to_string()]);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0],
            FilteredEntry::Expression {
                input: "=5+3".to_string(),
                display: "=5+3 -> 8".to_string(),
            }
        );
    }

    #[test]
    fn entries_file_path_recent_appears() {
        let entries = entries_for(vec!["C:\\Users\\test\\file.txt".to_string()]);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0],
            FilteredEntry::Path("C:\\Users\\test\\file.txt".to_string())
        );
    }

    #[test]
    fn entries_mixed_recents_preserves_order() {
        let entries = entries_for(vec![
            "jira".to_string(),
            "=10*2".to_string(),
            "C:\\temp\\notes.md".to_string(),
            "github".to_string(),
        ]);
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0], FilteredEntry::Item(make_item("jira")));
        assert_eq!(
            entries[1],
            FilteredEntry::Expression {
                input: "=10*2".to_string(),
                display: "=10*2 -> 20".to_string(),
            }
        );
        assert_eq!(
            entries[2],
            FilteredEntry::Path("C:\\temp\\notes.md".to_string())
        );
        assert_eq!(entries[3], FilteredEntry::Item(make_item("github")));
    }

    #[test]
    fn entries_pinned_recent_pulled_to_front() {
        // "github" is pinned but used less recently than "jira"; it must lead.
        assert_pinned_github_leads(vec!["jira".to_string(), "github".to_string()]);
    }

    #[test]
    fn entries_never_accessed_pin_injected_at_top() {
        // "github" is pinned but never accessed; it must lead, ahead of the
        // accessed non-pinned "jira".
        assert_pinned_github_leads(vec!["jira".to_string()]);
    }

    #[test]
    fn entries_invalid_expression_excluded() {
        let entries = entries_for(vec!["=abc".to_string(), "jira".to_string()]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], FilteredEntry::Item(make_item("jira")));
    }

    #[test]
    fn entries_count_includes_non_item_recents() {
        let mut state = WindowState::new(sample_items(), mixed_recents());
        state.show();
        assert_eq!(state.filtered_count(), 3);
    }

    #[test]
    fn entries_navigate_clamps_with_mixed_recents() {
        let mut state =
            WindowState::new(sample_items(), vec!["jira".to_string(), "=5+3".to_string()]);
        state.show();
        assert_eq!(state.filtered_count(), 2);
        state.navigate_down();
        state.navigate_down(); // past end
        assert_eq!(state.selected, 1); // clamped
    }

    #[test]
    fn entries_typing_after_mixed_recents_filters_items_only() {
        let mut state = WindowState::new(sample_items(), mixed_recents());
        state.show();
        state.set_input("git".to_string());
        let entries = state.filtered_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], FilteredEntry::Item(make_item("github")));
    }

    // --- Input with args preserves full text (regression: backlog #11) ---

    /// When a user types "g some text", the GUI must filter to the "g" item
    /// while preserving the full input so execute_action receives the args.
    #[test]
    fn input_with_args_filters_and_preserves_full_input() {
        let mut state = WindowState::new(sample_items_with_g(), vec![]);
        state.show();
        state.set_input("g some text".to_string());
        let entries = state.filtered_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0],
            FilteredEntry::Item(ListItem {
                key: "g".to_string(),
                display_detail: "https://google.com/search?q=".to_string(),
                kind: ListItemKind::Command,
                pinned: false,
            })
        );
        // The full input (key + args) must be available for execute_action
        assert_eq!(state.input, "g some text");
    }

    // --- selected_action_input (regression: backlog #12) ---

    /// Pressing Enter after arrowing down must open the highlighted item,
    /// not the first item. This was broken because the GUI passed raw input
    /// text to resolve_item instead of using the selected entry's key.
    #[test]
    fn selected_action_input_uses_highlighted_item() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        // Arrow down once — "jira" is second in [github, jira, slack]
        state.navigate_down();
        let action = state.selected_action_input().unwrap();
        assert_eq!(action, "jira");
    }

    #[test]
    fn selected_action_input_first_item_without_navigation() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        let action = state.selected_action_input().unwrap();
        assert_eq!(action, "github");
    }

    #[test]
    fn selected_action_input_preserves_args() {
        let mut state = WindowState::new(sample_items_with_g(), vec![]);
        state.show();
        state.set_input("g some text".to_string());
        let action = state.selected_action_input().unwrap();
        assert_eq!(action, "g some text");
    }

    #[test]
    fn selected_action_input_partial_filter_uses_key() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("ji".to_string());
        // "ji" partially matches "jira" — action should be the key, not "ji"
        let action = state.selected_action_input().unwrap();
        assert_eq!(action, "jira");
    }

    #[test]
    fn selected_action_input_empty_list_returns_none() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("nonexistent".to_string());
        assert!(state.selected_action_input().is_none());
    }

    #[test]
    fn selected_action_input_recent_item_uses_key() {
        let recents = vec!["slack".to_string(), "jira".to_string()];
        let mut state = WindowState::new(sample_items(), recents);
        state.show();
        // Input is empty, recents shown: [slack, jira]
        state.navigate_down(); // select "jira"
        let action = state.selected_action_input().unwrap();
        assert_eq!(action, "jira");
    }

    #[test]
    fn entries_selecting_expression_switches_to_calculator_mode() {
        let mut state = WindowState::new(sample_items(), vec!["=5+3".to_string()]);
        state.show();
        // Simulate selecting the expression (set_input like the GUI does)
        state.set_input("=5+3".to_string());
        assert_eq!(
            state.input_mode(),
            InputMode::Calculator {
                result: CalcResult::Ok("8".to_string())
            }
        );
    }
}
