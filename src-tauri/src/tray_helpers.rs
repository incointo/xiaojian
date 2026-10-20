pub fn menu_item_id(task_id: i64) -> String {
    format!("complete-{task_id}")
}

pub fn menu_item_title(title: &str) -> String {
    let mut characters = title.chars();
    let preview: String = characters.by_ref().take(48).collect();
    if characters.next().is_some() {
        format!("{}…", preview.chars().take(47).collect::<String>())
    } else {
        preview
    }
}

#[cfg(test)]
mod tests {
    use super::{menu_item_id, menu_item_title};

    #[test]
    fn menu_ids_are_stable_and_labels_are_safely_truncated() {
        assert_eq!(menu_item_id(42), "complete-42");
        assert_eq!(menu_item_id(42), "complete-42");

        let title_48 = "中".repeat(48);
        let title_49 = "中".repeat(49);
        assert_eq!(menu_item_title(&title_48), title_48);

        let label = menu_item_title(&title_49);
        assert_eq!(label.chars().count(), 48);
        assert!(label.ends_with('…'));
        assert_eq!(label.chars().take(47).collect::<String>(), "中".repeat(47));
        assert_eq!(title_49.chars().count(), 49);
    }
}
