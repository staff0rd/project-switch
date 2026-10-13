use crate::config::BrowserOption;

#[derive(Debug, Clone)]
pub struct BrowserPicker {
    pub command_key: String,
    pub action_input: String,
    options: Vec<BrowserOption>,
}

impl BrowserPicker {
    pub fn new(command_key: String, action_input: String, options: Vec<BrowserOption>) -> Self {
        Self {
            command_key,
            action_input,
            options,
        }
    }

    pub fn title(&self) -> String {
        format!("Open '{}' with", self.command_key)
    }

    pub fn entries(&self, filter: &str) -> Vec<BrowserOption> {
        let needle = filter.trim().to_lowercase();
        self.options
            .iter()
            .filter(|option| option.label.to_lowercase().contains(&needle))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picker() -> BrowserPicker {
        BrowserPicker::new(
            "member-dev".to_string(),
            "member-dev".to_string(),
            vec![
                BrowserOption {
                    label: "integrator".to_string(),
                    value: "firefox -P integrator".to_string(),
                },
                BrowserOption {
                    label: "integrator+coach".to_string(),
                    value: "firefox -P coach".to_string(),
                },
            ],
        )
    }

    fn labels(picker: &BrowserPicker, filter: &str) -> Vec<String> {
        picker
            .entries(filter)
            .into_iter()
            .map(|option| option.label)
            .collect()
    }

    #[test]
    fn empty_filter_lists_all_in_config_order() {
        assert_eq!(
            labels(&picker(), ""),
            vec!["integrator", "integrator+coach"]
        );
    }

    #[test]
    fn filter_narrows_by_label_ignoring_case() {
        assert_eq!(labels(&picker(), "COACH"), vec!["integrator+coach"]);
        assert!(labels(&picker(), "nope").is_empty());
    }

    #[test]
    fn title_names_the_command() {
        assert_eq!(picker().title(), "Open 'member-dev' with");
    }
}
