use std::path::Path;

use crate::{AppState, Project, ProjectId};

/// The project a browser command acts in: the current one, or the one named
/// by `--project` and `--worktree`. A worktree is looked for in the named or
/// current project first, then in every project.
pub fn resolve_target(
    state: &AppState,
    project: Option<&str>,
    worktree: Option<&str>,
) -> Result<ProjectId, String> {
    let projects = state.projects();
    let root = match project {
        Some(name) => Some(unique(
            projects.iter().filter(|candidate| names(candidate, name)),
            || format!("no project named '{name}'"),
            || format!("project '{name}' is ambiguous; pass its ID"),
        )?),
        None => None,
    };
    let Some(name) = worktree else {
        return Ok(root.unwrap_or_else(|| state.current_project().id));
    };
    let family = root.unwrap_or_else(|| {
        let current = state.current_project();
        current.parent_id.unwrap_or(current.id)
    });
    let nearby = unique(
        projects.iter().filter(|candidate| {
            (candidate.id == family || candidate.parent_id == Some(family))
                && names_worktree(candidate, name)
        }),
        || format!("no worktree named '{name}'"),
        || format!("worktree '{name}' is ambiguous; pass its ID"),
    );
    if nearby.is_ok() || root.is_some() {
        return nearby;
    }
    unique(
        projects
            .iter()
            .filter(|candidate| candidate.parent_id.is_some() && names_worktree(candidate, name)),
        || format!("no worktree named '{name}'"),
        || {
            format!(
                "worktree '{name}' is ambiguous across projects; pass --project to disambiguate"
            )
        },
    )
}

fn names(project: &Project, text: &str) -> bool {
    project.id.to_string() == text
        || project.name == text
        || same_directory(&project.directory, Path::new(text))
}

fn names_worktree(project: &Project, text: &str) -> bool {
    names(project, text)
        || project
            .directory
            .file_name()
            .is_some_and(|folder| folder == text)
}

fn same_directory(directory: &Path, text: &Path) -> bool {
    text.is_absolute()
        && (directory == text
            || text
                .canonicalize()
                .is_ok_and(|canonical| canonical == directory))
}

fn unique<'a>(
    mut candidates: impl Iterator<Item = &'a Project>,
    missing: impl FnOnce() -> String,
    ambiguous: impl FnOnce() -> String,
) -> Result<ProjectId, String> {
    let first = candidates.next().ok_or_else(missing)?;
    if candidates.next().is_some() {
        return Err(ambiguous());
    }
    Ok(first.id)
}
