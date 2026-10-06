use crate::tr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessibilityRole {
    Group,
    Status,
    Button,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessibilityNode {
    pub identifier: &'static str,
    pub role: AccessibilityRole,
    pub label: String,
    pub value: String,
    pub focus_order: usize,
    pub announces_changes: bool,
}

pub fn bridge_accessibility_model(status: &str, shortcut: &str) -> Vec<AccessibilityNode> {
    let definitions = [
        (
            "quick-terminal",
            AccessibilityRole::Group,
            tr!("Quick Terminal"),
            String::new(),
            false,
        ),
        (
            "quick-terminal-status",
            AccessibilityRole::Status,
            tr!("Quick Terminal status"),
            status.to_owned(),
            true,
        ),
        (
            "quick-terminal-shortcut",
            AccessibilityRole::Button,
            tr!("Quick Terminal shortcut"),
            shortcut.to_owned(),
            false,
        ),
        (
            "quick-terminal-settings",
            AccessibilityRole::Button,
            tr!("Open Quick Terminal settings"),
            tr!("Opens Settings").to_string(),
            false,
        ),
        (
            "quick-terminal-close",
            AccessibilityRole::Button,
            tr!("Close Quick Terminal"),
            tr!("Hides the panel").to_string(),
            false,
        ),
    ];
    definitions
        .into_iter()
        .enumerate()
        .map(
            |(focus_order, (identifier, role, label, value, announces_changes))| {
                AccessibilityNode {
                    identifier,
                    role,
                    label: label.to_string(),
                    value,
                    focus_order,
                    announces_changes,
                }
            },
        )
        .collect()
}
