use crate::paprika::GroceryCommand;

#[derive(Debug, Clone, PartialEq)]
pub enum GroceryRow {
    Item(String),
    Add(String),
}

#[derive(Debug, Clone, Default)]
pub struct GroceryPicker {
    items: Vec<String>,
    pub loading: bool,
}

impl GroceryPicker {
    pub fn loading() -> Self {
        Self {
            items: Vec::new(),
            loading: true,
        }
    }

    pub fn title(&self) -> String {
        "Grocery list: Enter removes, type a new item to add it".to_string()
    }

    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        self.loading = false;
    }

    pub fn rows(&self, filter: &str) -> Vec<GroceryRow> {
        let filter = filter.trim();
        let needle = filter.to_lowercase();
        let rows: Vec<GroceryRow> = self
            .items
            .iter()
            .filter(|item| item.to_lowercase().contains(&needle))
            .cloned()
            .map(GroceryRow::Item)
            .collect();
        if rows.is_empty() && !filter.is_empty() {
            return vec![GroceryRow::Add(filter.to_string())];
        }
        rows
    }

    pub fn command(&self, filter: &str, selected: usize) -> Option<GroceryCommand> {
        if self.loading {
            return None;
        }
        Some(match self.rows(filter).into_iter().nth(selected)? {
            GroceryRow::Item(item) => GroceryCommand::Remove { item, list: None },
            GroceryRow::Add(item) => GroceryCommand::Add { item, list: None },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picker() -> GroceryPicker {
        let mut picker = GroceryPicker::loading();
        picker.set_items(vec![
            "Milk".to_string(),
            "Oat milk".to_string(),
            "Eggs".to_string(),
        ]);
        picker
    }

    #[test]
    fn filter_narrows_items_ignoring_case() {
        assert_eq!(
            picker().rows("MILK"),
            [
                GroceryRow::Item("Milk".to_string()),
                GroceryRow::Item("Oat milk".to_string())
            ]
        );
    }

    #[test]
    fn unmatched_filter_offers_to_add() {
        assert_eq!(
            picker().rows(" bread "),
            [GroceryRow::Add("bread".to_string())]
        );
        assert!(GroceryPicker::default().rows("").is_empty());
    }

    #[test]
    fn selected_item_is_removed() {
        assert_eq!(
            picker().command("milk", 1),
            Some(GroceryCommand::Remove {
                item: "Oat milk".to_string(),
                list: None
            })
        );
    }

    #[test]
    fn unmatched_filter_is_added() {
        assert_eq!(
            picker().command("bread", 0),
            Some(GroceryCommand::Add {
                item: "bread".to_string(),
                list: None
            })
        );
    }

    #[test]
    fn nothing_to_do_while_loading_or_empty() {
        assert_eq!(GroceryPicker::loading().command("bread", 0), None);
        assert_eq!(GroceryPicker::default().command("", 0), None);
    }
}
