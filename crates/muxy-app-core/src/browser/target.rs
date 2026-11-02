use std::path::Path;

use crate::project_selector::{Mismatch, pick, same_name};
use crate::{AppState, Project, ProjectId};

/// The project a browser command acts in: the current one, or the one named
/// by `--project` and `--worktree`. A worktree is looked for in the named or
/// current project first, then in every project. Without `--project`, a
/// worktree name that only differs in case counts only when no worktree
/// anywhere has the exact name.
pub fn resolve_target(
    state: &AppState,
    project: Option<&str>,
    worktree: Option<&str>,
) -> Result<ProjectId, String> {
    let projects = state.projects();
    let root = match project {
        Some(name) => Some(id_or_message(
            pick(
                projects,
                |candidate| names(candidate, name),
                |candidate| same_name(&candidate.name, name),
            ),
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
    let in_family =
        |candidate: &Project| candidate.id == family || candidate.parent_id == Some(family);
    let is_worktree = |candidate: &Project| candidate.parent_id.is_some();
    let exact_elsewhere = root.is_none()
        && projects
            .iter()
            .any(|candidate| is_worktree(candidate) && names_worktree(candidate, name));
    let nearby = id_or_message(
        pick(
            projects,
            |candidate| in_family(candidate) && names_worktree(candidate, name),
            |candidate| {
                !exact_elsewhere && in_family(candidate) && loosely_names_worktree(candidate, name)
            },
        ),
        || format!("no worktree named '{name}'"),
        || format!("worktree '{name}' is ambiguous; pass its ID"),
    );
    if nearby.is_ok() || root.is_some() {
        return nearby;
    }
    id_or_message(
        pick(
            projects,
            |candidate| is_worktree(candidate) && names_worktree(candidate, name),
            |candidate| is_worktree(candidate) && loosely_names_worktree(candidate, name),
        ),
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

fn loosely_names_worktree(project: &Project, text: &str) -> bool {
    same_name(&project.name, text)
        || project
            .directory
            .file_name()
            .and_then(|folder| folder.to_str())
            .is_some_and(|folder| same_name(folder, text))
}

fn same_directory(directory: &Path, text: &Path) -> bool {
    text.is_absolute()
        && (directory == text
            || text
                .canonicalize()
                .is_ok_and(|canonical| canonical == directory))
}

fn id_or_message(
    found: Result<&Project, Mismatch>,
    missing: impl FnOnce() -> String,
    ambiguous: impl FnOnce() -> String,
) -> Result<ProjectId, String> {
    match found {
        Ok(project) => Ok(project.id),
        Err(Mismatch::NotFound) => Err(missing()),
        Err(Mismatch::Ambiguous) => Err(ambiguous()),
    }
}
