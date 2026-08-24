use super::navigator::Navigator;
use super::path_service::{DirectoryItem, PathService, PathState, TypedPathState};
use crate::picker::search::{SearchResult, Snapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    FolderSearch,
    Path,
}

impl InputMode {
    pub fn resolve(input: &str) -> Self {
        let value = input.trim();
        let explicit = value.starts_with('/')
            || value == "~"
            || value.starts_with("~/")
            || value == "."
            || value.starts_with("./")
            || value == ".."
            || value.starts_with("../");
        if explicit {
            Self::Path
        } else {
            Self::FolderSearch
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadState {
    Loading { shows_message: bool },
    Loaded,
    Failed,
}

impl LoadState {
    pub fn is_loading(self) -> bool {
        matches!(self, Self::Loading { .. })
    }

    pub fn shows_message(self) -> bool {
        matches!(
            self,
            Self::Loading {
                shows_message: true
            }
        )
    }

    pub fn read_failed(self) -> bool {
        matches!(self, Self::Failed)
    }
}

pub struct Session {
    pub input: String,
    pub rows: Vec<DirectoryItem>,
    pub search_results: Vec<SearchResult>,
    pub folder_search_is_truncated: bool,
    pub folder_search_has_more_results: bool,
    pub highlighted_index: Option<usize>,
    pub load_state: LoadState,
    pub project_paths: Vec<String>,
    pub search_root_path: String,
    pub path_service: PathService,
}

impl Session {
    pub fn new(default_display_path: &str, project_paths: Vec<String>) -> Self {
        let path_service = PathService::default();
        let search_root_path =
            super::path_service::standardize(&path_service.expanded_path(default_display_path));
        Self {
            input: String::new(),
            rows: Vec::new(),
            search_results: Vec::new(),
            folder_search_is_truncated: false,
            folder_search_has_more_results: false,
            highlighted_index: None,
            load_state: LoadState::Loading {
                shows_message: false,
            },
            project_paths,
            search_root_path,
            path_service,
        }
    }

    pub fn input_mode(&self) -> InputMode {
        InputMode::resolve(&self.input)
    }

    pub fn search_query(&self) -> &str {
        self.input.trim()
    }

    pub fn path_state(&self) -> PathState {
        self.path_service.state(&self.input)
    }

    pub fn highlighted_item(&self) -> Option<&DirectoryItem> {
        if self.input_mode() != InputMode::Path {
            return None;
        }
        self.rows.get(self.highlighted_index?)
    }

    pub fn highlighted_search_result(&self) -> Option<&SearchResult> {
        if self.input_mode() != InputMode::FolderSearch {
            return None;
        }
        self.search_results.get(self.highlighted_index?)
    }

    pub fn confirmation_path(&self) -> Option<String> {
        match self.input_mode() {
            InputMode::FolderSearch => self
                .highlighted_search_result()
                .map(|result| result.path.clone()),
            InputMode::Path => Some(self.path_state().standardized_confirm_path),
        }
    }

    pub fn typed_path_state(&self) -> TypedPathState {
        self.path_service
            .typed_path_state(&self.path_state().standardized_confirm_path)
    }

    pub fn is_existing_project(&self) -> bool {
        let Some(path) = self.confirmation_path() else {
            return false;
        };
        self.project_paths
            .iter()
            .any(|project| super::path_service::standardize(project) == path)
    }

    pub fn action_title(&self) -> &'static str {
        if self.is_existing_project() {
            return "Open";
        }
        if self.input_mode() == InputMode::FolderSearch {
            return "Add";
        }
        if self.typed_path_state() == TypedPathState::Missing {
            "Create & Add"
        } else {
            "Add"
        }
    }

    pub fn top_right_action_title(&self) -> &'static str {
        if self.is_existing_project() {
            return "Open Project";
        }
        if self.input_mode() == InputMode::FolderSearch {
            return "Add Project";
        }
        if self.typed_path_state() == TypedPathState::Missing {
            "Create & Add Project"
        } else {
            "Add Project"
        }
    }

    pub fn ghost_text(&self) -> String {
        if self.input_mode() != InputMode::Path {
            return String::new();
        }
        let path_state = self.path_state();
        Navigator::new(&path_state).ghost_text(self.highlighted_item().map(DirectoryItem::name))
    }

    pub fn project_rows(&self) -> impl Iterator<Item = &DirectoryItem> {
        self.rows.iter().filter(|row| !row.is_parent())
    }

    pub fn has_parent_row(&self) -> bool {
        self.input_mode() == InputMode::Path && self.rows.iter().any(DirectoryItem::is_parent)
    }

    pub fn shows_unavailable_state(&self) -> bool {
        match self.input_mode() {
            InputMode::FolderSearch => self.search_results.is_empty(),
            InputMode::Path => {
                self.load_state.read_failed() || self.project_rows().next().is_none()
            }
        }
    }

    pub fn set_input(&mut self, input: impl Into<String>) {
        self.input = input.into();
        self.load_state = LoadState::Loading {
            shows_message: false,
        };
    }

    pub fn show_loading_message(&mut self) {
        if self.load_state.is_loading() {
            self.load_state = LoadState::Loading {
                shows_message: true,
            };
        }
    }

    pub fn select_row(&mut self, index: usize) {
        if index < self.active_row_count() {
            self.highlighted_index = Some(index);
        }
    }

    pub fn apply_search_snapshot(&mut self, snapshot: Snapshot) {
        self.load_state = if snapshot.read_failed {
            LoadState::Failed
        } else {
            LoadState::Loaded
        };
        self.search_results = snapshot.results;
        self.folder_search_is_truncated = snapshot.is_truncated;
        self.folder_search_has_more_results = snapshot.has_more_results;
        self.highlighted_index = (!self.search_results.is_empty()).then_some(0);
    }

    pub fn apply_directory_snapshot(&mut self, rows: Vec<DirectoryItem>, read_failed: bool) {
        self.load_state = if read_failed {
            LoadState::Failed
        } else {
            LoadState::Loaded
        };
        self.highlighted_index = initial_highlight(&rows);
        self.rows = rows;
    }

    pub fn move_highlight(&mut self, delta: i32) {
        let count = self.active_row_count();
        if count == 0 {
            return;
        }
        self.highlighted_index = Some(match self.highlighted_index {
            None => {
                if delta > 0 {
                    0
                } else {
                    count - 1
                }
            }
            Some(current) => (current as i32 + delta).clamp(0, count as i32 - 1) as usize,
        });
    }

    pub fn open_highlighted(&mut self) {
        if self.input_mode() != InputMode::Path {
            return;
        }
        let Some(item) = self.highlighted_item().cloned() else {
            return;
        };
        self.descend(&item);
    }

    pub fn go_back(&mut self) {
        if self.input_mode() != InputMode::Path {
            return;
        }
        self.go_up();
    }

    pub fn complete_highlighted(&mut self) {
        if let Some(result) = self.highlighted_search_result() {
            let path = self.path_service.abbreviated_display_path(&result.path);
            self.set_input(path);
            return;
        }
        let Some(row) = self.highlighted_item().map(|item| item.name().to_owned()) else {
            return;
        };
        let path_state = self.path_state();
        let completed = Navigator::new(&path_state).completed_path(&row);
        self.set_input(completed);
    }

    pub fn activate(&mut self, item: &DirectoryItem) {
        self.descend(item);
    }

    fn descend(&mut self, item: &DirectoryItem) {
        if item.is_parent() {
            self.go_up();
            return;
        }
        let path_state = self.path_state();
        let completed = Navigator::new(&path_state).completed_path(item.name());
        self.set_input(completed);
    }

    fn go_up(&mut self) {
        let path_state = self.path_state();
        let parent = path_state.parent_display_path.clone();
        if parent == self.input {
            return;
        }
        self.set_input(parent);
    }

    fn active_row_count(&self) -> usize {
        match self.input_mode() {
            InputMode::FolderSearch => self.search_results.len(),
            InputMode::Path => self.rows.len(),
        }
    }
}

fn initial_highlight(rows: &[DirectoryItem]) -> Option<usize> {
    if rows.is_empty() {
        return None;
    }
    if rows.first().map(DirectoryItem::is_parent) == Some(true) && rows.len() > 1 {
        Some(1)
    } else {
        Some(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(input: &str) -> Session {
        let mut session = Session::new("/Users/alice/Projects", vec!["/tmp/known".to_owned()]);
        session.path_service.home_directory = "/Users/alice".to_owned();
        session.set_input(input);
        session
    }

    #[test]
    fn resolves_input_mode_from_prefixes() {
        assert_eq!(InputMode::resolve("muxy"), InputMode::FolderSearch);
        assert_eq!(InputMode::resolve(""), InputMode::FolderSearch);
        assert_eq!(InputMode::resolve("/tmp"), InputMode::Path);
        assert_eq!(InputMode::resolve("~"), InputMode::Path);
        assert_eq!(InputMode::resolve("~/code"), InputMode::Path);
        assert_eq!(InputMode::resolve("./code"), InputMode::Path);
        assert_eq!(InputMode::resolve("../code"), InputMode::Path);
    }

    #[test]
    fn highlight_moves_within_active_rows() {
        let mut session = session("/tmp/");
        session.apply_directory_snapshot(
            vec![
                DirectoryItem::Parent,
                DirectoryItem::Directory("alpha".into()),
                DirectoryItem::Directory("beta".into()),
            ],
            false,
        );
        assert_eq!(session.highlighted_index, Some(1));

        session.move_highlight(1);
        assert_eq!(session.highlighted_index, Some(2));
        session.move_highlight(5);
        assert_eq!(session.highlighted_index, Some(2));
        session.move_highlight(-5);
        assert_eq!(session.highlighted_index, Some(0));
    }

    #[test]
    fn opening_and_going_back_rewrite_the_input() {
        let mut session = session("~/Projects/");
        session.apply_directory_snapshot(
            vec![
                DirectoryItem::Parent,
                DirectoryItem::Directory("muxy".into()),
            ],
            false,
        );

        session.open_highlighted();
        assert_eq!(session.input, "~/Projects/muxy/");

        session.go_back();
        assert_eq!(session.input, "~/Projects/");
    }

    #[test]
    fn tab_completion_uses_highlighted_row() {
        let mut session = session("~/Projects/mu");
        session.apply_directory_snapshot(
            vec![
                DirectoryItem::Parent,
                DirectoryItem::Directory("muxy".into()),
            ],
            false,
        );
        session.complete_highlighted();
        assert_eq!(session.input, "~/Projects/muxy/");
    }

    #[test]
    fn action_titles_follow_mode_and_existing_projects() {
        let mut session = session("/tmp/known");
        assert!(session.is_existing_project());
        assert_eq!(session.action_title(), "Open");
        assert_eq!(session.top_right_action_title(), "Open Project");

        session.set_input("/tmp/muxy-missing-directory-for-tests");
        assert_eq!(session.action_title(), "Create & Add");
        assert_eq!(session.top_right_action_title(), "Create & Add Project");

        session.set_input("muxy");
        assert_eq!(session.action_title(), "Add");
        assert_eq!(session.top_right_action_title(), "Add Project");
    }

    #[test]
    fn folder_search_confirms_the_highlighted_result() {
        let mut session = session("muxy");
        session.apply_search_snapshot(crate::picker::search::Snapshot {
            results: vec![crate::picker::search::SearchResult {
                name: "muxy".into(),
                path: "/Users/alice/Projects/muxy".into(),
                display_path: "~/Projects/muxy/".into(),
            }],
            ..Default::default()
        });
        assert_eq!(
            session.confirmation_path().as_deref(),
            Some("/Users/alice/Projects/muxy")
        );

        session.complete_highlighted();
        assert_eq!(session.input, "~/Projects/muxy/");
    }
}
