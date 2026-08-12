//! Client/project selection model, shared by the terminal `switch` prompt and
//! the GUI launcher's in-window picker so both offer the same choices.

/// A client and the names of its nested projects.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerClient {
    pub name: String,
    pub projects: Vec<String>,
}

/// Which list the picker is currently showing.
#[derive(Debug, Clone, PartialEq)]
pub enum PickerStage {
    Clients,
    Projects { client: String },
}

/// A selectable row in the picker.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerEntry {
    pub label: String,
    pub client: String,
    pub project: Option<String>,
    /// Activating this row opens the client's project list instead of switching.
    pub descends: bool,
    pub is_current: bool,
}

/// The effect of activating an entry.
#[derive(Debug, Clone, PartialEq)]
pub enum PickerOutcome {
    /// Now showing the client's projects.
    Descended,
    /// The user picked a target to switch to.
    Chosen {
        client: String,
        project: Option<String>,
    },
}

/// Two-stage client/project picker: clients first, then — for clients with
/// nested projects — the client itself or one of its projects.
#[derive(Debug, Clone)]
pub struct PickerState {
    stage: PickerStage,
    clients: Vec<PickerClient>,
    current_client: Option<String>,
    current_project: Option<String>,
}

impl PickerState {
    pub fn new(
        clients: Vec<PickerClient>,
        current_client: Option<String>,
        current_project: Option<String>,
    ) -> Self {
        Self {
            stage: PickerStage::Clients,
            clients,
            current_client,
            current_project,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    /// Prompt text for the current stage.
    pub fn title(&self) -> String {
        match &self.stage {
            PickerStage::Clients => "Select a client".to_string(),
            PickerStage::Projects { client } => format!("Select '{}' or a project", client),
        }
    }

    /// Rows for the current stage, narrowed to those whose label contains `filter`.
    pub fn entries(&self, filter: &str) -> Vec<PickerEntry> {
        let all = match &self.stage {
            PickerStage::Clients => self
                .clients
                .iter()
                .map(|client| PickerEntry {
                    label: client.name.clone(),
                    client: client.name.clone(),
                    project: None,
                    descends: !client.projects.is_empty(),
                    is_current: self.current_client.as_deref() == Some(client.name.as_str()),
                })
                .collect(),
            PickerStage::Projects { client } => {
                let mut entries = vec![PickerEntry {
                    label: format!("{} (client)", client),
                    client: client.clone(),
                    project: None,
                    descends: false,
                    is_current: self.is_current(client, None),
                }];
                for project in self.projects_of(client) {
                    entries.push(PickerEntry {
                        label: project.clone(),
                        client: client.clone(),
                        project: Some(project.clone()),
                        descends: false,
                        is_current: self.is_current(client, Some(project)),
                    });
                }
                entries
            }
        };

        let needle = filter.trim().to_lowercase();
        if needle.is_empty() {
            return all;
        }
        all.into_iter()
            .filter(|entry| entry.label.to_lowercase().contains(&needle))
            .collect()
    }

    /// Row to start the cursor on: the active selection when it is in view.
    pub fn cursor(&self, filter: &str) -> usize {
        self.entries(filter)
            .iter()
            .position(|entry| entry.is_current)
            .unwrap_or(0)
    }

    pub fn activate(&mut self, entry: &PickerEntry) -> PickerOutcome {
        if entry.descends {
            self.stage = PickerStage::Projects {
                client: entry.client.clone(),
            };
            PickerOutcome::Descended
        } else {
            PickerOutcome::Chosen {
                client: entry.client.clone(),
                project: entry.project.clone(),
            }
        }
    }

    /// Step back one stage. Returns `false` when already at the client list.
    pub fn back(&mut self) -> bool {
        match self.stage {
            PickerStage::Clients => false,
            PickerStage::Projects { .. } => {
                self.stage = PickerStage::Clients;
                true
            }
        }
    }

    fn projects_of(&self, client: &str) -> &[String] {
        self.clients
            .iter()
            .find(|c| c.name == client)
            .map(|c| c.projects.as_slice())
            .unwrap_or_default()
    }

    fn is_current(&self, client: &str, project: Option<&str>) -> bool {
        self.current_client.as_deref() == Some(client) && self.current_project.as_deref() == project
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(name: &str, projects: &[&str]) -> PickerClient {
        PickerClient {
            name: name.to_string(),
            projects: projects.iter().map(|p| p.to_string()).collect(),
        }
    }

    fn sample() -> Vec<PickerClient> {
        vec![
            client("nero", &[]),
            client("EventsAir", &["Build & Deploy", "Other"]),
        ]
    }

    fn picker(current_client: Option<&str>, current_project: Option<&str>) -> PickerState {
        PickerState::new(
            sample(),
            current_client.map(|s| s.to_string()),
            current_project.map(|s| s.to_string()),
        )
    }

    fn labels(entries: &[PickerEntry]) -> Vec<String> {
        entries.iter().map(|e| e.label.clone()).collect()
    }

    /// A picker that has descended into EventsAir's project list.
    fn descended(current_client: Option<&str>, current_project: Option<&str>) -> PickerState {
        let mut state = picker(current_client, current_project);
        let clients = state.entries("");
        state.activate(&clients[1]);
        state
    }

    /// The outcome of activating the project-stage row at `index`.
    fn choose_project_row(index: usize) -> PickerOutcome {
        let mut state = descended(None, None);
        let projects = state.entries("");
        state.activate(&projects[index])
    }

    #[test]
    fn starts_on_client_stage() {
        let state = picker(None, None);
        assert_eq!(labels(&state.entries("")), vec!["nero", "EventsAir"]);
    }

    #[test]
    fn client_without_projects_is_chosen_directly() {
        let mut state = picker(None, None);
        let entries = state.entries("");
        assert!(!entries[0].descends);
        assert_eq!(
            state.activate(&entries[0]),
            PickerOutcome::Chosen {
                client: "nero".to_string(),
                project: None,
            }
        );
    }

    #[test]
    fn client_with_projects_descends() {
        let mut state = picker(None, None);
        let entries = state.entries("");
        assert!(entries[1].descends);
        assert_eq!(state.activate(&entries[1]), PickerOutcome::Descended);
    }

    #[test]
    fn project_stage_offers_client_itself_first() {
        let state = descended(None, None);
        assert_eq!(
            labels(&state.entries("")),
            vec!["EventsAir (client)", "Build & Deploy", "Other"]
        );
    }

    #[test]
    fn choosing_client_entry_clears_project() {
        assert_eq!(
            choose_project_row(0),
            PickerOutcome::Chosen {
                client: "EventsAir".to_string(),
                project: None,
            }
        );
    }

    #[test]
    fn choosing_nested_project() {
        assert_eq!(
            choose_project_row(1),
            PickerOutcome::Chosen {
                client: "EventsAir".to_string(),
                project: Some("Build & Deploy".to_string()),
            }
        );
    }

    #[test]
    fn filter_narrows_by_label() {
        let state = picker(None, None);
        assert_eq!(labels(&state.entries("event")), vec!["EventsAir"]);
        assert!(state.entries("nonexistent").is_empty());
    }

    #[test]
    fn cursor_starts_on_current_client() {
        let state = picker(Some("EventsAir"), Some("Other"));
        assert_eq!(state.cursor(""), 1);
    }

    #[test]
    fn cursor_starts_on_current_project() {
        assert_eq!(descended(Some("EventsAir"), Some("Other")).cursor(""), 2);
    }

    #[test]
    fn cursor_starts_on_client_entry_when_no_project_selected() {
        assert_eq!(descended(Some("EventsAir"), None).cursor(""), 0);
    }

    #[test]
    fn cursor_falls_back_to_first_row() {
        let state = picker(Some("deleted"), None);
        assert_eq!(state.cursor(""), 0);
    }

    #[test]
    fn current_project_marks_only_that_project() {
        let state = descended(Some("EventsAir"), Some("Other"));
        let projects = state.entries("");
        assert!(!projects[0].is_current);
        assert!(!projects[1].is_current);
        assert!(projects[2].is_current);
    }

    #[test]
    fn back_returns_to_client_stage() {
        let mut state = descended(None, None);
        assert!(state.back());
        assert_eq!(labels(&state.entries("")), vec!["nero", "EventsAir"]);
    }

    #[test]
    fn back_at_client_stage_reports_top() {
        let mut state = picker(None, None);
        assert!(!state.back());
    }

    #[test]
    fn empty_config_has_no_entries() {
        let state = PickerState::new(vec![], None, None);
        assert!(state.is_empty());
        assert!(state.entries("").is_empty());
    }
}
