use muxy_core::store::Project;
use std::path::{Component, Path, PathBuf};

pub const SUGGESTED_PATH_TEMPLATE: &str = "../{base-dir}.{branch}";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocationContext {
    pub home: PathBuf,
    pub profile_worktree_root: PathBuf,
    pub default_path_template: Option<String>,
    pub default_parent_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeLocationRequest {
    ConfiguredPrecedence,
    NativeAppDefault,
    NativeTemplate(String),
    NativeFolder(PathBuf),
    Explicit(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLocation {
    pub path: PathBuf,
    pub profile_managed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum WorktreeLocationError {
    #[error("Path template is required.")]
    PathTemplateRequired,
    #[error("Path template must include {{branch}}.")]
    BranchVariableRequired,
    #[error("Path template must keep {{branch}} in the resolved path.")]
    BranchVariableMustAffectPath,
    #[error("Path component is required.")]
    InvalidComponent,
    #[error("Could not create worktree parent directory: {0}")]
    CreateParent(#[source] std::io::Error),
}

pub fn sanitize_component(value: &str) -> Result<String, WorktreeLocationError> {
    let mut output = String::new();
    let mut pending_hyphen = false;
    for character in value.chars() {
        if character.is_alphanumeric() || matches!(character, '.' | '_') {
            if pending_hyphen && !output.is_empty() {
                output.push('-');
            }
            pending_hyphen = false;
            output.push(character);
        } else {
            pending_hyphen = true;
        }
    }
    while output.ends_with('-') {
        output.pop();
    }
    while output.starts_with('-') {
        output.remove(0);
    }
    if output.is_empty() || output == "." || output == ".." {
        return Err(WorktreeLocationError::InvalidComponent);
    }
    Ok(output)
}

pub fn validate_template(template: &str) -> Result<String, WorktreeLocationError> {
    let template = template.trim();
    if template.is_empty() {
        return Err(WorktreeLocationError::PathTemplateRequired);
    }
    if !template.contains("{branch}") {
        return Err(WorktreeLocationError::BranchVariableRequired);
    }
    let first = validation_path(template, "muxy-validation-a");
    let second = validation_path(template, "muxy-validation-b");
    if first == second {
        return Err(WorktreeLocationError::BranchVariableMustAffectPath);
    }
    Ok(template.to_owned())
}

pub fn resolve(
    project: &Project,
    slug: &str,
    branch: &str,
    request: WorktreeLocationRequest,
    context: &LocationContext,
) -> Result<ResolvedLocation, WorktreeLocationError> {
    match request {
        WorktreeLocationRequest::Explicit(path) => Ok(resolved(path, project, context, false)),
        WorktreeLocationRequest::NativeTemplate(template) => {
            resolve_template(&template, project, branch, context)
        }
        WorktreeLocationRequest::NativeFolder(folder) => Ok(ResolvedLocation {
            path: resolve_relative(expand_tilde_path(&folder, &context.home), project)
                .join(sanitize_component(slug)?),
            profile_managed: false,
        }),
        WorktreeLocationRequest::ConfiguredPrecedence => {
            if let Some(template) = normalized(project.preferred_worktree_path_template.as_deref())
            {
                return resolve_template(&template, project, branch, context);
            }
            if let Some(parent) = normalized(project.preferred_worktree_parent_path.as_deref()) {
                return Ok(resolve_parent(&parent, project, context)
                    .join(sanitize_component(slug)?)
                    .into());
            }
            resolve_app_default(project, slug, branch, context)
        }
        WorktreeLocationRequest::NativeAppDefault => {
            resolve_app_default(project, slug, branch, context)
        }
    }
}

pub fn create_parent(location: &ResolvedLocation) -> Result<(), WorktreeLocationError> {
    let Some(parent) = location.path.parent() else {
        return Err(WorktreeLocationError::InvalidComponent);
    };
    let mut created = Vec::new();
    if location.profile_managed {
        let mut current = parent;
        while !current.exists() {
            created.push(current.to_path_buf());
            let Some(next) = current.parent() else {
                break;
            };
            current = next;
        }
    }
    std::fs::create_dir_all(parent).map_err(WorktreeLocationError::CreateParent)?;
    #[cfg(unix)]
    if location.profile_managed {
        use std::os::unix::fs::PermissionsExt;
        for directory in created {
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
                .map_err(WorktreeLocationError::CreateParent)?;
        }
    }
    Ok(())
}

impl From<PathBuf> for ResolvedLocation {
    fn from(path: PathBuf) -> Self {
        Self {
            path,
            profile_managed: false,
        }
    }
}

fn resolve_app_default(
    project: &Project,
    slug: &str,
    branch: &str,
    context: &LocationContext,
) -> Result<ResolvedLocation, WorktreeLocationError> {
    if let Some(template) = normalized(context.default_path_template.as_deref()) {
        return resolve_template(&template, project, branch, context);
    }
    if let Some(parent) = normalized(context.default_parent_path.as_deref()) {
        return Ok(ResolvedLocation {
            path: resolve_parent(&parent, project, context)
                .join(directory_component(&project.name))
                .join(sanitize_component(slug)?),
            profile_managed: false,
        });
    }
    Ok(ResolvedLocation {
        path: lexical_normalize(
            &context
                .profile_worktree_root
                .join(&project.id)
                .join(sanitize_component(slug)?),
        ),
        profile_managed: true,
    })
}

fn resolve_template(
    template: &str,
    project: &Project,
    branch: &str,
    context: &LocationContext,
) -> Result<ResolvedLocation, WorktreeLocationError> {
    let template = validate_template(template)?;
    let base_directory = Path::new(&project.path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let replacements = [
        ("{project-name}", directory_component(&project.name)),
        ("{base-dir}", directory_component(base_directory)),
        (
            "{branch}",
            sanitize_component(branch).unwrap_or_else(|_| "branch".into()),
        ),
    ];
    let mut path = template;
    for (token, value) in replacements {
        path = path.replace(token, &value);
    }
    Ok(resolved(PathBuf::from(path), project, context, false))
}

fn directory_component(value: &str) -> String {
    sanitize_component(value).unwrap_or_else(|_| "project".into())
}

fn resolved(
    path: PathBuf,
    project: &Project,
    context: &LocationContext,
    profile_managed: bool,
) -> ResolvedLocation {
    ResolvedLocation {
        path: resolve_relative(expand_tilde_path(&path, &context.home), project),
        profile_managed,
    }
}

fn resolve_parent(parent: &str, project: &Project, context: &LocationContext) -> PathBuf {
    resolve_relative(expand_tilde_path(Path::new(parent), &context.home), project)
}

fn resolve_relative(path: PathBuf, project: &Project) -> PathBuf {
    if path.is_absolute() {
        lexical_normalize(&path)
    } else {
        lexical_normalize(&Path::new(&project.path).join(path))
    }
}

fn expand_tilde_path(path: &Path, home: &Path) -> PathBuf {
    if path == Path::new("~") {
        return home.to_path_buf();
    }
    if let Ok(rest) = path.strip_prefix("~/") {
        return home.join(rest);
    }
    path.to_path_buf()
}

fn normalized(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn validation_path(template: &str, branch: &str) -> PathBuf {
    let path = template
        .replace("{project-name}", "project")
        .replace("{base-dir}", "base")
        .replace("{branch}", branch);
    lexical_normalize(&Path::new("/muxy/project").join(path))
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !output.pop() && !path.is_absolute() {
                    output.push(component.as_os_str());
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                output.push(component.as_os_str());
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_core::store::Project;

    fn project(path: &std::path::Path) -> Project {
        let mut project = Project::new("My Project".into(), path.to_string_lossy().into_owned(), 0);
        project.id = "PROJECT-ID".into();
        project
    }

    #[test]
    fn worktree_location_follows_every_configured_precedence_and_folder_layout() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let profile = temp.path().join("profile/worktree-checkouts");
        let mut project = project(&repo);
        project.preferred_worktree_path_template = Some("../project-{branch}".into());
        project.preferred_worktree_parent_path = Some("../ignored-project-parent".into());
        let context = LocationContext {
            home: temp.path().join("home"),
            profile_worktree_root: profile.clone(),
            default_path_template: Some("../global-{branch}".into()),
            default_parent_path: Some(temp.path().join("global-parent").to_string_lossy().into()),
        };

        assert_eq!(
            resolve(
                &project,
                "feature-name",
                "feature/name",
                WorktreeLocationRequest::ConfiguredPrecedence,
                &context,
            )
            .unwrap()
            .path,
            temp.path().join("project-feature-name")
        );
        project.preferred_worktree_path_template = None;
        assert_eq!(
            resolve(
                &project,
                "feature-name",
                "feature/name",
                WorktreeLocationRequest::ConfiguredPrecedence,
                &context,
            )
            .unwrap()
            .path,
            temp.path().join("ignored-project-parent/feature-name")
        );
        project.preferred_worktree_parent_path = None;
        assert_eq!(
            resolve(
                &project,
                "feature-name",
                "feature/name",
                WorktreeLocationRequest::ConfiguredPrecedence,
                &context,
            )
            .unwrap()
            .path,
            temp.path().join("global-feature-name")
        );
        let mut parent_context = context.clone();
        parent_context.default_path_template = None;
        assert_eq!(
            resolve(
                &project,
                "feature-name",
                "feature/name",
                WorktreeLocationRequest::ConfiguredPrecedence,
                &parent_context,
            )
            .unwrap()
            .path,
            temp.path().join("global-parent/My-Project/feature-name")
        );
        parent_context.default_parent_path = None;
        let fallback = resolve(
            &project,
            "feature-name",
            "feature/name",
            WorktreeLocationRequest::ConfiguredPrecedence,
            &parent_context,
        )
        .unwrap();
        assert_eq!(fallback.path, profile.join("PROJECT-ID/feature-name"));
        assert!(fallback.profile_managed);
    }

    #[test]
    fn worktree_location_native_and_explicit_choices_have_distinct_layouts() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let mut project = project(&repo);
        project.preferred_worktree_path_template = Some("../ignored-{branch}".into());
        let context = LocationContext {
            home: temp.path().join("home"),
            profile_worktree_root: temp.path().join("profile"),
            default_path_template: None,
            default_parent_path: Some(temp.path().join("global").to_string_lossy().into()),
        };

        assert_eq!(
            resolve(
                &project,
                "slug",
                "branch",
                WorktreeLocationRequest::NativeAppDefault,
                &context,
            )
            .unwrap()
            .path,
            temp.path().join("global/My-Project/slug")
        );
        assert_eq!(
            resolve(
                &project,
                "slug",
                "branch",
                WorktreeLocationRequest::NativeFolder(temp.path().join("selected")),
                &context,
            )
            .unwrap()
            .path,
            temp.path().join("selected/slug")
        );
        assert_eq!(
            resolve(
                &project,
                "slug",
                "branch",
                WorktreeLocationRequest::NativeTemplate("../native-{branch}".into()),
                &context,
            )
            .unwrap()
            .path,
            temp.path().join("native-branch")
        );
        assert_eq!(
            resolve(
                &project,
                "---",
                "branch",
                WorktreeLocationRequest::Explicit(repo.join("../explicit/./target")),
                &context,
            )
            .unwrap()
            .path,
            temp.path().join("explicit/target")
        );
    }

    #[test]
    fn worktree_location_uses_swift_fallbacks_for_unsanitizable_names() {
        let temp = tempfile::tempdir().unwrap();
        let mut project = project(std::path::Path::new("/"));
        project.name = "///".into();
        let context = LocationContext {
            home: temp.path().join("home"),
            profile_worktree_root: temp.path().join("profile"),
            default_path_template: Some("{project-name}/{base-dir}/{branch}".into()),
            default_parent_path: None,
        };

        assert_eq!(
            resolve(
                &project,
                "slug",
                "branch",
                WorktreeLocationRequest::NativeAppDefault,
                &context,
            )
            .unwrap()
            .path,
            std::path::Path::new("/").join("project/project/branch")
        );

        let mut parent_context = context;
        parent_context.default_path_template = None;
        parent_context.default_parent_path =
            Some(temp.path().join("parent").to_string_lossy().into());
        assert_eq!(
            resolve(
                &project,
                "slug",
                "branch",
                WorktreeLocationRequest::NativeAppDefault,
                &parent_context,
            )
            .unwrap()
            .path,
            temp.path().join("parent/project/slug")
        );
    }

    #[test]
    fn worktree_location_validates_templates_sanitizes_and_expands_tilde() {
        let temp = tempfile::tempdir().unwrap();
        let project = project(&temp.path().join("repo"));
        let context = LocationContext {
            home: temp.path().join("home"),
            profile_worktree_root: temp.path().join("profile"),
            default_path_template: None,
            default_parent_path: None,
        };

        assert_eq!(sanitize_component("  f/é:::x  ").unwrap(), "f-é-x");
        assert_eq!(sanitize_component("---a---b---").unwrap(), "a-b");
        for invalid in ["", "---", ".", ".."] {
            assert!(sanitize_component(invalid).is_err());
        }
        assert!(matches!(
            validate_template("../without-token"),
            Err(WorktreeLocationError::BranchVariableRequired)
        ));
        assert!(matches!(
            validate_template("{branch}/.."),
            Err(WorktreeLocationError::BranchVariableMustAffectPath)
        ));
        assert_eq!(
            resolve(
                &project,
                "slug",
                "branch",
                WorktreeLocationRequest::NativeTemplate("~/trees/{project-name}/{branch}".into()),
                &context,
            )
            .unwrap()
            .path,
            temp.path().join("home/trees/My-Project/branch")
        );
    }

    #[cfg(unix)]
    #[test]
    fn worktree_location_creates_profile_roots_privately() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let resolved = ResolvedLocation {
            path: temp.path().join("private/PROJECT/slug"),
            profile_managed: true,
        };

        create_parent(&resolved).unwrap();

        assert_eq!(
            std::fs::metadata(temp.path().join("private"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}
