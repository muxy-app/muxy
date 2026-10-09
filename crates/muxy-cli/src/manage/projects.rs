use std::io::{self, Write};
use std::path::{Path, PathBuf};

use muxy_app_core::settings::Settings;
use muxy_client::Client;
use muxy_protocol::{
    GitAction, GitReply, GitRequest, GitWorktree, OperationId, ProjectDescriptor, ProjectId,
    ProjectIntent, ProjectMutation, ProjectPatch, WorktreeAction, WorktreeHook, WorktreeIntent,
    WorktreeOptions,
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
        Project::Add {
            directory,
            name,
            create,
            reuse,
        } => {
            let mut directory = paths.directory(&directory)?;
            if create {
                make_directory(client, &directory, paths)?;
            }
            if paths == Paths::Local {
                directory = directory.canonicalize()?;
                if !directory.is_dir() {
                    return Err("project directory must exist".into());
                }
            }
            let projects = client.catalog()?.projects;
            if reuse && let Some(existing) = existing(&projects, &directory) {
                return output.record(&record(existing), &["id", "name"]);
            }
            let mut project = ProjectDescriptor::new(
                server_path(&directory),
                projects.iter().map(|project| project.color.as_str()),
            );
            if let Some(name) = name {
                project.name = name.trim().into();
            }
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

/// The project for this computer's `folder`: one already there, or a new one.
pub(crate) fn folder_project(client: &Client, folder: &Path) -> Result<ProjectId> {
    let projects = client.catalog()?.projects;
    if let Some(project) = existing(&projects, folder) {
        return Ok(project.id);
    }
    let project = ProjectDescriptor::new(
        server_path(folder),
        projects.iter().map(|project| project.color.as_str()),
    );
    mutate(client, ProjectMutation::Create(project.clone()))?;
    Ok(project.id)
}

/// Creates `directory` and its parents. Another computer's are created by
/// running mkdir in its Home.
fn make_directory(client: &Client, directory: &Path, paths: Paths) -> Result {
    if paths == Paths::Local {
        std::fs::create_dir_all(directory)?;
        return Ok(());
    }
    let result = client.exec(muxy_protocol::ExecRequest {
        job: 1,
        project: client.catalog_page(None, None)?.home,
        argv: vec![
            "mkdir".into(),
            "-p".into(),
            "--".into(),
            directory.to_str().ok_or("directory must be UTF-8")?.into(),
        ],
        shell: None,
        cwd: None,
        env: std::collections::BTreeMap::new(),
        stdin: Vec::new(),
        timeout_ms: 30_000,
    })?;
    if result.exit_code != 0 || result.timed_out || result.cancelled {
        return Err(format!(
            "could not create {}: {}",
            directory.display(),
            result.stderr.trim()
        )
        .into());
    }
    Ok(())
}

/// The project already in `directory`, preferring one that isn't a worktree.
fn existing<'a>(
    projects: &'a [ProjectDescriptor],
    directory: &Path,
) -> Option<&'a ProjectDescriptor> {
    projects
        .iter()
        .filter(|project| {
            let path = local_path(&project.directory);
            path == directory || path.canonicalize().ok().as_deref() == Some(directory)
        })
        .min_by_key(|project| project.parent_id.is_some())
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
    match command {
        Worktree::List(selector) => {
            let root = root(client, &selector, paths)?;
            let GitReply::Worktrees(worktrees) = client.git(GitRequest {
                project: root.id,
                action: GitAction::Worktrees,
            })?
            else {
                return Err("unexpected worktree list reply".into());
            };
            output.list(
                &worktrees.iter().map(worktree_record).collect::<Vec<_>>(),
                &["project_id", "branch", "directory"],
            )
        }
        Worktree::Create {
            project,
            name,
            branch,
            base,
            directory,
            hooks,
        } => {
            let root = root(client, &project, paths)?;
            let directory = match directory {
                Some(directory) => paths.directory(&directory)?,
                None => default_directory(client, &root, &name, &branch, paths)?,
            };
            create(
                client,
                &root,
                WorktreeAction::Create {
                    project: ProjectId::new(),
                    directory: server_path(&directory),
                    branch,
                    base,
                },
                Some(name),
                hooks,
                output,
            )
        }
        Worktree::CheckoutPullRequest {
            project,
            number,
            name,
            directory,
            hooks,
        } => {
            let root = root(client, &project, paths)?;
            let folder = name.clone().unwrap_or_else(|| format!("pr-{number}"));
            let directory = match directory {
                Some(directory) => paths.directory(&directory)?,
                None => default_directory(client, &root, &folder, &folder, paths)?,
            };
            create(
                client,
                &root,
                WorktreeAction::CheckoutPullRequest {
                    project: ProjectId::new(),
                    directory: server_path(&directory),
                    number,
                },
                name,
                hooks,
                output,
            )
        }
        Worktree::Register { project, directory } => {
            let root = root(client, &project, paths)?;
            let reply = client.git(GitRequest {
                project: root.id,
                action: intent(
                    WorktreeAction::Register {
                        project: ProjectId::new(),
                        directory: server_path(&paths.directory(&directory)?),
                    },
                    None,
                ),
            })?;
            let GitReply::Project(project) = reply else {
                return Err("unexpected worktree registration reply".into());
            };
            output.record(&record(&project), &["id", "name", "directory"])
        }
        Worktree::Remove {
            worktree,
            force,
            hooks,
        } => {
            remove(client, &resolve(client, &worktree, paths)?, force, hooks)?;
            output.ok()
        }
    }
}

fn remove(client: &Client, project: &ProjectDescriptor, force: bool, hooks: bool) -> Result {
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
    if expected.dirty && !force {
        return Err("worktree has uncommitted changes; use --force to discard them".into());
    }
    let options = WorktreeOptions {
        name: None,
        hooks: if hooks {
            Some(approve(client, project.id, true)?)
        } else {
            None
        },
    };
    client.git(GitRequest {
        project: project.id,
        action: intent(WorktreeAction::Remove { expected }, Some(options)),
    })?;
    Ok(())
}

/// The project a worktree belongs to: `selector`'s parent if it is a worktree.
fn root(client: &Client, selector: &str, paths: Paths) -> Result<ProjectDescriptor> {
    let projects = client.catalog()?.projects;
    let project = select(projects.clone(), selector, paths)?;
    match project.parent_id {
        None => Ok(project),
        Some(parent) => projects
            .into_iter()
            .find(|project| project.id == parent)
            .ok_or_else(|| format!("the parent of {selector} no longer exists").into()),
    }
}

/// Where Settings → Worktrees puts a new worktree of `root`.
fn default_directory(
    client: &Client,
    root: &ProjectDescriptor,
    name: &str,
    branch: &str,
    paths: Paths,
) -> Result<PathBuf> {
    let path = Settings::default_path()?;
    let settings = if path.exists() {
        Settings::load(&path)?
    } else {
        Settings::default()
    };
    let location = settings
        .worktrees
        .projects
        .get(&root.id)
        .cloned()
        .unwrap_or_default();
    let home = match paths {
        Paths::Local => None,
        Paths::Remote => client
            .catalog()?
            .projects
            .iter()
            .find(|project| project.home)
            .map(|home| local_path(&home.directory)),
    };
    Ok(settings.worktrees.directory_in(
        &root.name,
        &local_path(&root.directory),
        &location,
        name,
        branch,
        home.as_deref(),
    )?)
}

fn create(
    client: &Client,
    root: &ProjectDescriptor,
    action: WorktreeAction,
    name: Option<String>,
    hooks: bool,
    output: &Output,
) -> Result {
    let requested = requested_name(&action, name.clone());
    let options = WorktreeOptions {
        name: requested.clone(),
        hooks: if hooks {
            Some(approve(client, root.id, false)?)
        } else {
            None
        },
    };
    let (mut project, failure) = match client.git(GitRequest {
        project: root.id,
        action: intent(action, Some(options)),
    })? {
        GitReply::Project(project) => (project, None),
        GitReply::WorktreeSetupFailed { project, message } => (project, Some(message)),
        _ => return Err("unexpected worktree creation reply".into()),
    };
    if let Some(name) = name.filter(|name| requested.is_none() && *name != project.name) {
        mutate(
            client,
            ProjectMutation::Patch {
                project: project.id,
                patch: ProjectPatch::Name(name.clone()),
            },
        )?;
        project.name = name;
    }
    output.record(&record(&project), &["id", "name", "directory"])?;
    match failure {
        Some(message) => Err(format!("worktree created, but setup failed: {message}").into()),
        None => Ok(()),
    }
}

/// The name the server gives a new worktree. A pull request's worktree must
/// start under the branch name the server prepares, because the server
/// checks out that name; it is renamed once it exists.
fn requested_name(action: &WorktreeAction, name: Option<String>) -> Option<String> {
    match action {
        WorktreeAction::CheckoutPullRequest { .. } => None,
        _ => name,
    }
}

/// The setup or teardown commands configured for `project`, printed as they
/// are approved.
fn approve(client: &Client, project: ProjectId, teardown: bool) -> Result<Vec<WorktreeHook>> {
    let GitReply::WorktreeHooks(hooks) = client.git(GitRequest {
        project,
        action: GitAction::WorktreeHooks { teardown },
    })?
    else {
        return Err("unexpected worktree hooks reply".into());
    };
    let stage = if teardown { "teardown" } else { "setup" };
    let mut stderr = io::stderr().lock();
    for hook in &hooks {
        writeln!(stderr, "{stage}: {}", hook.command)?;
    }
    Ok(hooks)
}

fn worktree_record(worktree: &GitWorktree) -> Value {
    json!({"project_id":worktree.registered, "branch":worktree.branch,
        "directory":path_text(&worktree.directory), "directory_bytes":worktree.directory.0,
        "head":worktree.head, "primary":worktree.primary, "locked":worktree.locked,
        "detached":worktree.detached, "prunable":worktree.prunable})
}

fn intent(action: WorktreeAction, options: Option<WorktreeOptions>) -> GitAction {
    GitAction::Worktree(WorktreeIntent {
        options,
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
    fn pull_request_worktrees_are_named_only_after_checkout() {
        let directory = server_path(Path::new("/tmp/w"));
        let checkout = WorktreeAction::CheckoutPullRequest {
            project: ProjectId::new(),
            directory: directory.clone(),
            number: 12,
        };
        assert_eq!(requested_name(&checkout, Some("review".into())), None);
        let create = WorktreeAction::Create {
            project: ProjectId::new(),
            directory,
            branch: "login".into(),
            base: None,
        };
        assert_eq!(
            requested_name(&create, Some("login".into())),
            Some("login".into())
        );
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
}
