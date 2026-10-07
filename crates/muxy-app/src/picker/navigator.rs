use super::path_service::{PARENT_ROW, PathState};

pub(crate) struct Navigator<'a> {
    pub(crate) path_state: &'a PathState,
}

impl<'a> Navigator<'a> {
    pub(crate) fn new(path_state: &'a PathState) -> Self {
        Self { path_state }
    }

    pub(crate) fn completed_path(&self, highlighted_row: &str) -> String {
        if highlighted_row == PARENT_ROW {
            return self.path_state.parent_display_path.clone();
        }
        format!(
            "{}{highlighted_row}/",
            self.path_state.completion_display_prefix
        )
    }

    pub(crate) fn ghost_text(&self, highlighted_row: Option<&str>) -> String {
        let Some(row) = highlighted_row.filter(|row| *row != PARENT_ROW) else {
            return String::new();
        };
        let input = &self.path_state.input;
        let completed = self.completed_path(row);
        if let Some(rest) = completed.strip_prefix(input.as_str()) {
            return rest.to_owned();
        }

        let trimmed = input.trim();
        if trimmed.contains('/') || trimmed.starts_with('~') {
            return String::new();
        }
        if row.eq_ignore_ascii_case(trimmed) {
            return "/".to_owned();
        }
        if !row.to_lowercase().starts_with(&trimmed.to_lowercase()) {
            return String::new();
        }
        format!("{}/", &row[trimmed.len()..])
    }
}
