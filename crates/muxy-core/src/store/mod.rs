mod command_shortcuts;
pub mod ghostty_conf;
mod groups;
pub mod logo;
mod persistence;
mod projects;
mod recently_removed;
pub mod worktrees;

pub use command_shortcuts::CommandShortcuts;
pub use groups::{Group, Groups};
pub use persistence::{write_atomic, write_private};
pub use projects::{HOME_PROJECT_ID, Project, home_project, load_projects};
pub use recently_removed::{load as load_recently_removed, take as take_recently_removed};
pub use worktrees::{Source as WorktreeSource, Worktree};

use crate::prefs::{Prefs, SortMode};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

const REFERENCE_EPOCH_OFFSET: f64 = 978_307_200.0;

fn named_entries(file: &str, name_keys: &[&str]) -> Vec<(String, String)> {
    let path = crate::prefs::app_support_dir().join(file);
    let Some(serde_json::Value::Array(items)) = crate::prefs::read_json(&path) else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| {
            let id = item
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let name = name_keys
                .iter()
                .find_map(|key| item.get(*key).and_then(serde_json::Value::as_str))
                .map(str::to_owned)
                .unwrap_or_else(|| id.clone())
                .to_owned();
            (!name.is_empty()).then_some((id, name))
        })
        .collect()
}

pub fn browser_profiles() -> Vec<(String, String)> {
    named_entries("browser-profiles.json", &["name"])
}

pub fn approved_devices() -> Vec<String> {
    named_entries("approved-devices.json", &["name", "deviceName"])
        .into_iter()
        .map(|(_, name)| name)
        .collect()
}

pub fn remote_devices() -> Vec<String> {
    named_entries("remote-devices.json", &["name", "host", "destination"])
        .into_iter()
        .map(|(_, name)| name)
        .collect()
}

pub fn reference_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs_f64() - REFERENCE_EPOCH_OFFSET)
        .unwrap_or_default()
}

pub fn new_uuid() -> String {
    let mut bytes = [0u8; 16];
    getrandom(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02X}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn getrandom(buffer: &mut [u8]) {
    use std::io::Read;
    if let Ok(mut file) = std::fs::File::open("/dev/urandom")
        && file.read_exact(buffer).is_ok()
    {
        return;
    }
    let seed = reference_now().to_bits();
    for (index, byte) in buffer.iter_mut().enumerate() {
        *byte = ((seed >> ((index % 8) * 8)) & 0xff) as u8;
    }
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub projects: Vec<Project>,
    pub groups: Groups,
    pub active_group_id: Option<String>,
    sort_mode: SortMode,
    show_home: bool,
}

impl Workspace {
    pub fn load(prefs: &Prefs) -> Self {
        let projects = load_projects();
        let groups = Groups::load();
        let active_group_id = prefs
            .active_group_id
            .as_ref()
            .filter(|id| {
                groups
                    .all()
                    .iter()
                    .any(|group| group.id.eq_ignore_ascii_case(id))
            })
            .cloned();
        if prefs.active_group_id.is_some() && active_group_id.is_none() {
            Prefs::store_default("muxy.activeProjectGroupID", None);
        }
        let mut workspace = Self {
            projects,
            groups,
            active_group_id,
            sort_mode: prefs.sort_mode,
            show_home: prefs.show_home_project,
        };
        workspace.sort();
        workspace
    }

    pub fn active_group_name(&self) -> String {
        self.groups.name_for(self.active_group_id.as_deref())
    }

    pub fn visible_projects(&self) -> Vec<&Project> {
        let home = self.projects.iter().filter(|project| project.is_home());
        let Some(active) = self.active_group_id.as_deref() else {
            return home
                .chain(self.projects.iter().filter(|project| !project.is_home()))
                .collect();
        };
        if !self.groups.is_local(active) {
            return home.collect();
        }
        let members: Vec<&str> = self
            .groups
            .all()
            .iter()
            .find(|group| group.id.eq_ignore_ascii_case(active))
            .map(|group| group.project_ids.iter().map(String::as_str).collect())
            .unwrap_or_default();
        home.chain(self.projects.iter().filter(|project| {
            !project.is_home()
                && members
                    .iter()
                    .any(|member| member.eq_ignore_ascii_case(&project.id))
        }))
        .collect()
    }

    pub fn select_group(&mut self, id: Option<String>) {
        self.active_group_id = id;
        Prefs::store_default("muxy.activeProjectGroupID", self.active_group_id.as_deref());
    }

    pub fn activate_group_for_project(&mut self, project_id: &str) {
        if project_id.eq_ignore_ascii_case(HOME_PROJECT_ID) {
            return;
        }
        if self.active_group_id.is_none() {
            return;
        }
        let containing = self
            .groups
            .group_id_containing(project_id)
            .map(str::to_owned);
        if containing.as_deref() == self.active_group_id.as_deref() {
            return;
        }
        self.select_group(containing);
    }

    pub fn sort(&mut self) {
        self.projects.retain(|project| !project.is_home());
        match self.sort_mode {
            SortMode::Manual => self.projects.sort_by_key(|project| project.sort_order),
            SortMode::NameAscending => self
                .projects
                .sort_by_key(|project| project.name.to_lowercase()),
            SortMode::NameDescending => {
                self.projects
                    .sort_by_key(|project| project.name.to_lowercase());
                self.projects.reverse();
            }
            SortMode::RecentlyActive => self.projects.sort_by(|left, right| {
                right
                    .last_active()
                    .total_cmp(&left.last_active())
                    .then_with(|| left.sort_order.cmp(&right.sort_order))
            }),
            SortMode::DateCreated => self
                .projects
                .sort_by(|left, right| left.created_at.total_cmp(&right.created_at)),
        }
        self.projects.sort_by_key(|project| !project.is_pinned);
        if self.show_home {
            self.projects.insert(0, home_project());
        }
    }

    pub fn hide_home(&mut self) {
        self.show_home = false;
        self.projects.retain(|project| !project.is_home());
    }

    pub fn set_show_home(&mut self, show_home: bool) {
        self.show_home = show_home;
        self.sort();
    }

    pub fn set_sort_mode(&mut self, sort_mode: SortMode) {
        self.sort_mode = sort_mode;
        self.sort();
    }

    pub fn sort_mode(&self) -> SortMode {
        self.sort_mode
    }

    pub fn ensure_home(&mut self) {
        if !self.projects.iter().any(Project::is_home) {
            self.projects.insert(0, home_project());
        }
    }

    pub fn resolve_active(&self, prefs: &Prefs) -> Option<String> {
        let stored = prefs
            .active_project_id
            .as_ref()
            .filter(|id| self.projects.iter().any(|project| &&project.id == id));
        if let Some(stored) = stored {
            return Some(stored.clone());
        }
        self.projects
            .iter()
            .max_by(|a, b| a.last_active().total_cmp(&b.last_active()))
            .map(|project| project.id.clone())
    }

    pub fn project(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|project| project.id == id)
    }

    pub fn contains_path(&self, path: &str) -> Option<&Project> {
        self.projects
            .iter()
            .find(|project| !project.is_home() && project.path == path)
    }

    pub fn project_paths(&self) -> Vec<String> {
        self.projects
            .iter()
            .filter(|project| !project.is_home())
            .map(|project| project.path.clone())
            .collect()
    }

    pub fn update<F>(&mut self, id: &str, edit: F) -> bool
    where
        F: FnOnce(&mut Project),
    {
        let Some(project) = self.projects.iter_mut().find(|project| project.id == id) else {
            return false;
        };
        if project.is_home() {
            return false;
        }
        let snapshot = project.clone();
        edit(project);
        if self.persist() {
            return true;
        }
        if let Some(project) = self.projects.iter_mut().find(|project| project.id == id) {
            *project = snapshot;
        }
        false
    }

    fn next_sort_order(&self) -> i64 {
        self.projects
            .iter()
            .filter(|project| !project.is_home())
            .map(|project| project.sort_order)
            .max()
            .unwrap_or(-1)
            + 1
    }

    pub fn add(&mut self, name: String, path: String) -> Option<String> {
        let sort_order = self.next_sort_order();
        let mut project = Project::new(name, path, sort_order);
        project.icon_color = random_swatch(&self.used_icon_colors());
        let id = project.id.clone();

        self.projects.push(project);
        if self.persist() {
            if let Some(project) = self.projects.iter().find(|project| project.id == id) {
                let _ = recently_removed::forget(project);
            }
            return Some(id);
        }
        self.projects.retain(|project| project.id != id);
        None
    }

    fn can_restore(&self, project: &Project) -> bool {
        !self.projects.iter().any(|existing| {
            existing.id.eq_ignore_ascii_case(&project.id) || existing.path == project.path
        })
    }

    pub fn restore(&mut self, project: Project) -> bool {
        if !self.can_restore(&project) {
            return false;
        }
        let sort_order = self.next_sort_order();
        let id = project.id.clone();
        let mut project = project;
        project.sort_order = sort_order;
        self.projects.push(project);
        if !self.persist() {
            self.projects.retain(|project| project.id != id);
            return false;
        }
        self.sort();
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let Some(index) = self
            .projects
            .iter()
            .position(|project| project.id == id && !project.is_home())
        else {
            return false;
        };
        let project = self.projects.remove(index);
        if !self.persist() {
            self.projects.insert(index, project);
            return false;
        }
        let _ = recently_removed::record(&project);
        logo::remove(&project.id);
        self.groups.remove_project_everywhere(&project.id);
        worktrees::remove(&project.id);
        true
    }

    fn used_icon_colors(&self) -> HashSet<String> {
        self.projects
            .iter()
            .filter_map(|project| project.icon_color.clone())
            .collect()
    }

    pub fn for_tests(projects: Vec<Project>) -> Self {
        Self {
            projects,
            groups: Groups::load_from("/nonexistent/project-groups.json"),
            active_group_id: None,
            sort_mode: SortMode::Manual,
            show_home: false,
        }
    }

    fn persist(&self) -> bool {
        let stored: Vec<&Project> = self
            .projects
            .iter()
            .filter(|project| !project.is_home())
            .collect();
        persistence::write_json(&projects::projects_path(), &stored).is_ok()
    }
}

fn random_swatch(used: &HashSet<String>) -> Option<String> {
    let available: Vec<&str> = ICON_PALETTE
        .iter()
        .map(|swatch| swatch.id)
        .filter(|id| !used.contains(*id))
        .collect();
    let pool: Vec<&str> = if available.is_empty() {
        ICON_PALETTE.iter().map(|swatch| swatch.id).collect()
    } else {
        available
    };
    let mut index = [0u8; 1];
    getrandom(&mut index);
    pool.get(index[0] as usize % pool.len())
        .map(|id| (*id).to_owned())
}

pub struct IconSwatch {
    pub id: &'static str,
    pub hex: &'static str,
}

pub const ICON_PALETTE: [IconSwatch; 12] = [
    IconSwatch {
        id: "red",
        hex: "#E5484D",
    },
    IconSwatch {
        id: "orange",
        hex: "#F76B15",
    },
    IconSwatch {
        id: "amber",
        hex: "#F5A623",
    },
    IconSwatch {
        id: "yellow",
        hex: "#EBCB00",
    },
    IconSwatch {
        id: "lime",
        hex: "#9BCD1E",
    },
    IconSwatch {
        id: "green",
        hex: "#30A46C",
    },
    IconSwatch {
        id: "teal",
        hex: "#12A594",
    },
    IconSwatch {
        id: "cyan",
        hex: "#05A2C2",
    },
    IconSwatch {
        id: "blue",
        hex: "#3E63DD",
    },
    IconSwatch {
        id: "indigo",
        hex: "#5B5BD6",
    },
    IconSwatch {
        id: "violet",
        hex: "#8E4EC6",
    },
    IconSwatch {
        id: "pink",
        hex: "#D6409F",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(orders: &[i64]) -> Workspace {
        Workspace::for_tests(
            orders
                .iter()
                .enumerate()
                .map(|(index, order)| {
                    Project::new(format!("p{index}"), format!("/tmp/p{index}"), *order)
                })
                .collect(),
        )
    }

    #[test]
    fn a_restored_project_lands_after_the_highest_sort_order() {
        assert_eq!(workspace(&[]).next_sort_order(), 0);
        assert_eq!(workspace(&[0, 1, 2]).next_sort_order(), 3);
        assert_eq!(workspace(&[7, 2, 5]).next_sort_order(), 8);
    }

    #[test]
    fn restore_rejects_an_existing_id_or_path() {
        let existing = Project::new("Existing".to_owned(), "/tmp/existing".to_owned(), 0);
        let workspace = Workspace::for_tests(vec![existing.clone()]);

        assert!(!workspace.can_restore(&existing));
        let same_path = Project::new("Replacement".to_owned(), existing.path.clone(), 1);
        assert!(!workspace.can_restore(&same_path));
        let distinct = Project::new("Distinct".to_owned(), "/tmp/distinct".to_owned(), 2);
        assert!(workspace.can_restore(&distinct));
    }

    #[test]
    fn a_restored_project_keeps_its_identity_through_projects_json() {
        let mut removed = Project::new("Restored".to_owned(), "/tmp/restored".to_owned(), 3);
        removed.icon = Some("hammer".to_owned());
        removed.icon_color = Some("blue".to_owned());
        let id = removed.id.clone();

        let mut restored = removed.clone();
        restored.sort_order = workspace(&[0, 1, 2]).next_sort_order();

        let encoded = serde_json::to_string(&vec![restored]).expect("encodes");
        let decoded: Vec<Project> = serde_json::from_str(&encoded).expect("decodes");
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].id, id);
        assert_eq!(decoded[0].sort_order, 3);
        assert_eq!(decoded[0].icon.as_deref(), Some("hammer"));
        assert_eq!(decoded[0].icon_color.as_deref(), Some("blue"));
    }

    #[test]
    fn sort_mode_changes_apply_immediately_and_keep_pinned_projects_first() {
        let mut alpha = Project::new("Alpha".to_owned(), "/tmp/alpha".to_owned(), 2);
        alpha.created_at = 3.0;
        alpha.last_active_at = Some(1.0);
        let mut beta = Project::new("Beta".to_owned(), "/tmp/beta".to_owned(), 5);
        beta.created_at = 2.0;
        beta.last_active_at = Some(2.0);
        beta.is_pinned = true;
        let mut gamma = Project::new("Gamma".to_owned(), "/tmp/gamma".to_owned(), 0);
        gamma.created_at = 1.0;
        gamma.last_active_at = Some(3.0);
        let mut workspace = Workspace::for_tests(vec![alpha, beta, gamma]);

        let cases = [
            (SortMode::Manual, vec!["Beta", "Gamma", "Alpha"]),
            (SortMode::NameAscending, vec!["Beta", "Alpha", "Gamma"]),
            (SortMode::NameDescending, vec!["Beta", "Gamma", "Alpha"]),
            (SortMode::RecentlyActive, vec!["Beta", "Gamma", "Alpha"]),
            (SortMode::DateCreated, vec!["Beta", "Gamma", "Alpha"]),
        ];

        for (mode, expected) in cases {
            workspace.set_sort_mode(mode);
            assert_eq!(workspace.sort_mode(), mode);
            assert_eq!(
                workspace
                    .projects
                    .iter()
                    .map(|project| project.name.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(workspace.projects[0].is_pinned);
            assert!(
                workspace.projects[1..]
                    .iter()
                    .all(|project| !project.is_pinned)
            );
        }
    }
}
