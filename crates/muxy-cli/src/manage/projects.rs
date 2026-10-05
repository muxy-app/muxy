use std::path::{Path, PathBuf};

use muxy_client::Client;
use muxy_protocol::{
    GitAction, GitReply, GitRequest, OperationId, ProjectDescriptor, ProjectId, ProjectIntent,
    ProjectMutation, ProjectPatch, WorktreeAction, WorktreeIntent,
};
use serde_json::{Value, json};

use super::args::{Project, Worktree};
use super::{Output, Paths, Result, absolute, local_path, path_text, server_path};

pub(super) fn resolve(client: &Client, selector: &str, paths: Paths) -> Result<ProjectDescriptor> {
    select(client.catalog()?.projects, selector, paths)
}

fn select(
    projects: Vec<ProjectDescriptor>,
    selector: &str,
    paths: Paths,
) -> Result<ProjectDescriptor> {
    if let Ok(id) = selector.parse::<ProjectId>() {
        return projects
            .into_iter()
            .find(|project| project.id == id)
            .ok_or_else(|| format!("project not found: {selector}").into());
    }
    let (path, canonical) = match paths {
        Paths::Local => {
            let path = absolute(Path::new(selector))?;
            let canonical = path.canonicalize().ok();
            (path, canonical)
        }
        Paths::Remote => (PathBuf::from(selector), None),
    };
    let mut matches = projects.into_iter().filter(|project| {
        let directory = local_path(&project.directory);
        project.name.eq_ignore_ascii_case(selector)
            || directory == path
            || canonical
                .as_ref()
                .is_some_and(|path| directory.canonicalize().ok().as_ref() == Some(path))
    });
    let found = matches
        .next()
        .ok_or_else(|| format!("project not found: {selector}"))?;
    if matches.next().is_some() {
        return Err(format!("ambiguous project: {selector}; use its ID").into());
    }
    Ok(found)
}

pub(super) fn record(project: &ProjectDescriptor) -> Value {
    json!({"id":project.id, "name":project.name, "directory":path_text(&project.directory),
        "directory_bytes":project.directory.0, "home":project.home, "parent_id":project.parent_id,
        "kind":project.kind, "color":project.color, "icon":project.icon})
}

pub(super) fn run(command: Project, client: &Client, output: &Output, paths: Paths) -> Result {
    match command {
        Project::List => output.list(
            &client
                .catalog()?
                .projects
                .iter()
                .map(record)
                .collect::<Vec<_>>(),
            &["id", "name", "directory"],
        ),
        Project::Add { directory, name } => {
            let mut directory = paths.directory(&directory)?;
            if paths == Paths::Local {
                directory = directory.canonicalize()?;
                if !directory.is_dir() {
                    return Err("project directory must exist".into());
                }
            }
            let name = name.unwrap_or_else(|| {
                directory
                    .file_name()
                    .unwrap_or(directory.as_os_str())
                    .to_string_lossy()
                    .into_owned()
            });
            let project = ProjectDescriptor {
                id: ProjectId::new(),
                home: false,
                name: name.trim().into(),
                icon: None,
                logo: None,
                color: "#808080".into(),
                directory: server_path(&directory),
                kind: None,
                parent_id: None,
            };
            mutate(client, ProjectMutation::Create(project.clone()))?;
            output.record(&record(&project), &["id", "name"])
        }
        Project::Delete(selector) => {
            mutate(
                client,
                ProjectMutation::Delete(resolve(client, &selector, paths)?.id),
            )?;
            output.ok()
        }
        Project::Rename { project, name } => {
            patch(client, &project, ProjectPatch::Name(name), output, paths)
        }
        Project::Color { project, color } => {
            patch(client, &project, ProjectPatch::Color(color), output, paths)
        }
        Project::Icon { project, icon } => {
            patch(client, &project, ProjectPatch::Icon(icon), output, paths)
        }
    }
}

fn mutate(client: &Client, mutation: ProjectMutation) -> Result {
    mutation
        .validate()
        .map_err(|code| format!("invalid project: {code:?}"))?;
    client.mutate_project(ProjectIntent {
        operation: OperationId::new(),
        mutation,
    })?;
    Ok(())
}
fn patch(
    client: &Client,
    selector: &str,
    patch: ProjectPatch,
    output: &Output,
    paths: Paths,
) -> Result {
    mutate(
        client,
        ProjectMutation::Patch {
            project: resolve(client, selector, paths)?.id,
            patch,
        },
    )?;
    output.ok()
}

pub(super) fn worktree(
    command: Worktree,
    client: &Client,
    output: &Output,
    paths: Paths,
) -> Result {
    let (selector, action) = match command {
        Worktree::List(project) => (project, GitAction::Worktrees),
        Worktree::Create {
            project,
            directory,
            branch,
            base,
        } => (
            project,
            intent(WorktreeAction::Create {
                project: ProjectId::new(),
                directory: server_path(&paths.directory(&directory)?),
                branch,
                base,
            }),
        ),
        Worktree::Register { project, directory } => (
            project,
            intent(WorktreeAction::Register {
                project: ProjectId::new(),
                directory: server_path(&paths.directory(&directory)?),
            }),
        ),
        Worktree::Remove(selector) => {
            let project = resolve(client, &selector, paths)?;
            if project.parent_id.is_none() {
                return Err("project is not a worktree".into());
            }
            let GitReply::Removal(expected) = client.git(GitRequest {
                project: project.id,
                action: GitAction::InspectRemoval,
            })?
            else {
                return Err("unexpected worktree inspection reply".into());
            };
            if expected.dirty {
                return Err("worktree has uncommitted changes; refusing removal".into());
            }
            client.git(GitRequest {
                project: project.id,
                action: intent(WorktreeAction::Remove { expected }),
            })?;
            return output.ok();
        }
    };
    let project = resolve(client, &selector, paths)?;
    let reply = client.git(GitRequest {
        project: project.parent_id.unwrap_or(project.id),
        action,
    })?;
    if let GitReply::Project(project) = reply {
        output.record(&record(&project), &["id", "name", "directory"])
    } else {
        Output::json(&reply)
    }
}
fn intent(action: WorktreeAction) -> GitAction {
    GitAction::Worktree(WorktreeIntent {
        options: None,
        operation: OperationId::new(),
        action,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str, directory: &str) -> ProjectDescriptor {
        ProjectDescriptor {
            id: ProjectId::new(),
            home: false,
            name: name.into(),
            directory: server_path(Path::new(directory)),
            icon: None,
            logo: None,
            color: "#808080".into(),
            kind: None,
            parent_id: None,
        }
    }

    #[test]
    fn ids_win_and_duplicate_names_or_paths_are_rejected() -> Result {
        let project = project("Example", "/tmp");
        let second = ProjectDescriptor {
            id: ProjectId::new(),
            ..project.clone()
        };
        let projects = vec![project.clone(), second];
        for paths in [Paths::Local, Paths::Remote] {
            assert!(select(projects.clone(), "example", paths).is_err());
            assert!(select(projects.clone(), "/tmp", paths).is_err());
            assert_eq!(
                select(projects.clone(), &project.id.to_string(), paths)?,
                project
            );
        }
        Ok(())
    }

    #[test]
    fn remote_selectors_match_names_and_exact_paths_only() -> Result {
        let directory = std::env::current_dir()?;
        let here = project("Here", &directory.to_string_lossy());
        let projects = vec![here.clone()];
        assert_eq!(select(projects.clone(), ".", Paths::Local)?, here);
        assert!(select(projects.clone(), ".", Paths::Remote).is_err());
        assert_eq!(
            select(
                projects.clone(),
                &directory.to_string_lossy(),
                Paths::Remote
            )?,
            here
        );
        assert_eq!(select(projects, "here", Paths::Remote)?, here);
        Ok(())
    }
}
