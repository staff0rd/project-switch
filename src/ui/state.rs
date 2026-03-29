//! Launcher window state machine.
//!
//! Manages visibility, input, filtering, selection, and transitions
//! independently of the GUI framework for testability.

use crate::launcher::{eval_calculator, filter_items, is_file_path, ListItem};

/// The current input mode, derived from the input text.
#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    /// Normal item filtering mode.
    Normal,
    /// Calculator mode (input starts with `=`).
    Calculator { result: Result<String, String> },
    /// File path browsing mode.
    FilePath,
}

/// Whether the launcher window is visible.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visibility {
    Hidden,
    Visible,
}

/// State machine for the launcher window.
pub struct WindowState {
    pub input: String,
    pub selected: usize,
    pub visibility: Visibility,
    /// Whether the window had focus on the previous frame.
    /// Used to detect focus *loss* (focused → unfocused) without
    /// false-triggering on the first frame before the OS grants focus.
    pub had_focus: bool,
    items: Vec<ListItem>,
    filtered_count: usize,
}

impl WindowState {
    pub fn new(items: Vec<ListItem>) -> Self {
        let count = items.len();
        Self {
            input: String::new(),
            selected: 0,
            visibility: Visibility::Hidden,
            had_focus: false,
            items,
            filtered_count: count,
        }
    }

    /// Show the window: clear input, reset selection, set visible.
    pub fn show(&mut self) {
        self.input.clear();
        self.selected = 0;
        self.visibility = Visibility::Visible;
        self.had_focus = false;
        self.filtered_count = self.items.len();
    }

    /// Hide the window.
    pub fn hide(&mut self) {
        self.visibility = Visibility::Hidden;
    }

    /// Track focus and hide on loss. Call once per frame with the current
    /// viewport focus state. Only hides when transitioning focused → unfocused
    /// (avoids hiding on the first frame before the OS grants focus).
    pub fn hide_on_focus_loss(&mut self, focused: bool) {
        if self.visibility == Visibility::Visible && self.had_focus && !focused {
            self.hide();
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
                    result: Err("type an expression".to_string()),
                }
            } else {
                InputMode::Calculator {
                    result: eval_calculator(expr),
                }
            }
        } else if is_file_path(&self.input) {
            InputMode::FilePath
        } else {
            InputMode::Normal
        }
    }

    /// Get the currently filtered items.
    pub fn filtered_items(&self) -> Vec<&ListItem> {
        filter_items(&self.items, &self.input)
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

    fn update_filtered_count(&mut self) {
        self.filtered_count = filter_items(&self.items, &self.input).len();
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
        }
    }

    fn sample_items() -> Vec<ListItem> {
        vec![make_item("github"), make_item("jira"), make_item("slack")]
    }

    // --- Visibility states ---

    #[test]
    fn initial_state_is_hidden() {
        let state = WindowState::new(sample_items());
        assert_eq!(state.visibility, Visibility::Hidden);
    }

    #[test]
    fn show_sets_visible() {
        let mut state = WindowState::new(sample_items());
        state.show();
        assert_eq!(state.visibility, Visibility::Visible);
    }

    #[test]
    fn hide_sets_hidden() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.hide();
        assert_eq!(state.visibility, Visibility::Hidden);
    }

    #[test]
    fn toggle_flips_visibility() {
        let mut state = WindowState::new(sample_items());
        state.toggle();
        assert_eq!(state.visibility, Visibility::Visible);
        state.toggle();
        assert_eq!(state.visibility, Visibility::Hidden);
    }

    // --- Show clears state ---

    #[test]
    fn show_clears_input() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.set_input("github".to_string());
        state.show();
        assert_eq!(state.input, "");
    }

    #[test]
    fn show_resets_selection() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.navigate_down();
        state.navigate_down();
        assert_eq!(state.selected, 2);
        state.show();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn show_populates_all_items() {
        let mut state = WindowState::new(sample_items());
        state.show();
        assert_eq!(state.filtered_count(), 3);
    }

    // --- Filtering ---

    #[test]
    fn empty_input_shows_all() {
        let mut state = WindowState::new(sample_items());
        state.show();
        assert_eq!(state.filtered_items().len(), 3);
    }

    #[test]
    fn typing_filters_items() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.set_input("git".to_string());
        assert_eq!(state.filtered_count(), 1);
        assert_eq!(state.filtered_items()[0].key, "github");
    }

    #[test]
    fn typing_resets_selection() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.navigate_down();
        assert_eq!(state.selected, 1);
        state.set_input("j".to_string());
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn no_matches_returns_empty() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.set_input("nonexistent".to_string());
        assert_eq!(state.filtered_count(), 0);
        assert!(state.filtered_items().is_empty());
    }

    // --- Navigation ---

    #[test]
    fn navigate_down_increments_selection() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.navigate_down();
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn navigate_down_clamps_at_bottom() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.navigate_down();
        state.navigate_down();
        state.navigate_down();
        state.navigate_down(); // past end
        assert_eq!(state.selected, 2); // clamped to last item
    }

    #[test]
    fn navigate_up_decrements_selection() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.navigate_down();
        state.navigate_down();
        state.navigate_up();
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn navigate_up_clamps_at_top() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.navigate_up(); // already at 0
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn navigate_within_filtered_list() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.set_input("sl".to_string()); // matches "slack" only
        assert_eq!(state.filtered_count(), 1);
        state.navigate_down(); // can't go past the single item
        assert_eq!(state.selected, 0);
    }

    // --- Selected item ---

    #[test]
    fn selected_item_from_filtered() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.navigate_down();
        let filtered = state.filtered_items();
        assert_eq!(filtered[state.selected].key, "jira");
    }

    // --- Input modes ---

    #[test]
    fn input_mode_normal_by_default() {
        let mut state = WindowState::new(sample_items());
        state.show();
        assert_eq!(state.input_mode(), InputMode::Normal);
    }

    #[test]
    fn input_mode_normal_with_text() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.set_input("github".to_string());
        assert_eq!(state.input_mode(), InputMode::Normal);
    }

    #[test]
    fn input_mode_calculator_empty_expr() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.set_input("=".to_string());
        assert!(matches!(
            state.input_mode(),
            InputMode::Calculator { result: Err(_) }
        ));
    }

    #[test]
    fn input_mode_calculator_valid_expr() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.set_input("=5+3".to_string());
        assert_eq!(
            state.input_mode(),
            InputMode::Calculator {
                result: Ok("8".to_string())
            }
        );
    }

    #[test]
    fn input_mode_calculator_invalid_expr() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.set_input("=abc".to_string());
        assert!(matches!(
            state.input_mode(),
            InputMode::Calculator { result: Err(_) }
        ));
    }

    #[test]
    fn input_mode_file_path_drive_letter() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.set_input("C:\\Users".to_string());
        assert_eq!(state.input_mode(), InputMode::FilePath);
    }

    #[test]
    fn input_mode_file_path_unc() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.set_input("\\\\server\\share".to_string());
        assert_eq!(state.input_mode(), InputMode::FilePath);
    }

    // --- set_items ---

    #[test]
    fn set_items_replaces_and_resets() {
        let mut state = WindowState::new(sample_items());
        state.show();
        state.navigate_down();
        state.set_items(vec![ListItem {
            key: "new".to_string(),
            display_detail: String::new(),
            kind: ListItemKind::Command,
        }]);
        assert_eq!(state.selected, 0);
        assert_eq!(state.filtered_count(), 1);
    }
}
