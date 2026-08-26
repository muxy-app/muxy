use crate::state::AppState;
use muxy_core::store::{Project, Worktree};
use std::path::{Component, Path, PathBuf};

pub struct ParsedTarget {
    pub project: Option<String>,
    pub worktree: Option<String>,
    pub remaining: Vec<String>,
}

pub struct ResolvedTarget {
    pub project_id: String,
    pub worktree_id: String,
}

pub fn parse_flags(parts: &[&str]) -> ParsedTarget {
    let mut project = None;
    let mut worktree = None;
    let mut end = parts.len();
    while end >= 2 {
        match parts[end - 2] {
            "--project" => project = Some(parts[end - 1].to_owned()),
            "--worktree" => worktree = Some(parts[end - 1].to_owned()),
            _ => break,
        }
        end -= 2;
    }
    ParsedTarget {
        project,
        worktree,
        remaining: parts[..end].iter().map(|part| (*part).to_owned()).collect(),
    }
}

pub fn display_id(id: &str) -> String {
    id.to_ascii_uppercase()
}

pub fn standardize_path(raw: &str) -> Option<String> {
    if raw.trim().is_empty() {
        return None;
    }
    let raw = if raw == "~" {
        muxy_core::prefs::home_dir()
    } else if let Some(suffix) = raw.strip_prefix("~/") {
        muxy_core::prefs::home_dir().join(suffix)
    } else {
        PathBuf::from(raw)
    };
    let absolute = if raw.is_absolute() {
        raw
    } else {
        std::env::current_dir().ok()?.join(raw)
    };
    let mut standardized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                standardized.pop();
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                standardized.push(component.as_os_str());
            }
        }
    }
    Some(standardized.to_string_lossy().into_owned())
}

pub fn find_project<'a>(state: &'a AppState, identifier: &str) -> Option<&'a Project> {
    let path = standardize_path(identifier);
    state.workspace.projects.iter().find(|project| {
        project.id.eq_ignore_ascii_case(identifier)
            || project.name.eq_ignore_ascii_case(identifier)
            || path.as_deref() == standardize_path(&project.path).as_deref()
    })
}

pub fn find_worktree<'a>(worktrees: &'a [Worktree], identifier: &str) -> Option<&'a Worktree> {
    let path = standardize_path(identifier);
    worktrees.iter().find(|worktree| {
        worktree.id.eq_ignore_ascii_case(identifier)
            || worktree.name.eq_ignore_ascii_case(identifier)
            || worktree
                .branch
                .as_deref()
                .is_some_and(|branch| branch.eq_ignore_ascii_case(identifier))
            || path.as_deref() == standardize_path(&worktree.path).as_deref()
    })
}

fn project_for<'a>(state: &'a AppState, identifier: Option<&str>) -> Option<&'a Project> {
    match identifier.filter(|identifier| !identifier.is_empty()) {
        Some(identifier) => find_project(state, identifier),
        None => state.active_project(),
    }
}

pub fn resolve(state: &AppState, parsed: &ParsedTarget) -> Result<Option<ResolvedTarget>, String> {
    let has_project = parsed
        .project
        .as_deref()
        .is_some_and(|value| !value.is_empty());
    let has_worktree = parsed
        .worktree
        .as_deref()
        .is_some_and(|value| !value.is_empty());
    if !has_project && !has_worktree {
        return Ok(None);
    }
    if has_worktree && !has_project {
        let identifier = parsed.worktree.as_deref().unwrap_or_default();
        if let Some(project) = state.active_project()
            && let Some(worktree) = state
                .worktrees
                .get(&project.id)
                .and_then(|worktrees| find_worktree(worktrees, identifier))
        {
            return Ok(Some(ResolvedTarget {
                project_id: project.id.clone(),
                worktree_id: worktree.id.clone(),
            }));
        }
        let mut matches = state.workspace.projects.iter().filter_map(|project| {
            state
                .worktrees
                .get(&project.id)
                .and_then(|worktrees| find_worktree(worktrees, identifier))
                .map(|worktree| (project, worktree))
        });
        let Some((project, worktree)) = matches.next() else {
            return Err(format!("worktree not found {identifier}"));
        };
        if matches.next().is_some() {
            return Err(format!(
                "worktree '{identifier}' is ambiguous across projects; pass --project to disambiguate"
            ));
        }
        return Ok(Some(ResolvedTarget {
            project_id: project.id.clone(),
            worktree_id: worktree.id.clone(),
        }));
    }
    let project_identifier = parsed.project.as_deref().unwrap_or_default();
    let Some(project) = project_for(state, Some(project_identifier)) else {
        return Err(format!("project not found {project_identifier}"));
    };
    let worktrees = state
        .worktrees
        .get(&project.id)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let worktree = match parsed.worktree.as_deref().filter(|value| !value.is_empty()) {
        Some(identifier) => find_worktree(worktrees, identifier)
            .ok_or_else(|| format!("worktree not found {identifier}"))?,
        None => {
            preferred_worktree(state, project).ok_or_else(|| "worktree not found ".to_owned())?
        }
    };
    Ok(Some(ResolvedTarget {
        project_id: project.id.clone(),
        worktree_id: worktree.id.clone(),
    }))
}

pub fn preferred_worktree<'a>(state: &'a AppState, project: &Project) -> Option<&'a Worktree> {
    let worktrees = state.worktrees.get(&project.id)?;
    state
        .prefs
        .active_worktree_ids
        .get(&project.id)
        .and_then(|id| find_worktree(worktrees, id))
        .or_else(|| worktrees.iter().find(|worktree| worktree.is_primary))
        .or_else(|| worktrees.first())
}

pub fn is_directory(path: &str) -> bool {
    Path::new(path).is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_trailing_target_flag_pairs() {
        let parsed = parse_flags(&[
            "command",
            "--project",
            "literal",
            "--worktree",
            "feature",
            "--project",
            "Project",
        ]);
        assert_eq!(parsed.project.as_deref(), Some("literal"));
        assert_eq!(parsed.worktree.as_deref(), Some("feature"));
        assert_eq!(parsed.remaining, ["command"]);
    }

    #[test]
    fn standardizes_relative_paths_without_resolving_symlinks() {
        let expected = std::env::current_dir()
            .expect("current directory")
            .join("fixture")
            .to_string_lossy()
            .into_owned();
        assert_eq!(standardize_path("one/../fixture"), Some(expected));
    }

    #[test]
    fn duplicate_worktree_names_require_an_explicit_project() {
        let directory = tempfile::TempDir::new().expect("temporary directory");
        let mut state = crate::socket::commands::test_state(directory.path());
        let first = Project::new("First".to_owned(), "/tmp/first".to_owned(), 0);
        let second = Project::new("Second".to_owned(), "/tmp/second".to_owned(), 1);
        let mut first_worktree = muxy_core::store::worktrees::primary("feature", "/tmp/first");
        let mut second_worktree = muxy_core::store::worktrees::primary("feature", "/tmp/second");
        first_worktree.is_primary = false;
        second_worktree.is_primary = false;
        state
            .worktrees
            .insert(first.id.clone(), vec![first_worktree]);
        state
            .worktrees
            .insert(second.id.clone(), vec![second_worktree]);
        state.workspace = muxy_core::store::Workspace::for_tests(vec![first, second]);
        let parsed = parse_flags(&["--worktree", "feature"]);
        assert_eq!(
            resolve(&state, &parsed).err().as_deref(),
            Some("worktree 'feature' is ambiguous across projects; pass --project to disambiguate")
        );
    }
}
