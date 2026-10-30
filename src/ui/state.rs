//! Launcher window state machine.
//!
//! Manages visibility, input, filtering, selection, and transitions
//! independently of the GUI framework for testability.

use crate::calendar::cache::CalendarCache;
use crate::calendar::{CalendarContent, CalendarDay};
use crate::config::BrowserOption;
use crate::launcher::browser_picker::BrowserPicker;
use crate::launcher::grocery_picker::{GroceryPicker, GroceryRow};
use crate::launcher::key_combo::KeyCombo;
use crate::launcher::picker::PickerState;
use crate::launcher::{
    eval_calc_input, filter_items, is_file_path, order_recent_keys, CalcResult, ListItem,
};
use crate::paprika::GroceryCommand;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, SystemTime};

pub const CALENDAR_FRESH_FOR: Duration = Duration::from_secs(5 * 60);

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
    /// Active client/project picker; `Some` puts the launcher in picker mode.
    pub picker: Option<PickerState>,
    pub browser_picker: Option<BrowserPicker>,
    pub grocery_picker: Option<GroceryPicker>,
    /// Message shown in the launcher's error banner.
    pub error: Option<String>,
    pub calendar_open: bool,
    pub calendar: CalendarContent,
    pub calendar_refreshing: bool,
    calendar_fetched_at: Option<SystemTime>,
    action_tx: Sender<String>,
    action_rx: Receiver<String>,
    grocery_tx: Sender<Result<Vec<String>, String>>,
    grocery_rx: Receiver<Result<Vec<String>, String>>,
    calendar_tx: Sender<Result<CalendarContent, String>>,
    calendar_rx: Receiver<Result<CalendarContent, String>>,
    pending_actions: usize,
}

impl WindowState {
    pub fn new(items: Vec<ListItem>, recent_keys: Vec<String>) -> Self {
        let (action_tx, action_rx) = channel();
        let (grocery_tx, grocery_rx) = channel();
        let (calendar_tx, calendar_rx) = channel();
        let mut s = Self {
            input: String::new(),
            selected: 0,
            visibility: Visibility::Hidden,
            had_focus: false,
            visible_frames: 0,
            items,
            filtered_count: 0,
            recent_keys,
            picker: None,
            browser_picker: None,
            grocery_picker: None,
            error: None,
            calendar_open: false,
            calendar: CalendarContent::Loading,
            calendar_refreshing: false,
            calendar_fetched_at: None,
            action_tx,
            action_rx,
            grocery_tx,
            grocery_rx,
            calendar_tx,
            calendar_rx,
            pending_actions: 0,
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
        self.picker = None;
        self.browser_picker = None;
        self.grocery_picker = None;
        self.error = None;
        self.calendar_open = false;
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
        self.error = None;
        self.update_filtered_count();
    }

    /// Run `action` on a background thread, keeping the launcher responsive.
    /// A failure comes back through the channel so [`poll_actions`] can show it
    /// instead of losing it to a stderr no one sees in GUI mode.
    ///
    /// [`poll_actions`]: WindowState::poll_actions
    pub fn spawn_action<F>(&mut self, action: F)
    where
        F: FnOnce() -> anyhow::Result<()> + Send + 'static,
    {
        let tx = self.action_tx.clone();
        self.pending_actions += 1;
        std::thread::spawn(move || {
            let message = match action() {
                Ok(()) => String::new(),
                Err(e) => format!("{e:#}"),
            };
            let _ = tx.send(message);
        });
    }

    pub fn has_pending_actions(&self) -> bool {
        self.pending_actions > 0
    }

    /// Collect finished background actions, re-showing the window on failure.
    pub fn poll_actions(&mut self) {
        while let Ok(message) = self.action_rx.try_recv() {
            self.pending_actions = self.pending_actions.saturating_sub(1);
            if !message.is_empty() {
                self.set_error(message);
            }
        }
        while let Ok(result) = self.grocery_rx.try_recv() {
            self.pending_actions = self.pending_actions.saturating_sub(1);
            match result {
                Ok(items) => {
                    if let Some(picker) = self.grocery_picker.as_mut() {
                        picker.set_items(items);
                    }
                }
                Err(message) => {
                    if let Some(picker) = self.grocery_picker.as_mut() {
                        picker.loading = false;
                    }
                    self.set_error(message);
                }
            }
        }
        while let Ok(result) = self.calendar_rx.try_recv() {
            self.pending_actions = self.pending_actions.saturating_sub(1);
            self.calendar_refreshing = false;
            match result {
                Ok(content) => {
                    self.calendar = content;
                    self.calendar_fetched_at = Some(SystemTime::now());
                    self.show_calendar_errors();
                }
                Err(message) => {
                    if self.calendar == CalendarContent::Loading {
                        self.calendar = CalendarContent::Day(CalendarDay::default());
                    }
                    self.calendar_fetched_at = None;
                    if self.calendar_open {
                        self.set_error(message);
                    }
                }
            }
        }
    }

    fn show_calendar_errors(&mut self) {
        if let CalendarContent::Day(day) = &self.calendar {
            if self.calendar_open && !day.errors.is_empty() {
                self.set_error(day.errors.join("\n"));
            }
        }
    }

    fn spawn_result<T, F>(&mut self, tx: Sender<Result<T, String>>, action: F)
    where
        T: Send + 'static,
        F: FnOnce() -> anyhow::Result<T> + Send + 'static,
    {
        self.pending_actions += 1;
        std::thread::spawn(move || {
            let _ = tx.send(action().map_err(|e| format!("{e:#}")));
        });
    }

    pub fn toggle_calendar<F>(&mut self, fetch: F)
    where
        F: FnOnce() -> anyhow::Result<CalendarContent> + Send + 'static,
    {
        if self.calendar_open {
            self.close_calendar();
            return;
        }
        self.calendar_open = true;
        self.show_calendar_errors();
        let fresh = self
            .calendar_age()
            .is_some_and(|age| age < CALENDAR_FRESH_FOR);
        if fresh || self.calendar_refreshing {
            return;
        }
        self.calendar_refreshing = true;
        self.spawn_result(self.calendar_tx.clone(), fetch);
    }

    pub fn calendar_age(&self) -> Option<Duration> {
        self.calendar_fetched_at?.elapsed().ok()
    }

    pub fn restore_calendar(&mut self, cache: Option<CalendarCache>) {
        if let Some(cache) = cache {
            self.calendar = cache.content;
            self.calendar_fetched_at = Some(cache.fetched_at);
        }
    }

    pub fn close_calendar(&mut self) {
        self.calendar_open = false;
    }

    pub fn open_grocery_picker(&mut self) {
        self.error = None;
        self.grocery_picker = Some(GroceryPicker::loading());
        self.input.clear();
        self.selected = 0;
    }

    pub fn spawn_grocery<F>(&mut self, action: F)
    where
        F: FnOnce() -> anyhow::Result<Vec<String>> + Send + 'static,
    {
        if let Some(picker) = self.grocery_picker.as_mut() {
            picker.loading = true;
        }
        self.input.clear();
        self.selected = 0;
        self.spawn_result(self.grocery_tx.clone(), action);
    }

    pub fn grocery_rows(&self) -> Vec<GroceryRow> {
        self.grocery_picker
            .as_ref()
            .map(|picker| picker.rows(&self.input))
            .unwrap_or_default()
    }

    pub fn grocery_command(&self) -> Option<GroceryCommand> {
        self.grocery_picker
            .as_ref()?
            .command(&self.input, self.selected)
    }

    /// Show `message` in the error banner, making the window visible again if
    /// it was hidden by the action that failed.
    pub fn set_error(&mut self, message: String) {
        self.visibility = Visibility::Visible;
        self.had_focus = false;
        self.visible_frames = 0;
        self.error = Some(message);
    }

    /// Enter client/project picker mode.
    pub fn open_picker(&mut self, picker: PickerState) {
        self.error = None;
        self.picker = Some(picker);
        self.reset_picker_cursor();
    }

    pub fn open_browser_picker(&mut self, picker: BrowserPicker) {
        self.error = None;
        self.browser_picker = Some(picker);
        self.input.clear();
        self.selected = 0;
    }

    pub fn browser_entries(&self) -> Vec<BrowserOption> {
        self.browser_picker
            .as_ref()
            .map(|picker| picker.entries(&self.input))
            .unwrap_or_default()
    }

    pub fn selected_browser(&self) -> Option<BrowserOption> {
        self.browser_entries().get(self.selected).cloned()
    }

    /// Leave picker mode, returning to the normal item list.
    pub fn close_picker(&mut self) {
        self.picker = None;
        self.browser_picker = None;
        self.grocery_picker = None;
        self.input.clear();
        self.selected = 0;
        self.update_filtered_count();
    }

    /// Step the picker back one stage. Returns `false` when already at the
    /// client list, meaning the caller should leave picker mode.
    pub fn picker_back(&mut self) -> bool {
        let stepped = self.picker.as_mut().is_some_and(|picker| picker.back());
        if stepped {
            self.reset_picker_cursor();
        }
        stepped
    }

    /// Clear the picker filter and put the cursor on the current selection.
    pub fn reset_picker_cursor(&mut self) {
        self.input.clear();
        self.selected = self.picker.as_ref().map(|p| p.cursor("")).unwrap_or(0);
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

    /// The [`ListItem`] behind the current selection, or `None` when the
    /// selection is empty, out-of-bounds, or a non-Item entry.
    pub fn selected_item(&self) -> Option<ListItem> {
        let entries = self.filtered_entries();
        match entries.get(self.selected)? {
            FilteredEntry::Item(item) => Some(item.clone()),
            _ => None,
        }
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

    pub fn hotkey_item(&self, combo: KeyCombo) -> Option<ListItem> {
        self.items
            .iter()
            .find(|item| item.hotkey == Some(combo))
            .cloned()
    }

    pub fn selected_completion(&self) -> Option<String> {
        let key = self.selected_item()?.key;
        if key.contains(char::is_whitespace) {
            Some(key)
        } else {
            Some(format!("{} ", key))
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
            hotkey: None,
        }
    }

    fn make_pinned_item(key: &str) -> ListItem {
        ListItem {
            key: key.to_string(),
            display_detail: format!("https://{}.com/", key),
            kind: ListItemKind::Command,
            pinned: true,
            hotkey: None,
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
            hotkey: None,
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
            hotkey: None,
        }]);
        assert_eq!(state.selected, 0);
        assert_eq!(state.filtered_count(), 1);
    }

    #[test]
    fn hotkey_item_finds_bound_item_regardless_of_filter() {
        let combo: KeyCombo = "alt+j".parse().unwrap();
        let mut items = sample_items();
        items[1].hotkey = Some(combo);
        let mut state = WindowState::new(items, vec![]);
        state.show();
        state.set_input("git".to_string());
        assert_eq!(state.hotkey_item(combo).unwrap().key, "jira");
        assert!(state.hotkey_item("alt+k".parse().unwrap()).is_none());
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
                hotkey: None,
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
    fn selected_completion_partial_key_adds_trailing_space() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("ji".to_string());
        assert_eq!(state.selected_completion().as_deref(), Some("jira "));
    }

    #[test]
    fn selected_completion_uses_highlighted_not_first() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.navigate_down();
        assert_eq!(state.selected_completion().as_deref(), Some("jira "));
    }

    #[test]
    fn selected_completion_multi_word_key_has_no_trailing_space() {
        let mut items = sample_items();
        items.push(make_item("Visual Studio Code"));
        let mut state = WindowState::new(items, vec![]);
        state.show();
        state.set_input("visual".to_string());
        let completion = state.selected_completion().unwrap();
        assert_eq!(completion, "Visual Studio Code");
        state.set_input(completion);
        assert_eq!(
            state.selected_action_input().as_deref(),
            Some("Visual Studio Code")
        );
    }

    #[test]
    fn selected_completion_empty_list_returns_none() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("nonexistent".to_string());
        assert!(state.selected_completion().is_none());
    }

    #[test]
    fn selected_completion_non_item_recent_returns_none() {
        let mut state = WindowState::new(sample_items(), vec!["=1+1".to_string()]);
        state.show();
        assert!(state.selected_completion().is_none());
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

    // --- Picker mode (backlog a935) ---

    use crate::launcher::picker::PickerClient;

    fn switch_item() -> ListItem {
        ListItem {
            key: "switch".to_string(),
            display_detail: "project-switch.exe switch".to_string(),
            kind: ListItemKind::Switch,
            pinned: false,
            hotkey: None,
        }
    }

    fn sample_picker() -> PickerState {
        PickerState::new(
            vec![
                PickerClient {
                    name: "nero".to_string(),
                    projects: vec![],
                },
                PickerClient {
                    name: "EventsAir".to_string(),
                    projects: vec!["Build & Deploy".to_string()],
                },
            ],
            Some("EventsAir".to_string()),
            None,
        )
    }

    /// State showing the sample items plus a `switch` command, filtered to it.
    fn state_on_switch_item() -> WindowState {
        let mut items = sample_items();
        items.push(switch_item());
        let mut state = WindowState::new(items, vec![]);
        state.show();
        state.set_input("switch".to_string());
        state
    }

    fn state_in_picker() -> WindowState {
        let mut state = state_on_switch_item();
        state.open_picker(sample_picker());
        state
    }

    #[test]
    fn selected_item_exposes_switch_kind() {
        let state = state_on_switch_item();
        assert_eq!(state.selected_item().unwrap().kind, ListItemKind::Switch);
    }

    #[test]
    fn open_picker_clears_filter_and_targets_current_client() {
        let state = state_in_picker();
        assert!(state.picker.is_some());
        assert_eq!(state.input, "");
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn picker_back_from_projects_returns_to_clients() {
        let mut state = state_in_picker();
        let entries = state.picker.as_ref().unwrap().entries("");
        state.picker.as_mut().unwrap().activate(&entries[1]);
        assert!(state.picker_back());
        assert!(state.picker.is_some());
    }

    #[test]
    fn picker_back_at_client_stage_reports_top() {
        let mut state = state_in_picker();
        assert!(!state.picker_back());
    }

    #[test]
    fn close_picker_restores_normal_list() {
        let mut state = state_in_picker();
        state.close_picker();
        assert!(state.picker.is_none());
        assert_eq!(state.input, "");
        assert_eq!(state.selected, 0);
        assert_eq!(state.filtered_count(), 4);
    }

    #[test]
    fn show_leaves_picker_mode() {
        let mut state = state_in_picker();
        state.show();
        assert!(state.picker.is_none());
    }

    fn browser_option(label: &str) -> BrowserOption {
        BrowserOption {
            label: label.to_string(),
            value: format!("firefox -P {label}"),
        }
    }

    fn state_in_browser_picker() -> WindowState {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("jira PROJ-1".to_string());
        let action = state.selected_action_input().unwrap();
        state.open_browser_picker(BrowserPicker::new(
            "jira".to_string(),
            action,
            vec![
                browser_option("integrator"),
                browser_option("coach"),
                browser_option("admin"),
            ],
        ));
        state
    }

    fn browser_labels(state: &WindowState) -> Vec<String> {
        state
            .browser_entries()
            .into_iter()
            .map(|option| option.label)
            .collect()
    }

    #[test]
    fn open_browser_picker_clears_filter_and_keeps_typed_args() {
        let state = state_in_browser_picker();
        assert_eq!(state.input, "");
        assert_eq!(state.selected, 0);
        assert_eq!(browser_labels(&state), vec!["integrator", "coach", "admin"]);
        assert_eq!(
            state.browser_picker.as_ref().unwrap().action_input,
            "jira PROJ-1"
        );
    }

    #[test]
    fn navigating_browser_picker_selects_entry() {
        let mut state = state_in_browser_picker();
        let count = state.browser_entries().len();
        state.navigate_down_bounded(count);
        assert_eq!(state.selected_browser(), Some(browser_option("coach")));
        state.navigate_down_bounded(count);
        state.navigate_down_bounded(count);
        assert_eq!(state.selected_browser(), Some(browser_option("admin")));
        state.navigate_up();
        assert_eq!(state.selected_browser(), Some(browser_option("coach")));
    }

    #[test]
    fn typing_filters_browser_picker() {
        let mut state = state_in_browser_picker();
        state.set_input("coa".to_string());
        assert_eq!(browser_labels(&state), vec!["coach"]);
        assert_eq!(state.selected_browser(), Some(browser_option("coach")));
    }

    #[test]
    fn close_picker_leaves_browser_picker_for_item_list() {
        let mut state = state_in_browser_picker();
        state.close_picker();
        assert!(state.browser_picker.is_none());
        assert_eq!(state.input, "");
        assert_eq!(state.filtered_count(), 3);
        assert!(state.browser_entries().is_empty());
    }

    #[test]
    fn show_leaves_browser_picker_mode() {
        let mut state = state_in_browser_picker();
        state.show();
        assert!(state.browser_picker.is_none());
    }

    fn grocery_items() -> Vec<String> {
        vec![
            "Milk".to_string(),
            "Oat milk".to_string(),
            "Eggs".to_string(),
        ]
    }

    fn settle(state: &mut WindowState) {
        while state.has_pending_actions() {
            state.poll_actions();
        }
    }

    fn state_in_grocery_picker() -> WindowState {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_input("grocery".to_string());
        state.open_grocery_picker();
        state.spawn_grocery(|| Ok(grocery_items()));
        settle(&mut state);
        state
    }

    #[test]
    fn grocery_picker_loads_items_asynchronously() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.open_grocery_picker();
        assert!(state.grocery_picker.as_ref().unwrap().loading);
        assert_eq!(state.grocery_command(), None);
        state.spawn_grocery(|| Ok(grocery_items()));
        settle(&mut state);
        assert!(!state.grocery_picker.as_ref().unwrap().loading);
        assert_eq!(state.grocery_rows().len(), 3);
    }

    #[test]
    fn enter_on_highlighted_grocery_item_removes_it() {
        let mut state = state_in_grocery_picker();
        state.set_input("milk".to_string());
        state.navigate_down_bounded(state.grocery_rows().len());
        assert_eq!(
            state.grocery_command(),
            Some(GroceryCommand::Remove {
                item: "Oat milk".to_string(),
                list: None
            })
        );
    }

    #[test]
    fn enter_on_unmatched_grocery_filter_adds_it() {
        let mut state = state_in_grocery_picker();
        state.set_input("Bread".to_string());
        assert_eq!(state.grocery_rows(), [GroceryRow::Add("Bread".to_string())]);
        assert_eq!(
            state.grocery_command(),
            Some(GroceryCommand::Add {
                item: "Bread".to_string(),
                list: None
            })
        );
    }

    #[test]
    fn grocery_change_clears_filter_and_refreshes_list() {
        let mut state = state_in_grocery_picker();
        state.set_input("eggs".to_string());
        state.spawn_grocery(|| Ok(vec!["Milk".to_string()]));
        assert_eq!(state.input, "");
        assert!(state.grocery_picker.as_ref().unwrap().loading);
        settle(&mut state);
        assert_eq!(state.grocery_rows(), [GroceryRow::Item("Milk".to_string())]);
    }

    #[test]
    fn failed_grocery_call_shows_error_and_keeps_picker() {
        let mut state = state_in_grocery_picker();
        state.spawn_grocery(|| anyhow::bail!(crate::paprika::AUTH_HINT));
        settle(&mut state);
        assert_eq!(state.error.as_deref(), Some(crate::paprika::AUTH_HINT));
        let picker = state.grocery_picker.as_ref().unwrap();
        assert!(!picker.loading);
        assert_eq!(state.grocery_rows().len(), 3);
    }

    #[test]
    fn close_picker_leaves_grocery_picker_for_item_list() {
        let mut state = state_in_grocery_picker();
        state.set_input("milk".to_string());
        state.close_picker();
        assert!(state.grocery_picker.is_none());
        assert_eq!(state.input, "");
        assert_eq!(state.filtered_count(), 3);
    }

    #[test]
    fn grocery_result_after_close_is_ignored() {
        let mut state = state_in_grocery_picker();
        state.spawn_grocery(|| Ok(grocery_items()));
        state.close_picker();
        settle(&mut state);
        assert!(state.grocery_picker.is_none());
        assert!(state.error.is_none());
    }

    fn state_with_calendar_open() -> WindowState {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.toggle_calendar(|| Ok(CalendarContent::NotConfigured));
        state
    }

    #[test]
    fn opening_calendar_loads_in_the_background() {
        let mut state = state_with_calendar_open();
        assert!(state.calendar_open);
        assert_eq!(state.calendar, CalendarContent::Loading);
        settle(&mut state);
        assert_eq!(state.calendar, CalendarContent::NotConfigured);
    }

    #[test]
    fn toggling_again_closes_the_calendar() {
        let mut state = state_with_calendar_open();
        state.toggle_calendar(|| panic!("closing must not fetch"));
        assert!(!state.calendar_open);
        settle(&mut state);
    }

    #[test]
    fn calendar_keeps_list_filtering_working() {
        let mut state = state_with_calendar_open();
        state.set_input("ji".to_string());
        assert_eq!(state.selected_action_input().as_deref(), Some("jira"));
        assert!(state.calendar_open);
    }

    #[test]
    fn failed_calendar_fetch_shows_error_banner() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.toggle_calendar(|| anyhow::bail!("Calendar 'work': boom"));
        settle(&mut state);
        assert_eq!(state.error.as_deref(), Some("Calendar 'work': boom"));
        assert_eq!(state.calendar, CalendarContent::Day(CalendarDay::default()));
    }

    #[test]
    fn failed_accounts_show_in_the_banner_beside_rendered_ones() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        let day = CalendarDay {
            errors: vec![
                "Calendar 'work': boom".to_string(),
                "Calendar 'family': bang".to_string(),
            ],
            ..CalendarDay::default()
        };
        let fetched = CalendarContent::Day(day);
        let expected = fetched.clone();
        state.toggle_calendar(move || Ok(fetched));
        settle(&mut state);
        assert_eq!(
            state.error.as_deref(),
            Some("Calendar 'work': boom\nCalendar 'family': bang")
        );
        assert_eq!(state.calendar, expected);
    }

    #[test]
    fn calendar_error_after_close_is_not_shown() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.toggle_calendar(|| anyhow::bail!("boom"));
        state.close_calendar();
        settle(&mut state);
        assert!(state.error.is_none());
    }

    fn reopen_calendar<F>(state: &mut WindowState, fetch: F)
    where
        F: FnOnce() -> anyhow::Result<CalendarContent> + Send + 'static,
    {
        state.close_calendar();
        state.show();
        state.toggle_calendar(fetch);
    }

    fn stale_calendar(state: &mut WindowState) {
        state.calendar_fetched_at = SystemTime::now().checked_sub(CALENDAR_FRESH_FOR);
    }

    fn day_with_errors(errors: &[&str]) -> CalendarContent {
        CalendarContent::Day(CalendarDay {
            errors: errors.iter().map(|e| e.to_string()).collect(),
            ..CalendarDay::default()
        })
    }

    #[test]
    fn reopening_within_five_minutes_uses_the_cache() {
        let mut state = state_with_calendar_open();
        settle(&mut state);
        reopen_calendar(&mut state, || panic!("fresh cache must not refetch"));
        assert_eq!(state.calendar, CalendarContent::NotConfigured);
        assert!(!state.calendar_refreshing);
        assert!(!state.has_pending_actions());
    }

    #[test]
    fn stale_cache_stays_visible_while_refreshing() {
        let mut state = state_with_calendar_open();
        settle(&mut state);
        stale_calendar(&mut state);
        let refreshed = day_with_errors(&[]);
        let expected = refreshed.clone();
        reopen_calendar(&mut state, move || Ok(refreshed));
        assert_eq!(state.calendar, CalendarContent::NotConfigured);
        assert!(state.calendar_refreshing);
        settle(&mut state);
        assert_eq!(state.calendar, expected);
        assert!(!state.calendar_refreshing);
        assert!(state.calendar_age().unwrap() < CALENDAR_FRESH_FOR);
    }

    #[test]
    fn failed_refresh_keeps_stale_events_and_retries_next_open() {
        let mut state = state_with_calendar_open();
        settle(&mut state);
        stale_calendar(&mut state);
        reopen_calendar(&mut state, || anyhow::bail!("offline"));
        settle(&mut state);
        assert_eq!(state.calendar, CalendarContent::NotConfigured);
        assert_eq!(state.error.as_deref(), Some("offline"));
        assert!(state.calendar_age().is_none());
    }

    #[test]
    fn cached_account_errors_show_again_on_reopen() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        let fetched = day_with_errors(&["Calendar 'work': boom"]);
        state.toggle_calendar(move || Ok(fetched));
        settle(&mut state);
        reopen_calendar(&mut state, || panic!("fresh cache must not refetch"));
        assert_eq!(state.error.as_deref(), Some("Calendar 'work': boom"));
    }

    fn cache_fetched(ago: Duration, content: CalendarContent) -> Option<CalendarCache> {
        Some(CalendarCache {
            fetched_at: SystemTime::now() - ago,
            content,
        })
    }

    #[test]
    fn fresh_cache_from_an_earlier_process_shows_without_refetching() {
        let mut state = WindowState::new(sample_items(), vec![]);
        let cached = day_with_errors(&[]);
        state.restore_calendar(cache_fetched(Duration::from_secs(120), cached.clone()));
        state.show();
        state.toggle_calendar(|| panic!("fresh cache must not refetch"));
        assert_eq!(state.calendar, cached);
        assert!(!state.calendar_refreshing);
        assert!(!state.has_pending_actions());
        assert_eq!(state.calendar_age().unwrap().as_secs() / 60, 2);
    }

    #[test]
    fn stale_cache_from_an_earlier_process_shows_while_refreshing() {
        let mut state = WindowState::new(sample_items(), vec![]);
        let cached = day_with_errors(&[]);
        state.restore_calendar(cache_fetched(CALENDAR_FRESH_FOR, cached.clone()));
        state.show();
        state.toggle_calendar(|| Ok(day_with_errors(&["Calendar 'work': boom"])));
        assert_eq!(state.calendar, cached);
        assert!(state.calendar_refreshing);
        settle(&mut state);
        assert_eq!(state.calendar, day_with_errors(&["Calendar 'work': boom"]));
    }

    #[test]
    fn missing_cache_leaves_the_calendar_loading() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.restore_calendar(None);
        assert_eq!(state.calendar, CalendarContent::Loading);
        assert!(state.calendar_age().is_none());
    }

    #[test]
    fn show_closes_the_calendar() {
        let mut state = state_with_calendar_open();
        settle(&mut state);
        state.show();
        assert!(!state.calendar_open);
    }

    // --- Error surfacing (backlog a935) ---

    #[test]
    fn set_error_makes_hidden_window_visible() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.hide();
        state.set_error("boom".to_string());
        assert_eq!(state.visibility, Visibility::Visible);
        assert_eq!(state.error.as_deref(), Some("boom"));
    }

    #[test]
    fn failed_action_surfaces_as_error() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.hide();
        state.spawn_action(|| -> anyhow::Result<()> { anyhow::bail!("no command found") });
        while state.has_pending_actions() {
            state.poll_actions();
        }
        assert_eq!(state.error.as_deref(), Some("no command found"));
        assert_eq!(state.visibility, Visibility::Visible);
    }

    #[test]
    fn successful_action_leaves_window_hidden() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.hide();
        state.spawn_action(|| -> anyhow::Result<()> { Ok(()) });
        while state.has_pending_actions() {
            state.poll_actions();
        }
        assert!(state.error.is_none());
        assert_eq!(state.visibility, Visibility::Hidden);
    }

    #[test]
    fn typing_dismisses_the_error() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.show();
        state.set_error("boom".to_string());
        state.set_input("git".to_string());
        assert!(state.error.is_none());
    }

    #[test]
    fn show_clears_the_error() {
        let mut state = WindowState::new(sample_items(), vec![]);
        state.set_error("boom".to_string());
        state.show();
        assert!(state.error.is_none());
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
