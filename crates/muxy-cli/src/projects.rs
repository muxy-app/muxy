//! The order projects are listed in: Home first, then each project followed
//! by its worktrees.

use std::collections::{BTreeMap, BTreeSet};

use muxy_protocol::{CatalogPage, ProjectDescriptor, ProjectId, ServerPath};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry<'a> {
    pub project: &'a ProjectDescriptor,
    pub worktree: bool,
    /// The last worktree under its parent.
    pub last: bool,
}

pub(crate) fn tree(catalog: &CatalogPage) -> Vec<Entry<'_>> {
    let tops: BTreeSet<ProjectId> = catalog
        .projects
        .iter()
        .filter(|project| project.parent_id.is_none())
        .map(|project| project.id)
        .collect();
    // A worktree whose parent is gone is listed on its own.
    let mut roots = Vec::new();
    let mut children: BTreeMap<ProjectId, Vec<&ProjectDescriptor>> = BTreeMap::new();
    for project in &catalog.projects {
        match project.parent_id.filter(|parent| tops.contains(parent)) {
            Some(parent) => children.entry(parent).or_default().push(project),
            None => roots.push(project),
        }
    }
    roots.sort_by_key(|project| !project.home);
    let mut entries = Vec::with_capacity(catalog.projects.len());
    for root in roots {
        entries.push(Entry {
            project: root,
            worktree: false,
            last: false,
        });
        let children = children.remove(&root.id).unwrap_or_default();
        let count = children.len();
        entries.extend(
            children
                .into_iter()
                .enumerate()
                .map(|(index, project)| Entry {
                    project,
                    worktree: true,
                    last: index + 1 == count,
                }),
        );
    }
    entries
}

/// The project before or after `current` in list order, wrapping around.
pub(crate) fn cycle(catalog: &CatalogPage, current: ProjectId, forward: bool) -> Option<ProjectId> {
    let entries = tree(catalog);
    let count = entries.len();
    let index = entries
        .iter()
        .position(|entry| entry.project.id == current)?;
    let next = if forward {
        (index + 1) % count
    } else {
        (index + count - 1) % count
    };
    Some(entries[next].project.id)
}

/// `directory` for display, with the home folder shortened to `~`.
pub(crate) fn display_path(catalog: &CatalogPage, directory: &ServerPath) -> String {
    let home = catalog
        .projects
        .iter()
        .find(|project| project.home)
        .map(|project| project.directory.0.as_slice())
        .filter(|home| home.len() > 1);
    let path = directory.0.as_slice();
    let shortened = match home.and_then(|home| path.strip_prefix(home)) {
        Some([]) => "~".to_owned(),
        Some(rest) if rest.starts_with(b"/") => format!("~{}", String::from_utf8_lossy(rest)),
        _ => String::from_utf8_lossy(path).into_owned(),
    };
    crate::ui::clean(&shortened)
}

#[cfg(test)]
mod tests {
    use muxy_protocol::{ProjectKind, ServerIdentity};

    use super::*;

    fn project(
        name: &str,
        home: bool,
        parent: Option<ProjectId>,
        directory: &str,
    ) -> ProjectDescriptor {
        ProjectDescriptor {
            id: ProjectId::new(),
            home,
            directory: ServerPath(directory.as_bytes().to_vec()),
            name: name.into(),
            icon: None,
            logo: None,
            color: "#808080".into(),
            kind: parent.map(|_| ProjectKind::Worktree),
            parent_id: parent,
        }
    }

    #[test]
    fn home_comes_first_and_worktrees_follow_their_parent() {
        let app = project("app", false, None, "/home/me/app");
        let home = project("Home", true, None, "/home/me");
        let feature = project("feature", false, Some(app.id), "/home/me/app-feature");
        let lost = project("lost", false, Some(ProjectId::new()), "/srv/lost");
        let fix = project("fix", false, Some(app.id), "/home/me/app-fix");
        let catalog = CatalogPage {
            server: ServerIdentity::new(),
            home: home.id,
            revision: 0,
            next: None,
            legacy_home: None,
            projects: vec![app.clone(), feature, home.clone(), lost, fix],
        };
        let entries: Vec<_> = tree(&catalog)
            .iter()
            .map(|entry| (entry.project.name.as_str(), entry.worktree, entry.last))
            .collect();
        assert_eq!(
            entries,
            [
                ("Home", false, false),
                ("app", false, false),
                ("feature", true, false),
                ("fix", true, true),
                ("lost", false, false),
            ]
        );
        assert_eq!(
            cycle(&catalog, home.id, false).map(|id| id == catalog.projects[3].id),
            Some(true)
        );
        assert_eq!(cycle(&catalog, home.id, true), Some(app.id));
        assert_eq!(display_path(&catalog, &app.directory), "~/app");
        assert_eq!(display_path(&catalog, &home.directory), "~");
        assert_eq!(
            display_path(&catalog, &ServerPath(b"/home/meadow".to_vec())),
            "/home/meadow"
        );
    }
}
