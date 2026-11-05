use std::cell::RefCell;

use super::*;

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes
}

fn package(folder: &Path, manifest: &Value) {
    fs::create_dir_all(folder).unwrap();
    fs::write(folder.join("package.json"), manifest.to_string()).unwrap();
}

fn ready_package(folder: &Path) {
    package(
        folder,
        &json!({
            "name": "hello",
            "version": "1.2.0",
            "scripts": { "build": "vite build" },
            "muxy": {
                "description": "Says hello",
                "permissions": ["notifications:write"],
                "marketplace": {
                    "icon": "assets/icon.svg",
                    "screenshots": ["assets/one.png"]
                }
            }
        }),
    );
    fs::create_dir_all(folder.join("public/assets")).unwrap();
    fs::write(folder.join("public/assets/icon.svg"), "<svg/>").unwrap();
    fs::write(folder.join("public/assets/one.png"), png(1600, 1000)).unwrap();
    fs::write(folder.join("README.md"), "# Hello").unwrap();
    fs::write(folder.join("package-lock.json"), "{}").unwrap();
}

#[test]
fn a_complete_package_is_ready_and_lists_its_files() {
    let folder = tempfile::tempdir().unwrap();
    ready_package(folder.path());
    for left_out in [
        "dist/index.html",
        "node_modules/a/b.js",
        "logs/output.log",
        ".claude/x",
        ".env",
    ] {
        let path = folder.path().join(left_out);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "x").unwrap();
    }
    let readiness = check(folder.path());
    assert_eq!(readiness.problems, Vec::<String>::new());
    assert_eq!(readiness.name, "hello");
    assert_eq!(readiness.version, "1.2.0");
    assert_eq!(readiness.permissions, ["notifications:write"]);
    assert_eq!(
        readiness.files,
        [
            Path::new("README.md"),
            Path::new("package-lock.json"),
            Path::new("package.json"),
            Path::new("public/assets/icon.svg"),
            Path::new("public/assets/one.png"),
        ]
    );
}

#[test]
fn everything_the_store_requires_is_reported_at_once() {
    let folder = tempfile::tempdir().unwrap();
    package(
        folder.path(),
        &json!({ "name": "Hello_World", "version": "1.0.0", "muxy": {} }),
    );
    let problems = check(folder.path()).problems.join("\n");
    assert!(problems.contains("a-z"), "{problems}");
    assert!(problems.contains("scripts.build"), "{problems}");
    assert!(problems.contains("README.md"), "{problems}");
    assert!(problems.contains("marketplace block"), "{problems}");
    assert!(problems.contains("package-lock.json"), "{problems}");
}

#[test]
fn names_and_versions_follow_the_stores_rules() {
    assert!(publishable_name("hello-world.2"));
    for name in ["", "-hello", ".hello", "Hello", "a_b", "a/b"] {
        assert!(!publishable_name(name), "{name:?}");
    }
    let folder = tempfile::tempdir().unwrap();
    package(
        folder.path(),
        &json!({ "name": "hello", "version": "1.0 beta", "muxy": {} }),
    );
    let problems = check(folder.path()).problems.join("\n");
    assert!(problems.contains("semantic version"), "{problems}");
}

#[test]
fn credentials_and_agent_files_are_left_out_in_any_case_and_anywhere() {
    for path in [
        "id_ed25519",
        "keys/id_rsa",
        ".ENV",
        ".env.local",
        ".envrc",
        "prod.env",
        "KEY.PEM",
        "certs/app.p12",
        "app.pfx",
        ".npmrc",
        ".netrc",
        ".cursor/rules",
        ".codex/config.toml",
        ".opencode/x",
        "src/.claude/skill.md",
        "AGENTS.md",
        "node_modules/a/index.js",
        "src/node_modules/a.js",
        "dist/index.js",
        "logs/output.log",
        "debug.log",
    ] {
        assert!(left_out(Path::new(path)), "{path}");
    }
    for path in [
        "src/logs/view.ts",
        "src/dist/helper.ts",
        "src/release/notes.ts",
        "environment.ts",
        "package.json",
        "README.md",
    ] {
        assert!(!left_out(Path::new(path)), "{path}");
    }
}

#[test]
fn the_pull_request_uses_the_sources_permissions_not_a_stale_build() {
    let folder = tempfile::tempdir().unwrap();
    ready_package(folder.path());
    fs::create_dir_all(folder.path().join("dist")).unwrap();
    fs::write(
        folder.path().join("dist/package.json"),
        json!({"name": "hello", "version": "1.2.0", "muxy": {"permissions": []}}).to_string(),
    )
    .unwrap();
    assert_eq!(check(folder.path()).permissions, ["notifications:write"]);
}

#[test]
fn listing_images_must_meet_the_stores_limits() {
    let folder = tempfile::tempdir().unwrap();
    ready_package(folder.path());
    package(
        folder.path(),
        &json!({
            "name": "hello",
            "version": "1.0.0",
            "scripts": { "build": "x" },
            "muxy": { "marketplace": {
                "icon": "small.png",
                "screenshots": ["square.png", "missing.png", "../outside.png"]
            }}
        }),
    );
    fs::write(folder.path().join("small.png"), png(64, 64)).unwrap();
    fs::write(folder.path().join("square.png"), png(800, 800)).unwrap();
    let problems = check(folder.path()).problems.join("\n");
    assert!(problems.contains("square PNG"), "{problems}");
    assert!(problems.contains("square.png must be"), "{problems}");
    assert!(problems.contains("missing.png wasn't found"), "{problems}");
    assert!(
        problems.contains("../outside.png wasn't found"),
        "{problems}"
    );
}

#[test]
fn a_published_version_must_be_bumped() {
    let published = Published {
        version: "1.2.0".into(),
        author: None,
    };
    let mut same = Readiness {
        version: "1.2.0".into(),
        ..Readiness::default()
    };
    check_against_store(&mut same, Some(&published));
    assert_eq!(same.problems.len(), 1);
    let mut newer = Readiness {
        version: "1.3.0".into(),
        ..Readiness::default()
    };
    check_against_store(&mut newer, Some(&published));
    assert!(newer.problems.is_empty());
    check_against_store(&mut same, None);
    assert_eq!(same.problems.len(), 1);
}

/// Stands in for `gh` and `git`, recording what ran.
#[derive(Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Each is a separate way a tool can answer"
)]
struct FakeTools {
    calls: RefCell<Vec<String>>,
    gh_missing: bool,
    signed_out: bool,
    open_pull_request: Option<&'static str>,
    taken_branches: Vec<&'static str>,
    pushes_failing: RefCell<usize>,
    nothing_changed: bool,
    has_identity: bool,
    published: Option<&'static str>,
    offline: bool,
    added: RefCell<Vec<String>>,
}

impl Tools for FakeTools {
    fn installed(&self, program: &str) -> bool {
        program != "gh" || !self.gh_missing
    }

    fn run(
        &self,
        program: &str,
        arguments: &[&str],
        directory: Option<&Path>,
    ) -> Result<String, String> {
        let line = format!("{program} {}", arguments.join(" "));
        self.calls.borrow_mut().push(line.clone());
        match arguments {
            ["api", "user", ..] if self.offline => Err("gh failed: could not resolve host".into()),
            ["api", "user", ..] if self.signed_out => Err("gh failed: not logged in".into()),
            ["api", "user", ..] => Ok("octocat".into()),
            ["api", "-X", "POST", ..] => Ok("octocat/extensions".into()),
            ["api", "-H", ..] => self
                .published
                .map(str::to_owned)
                .ok_or_else(|| "gh failed: HTTP 404".to_owned()),
            ["pr", "list", ..] => {
                let head = arguments[5];
                Ok(match self.open_pull_request {
                    Some(url) if head == "hello-1.2.0" => format!(
                        r#"[{{"url":"{url}","headRepositoryOwner":{{"login":"octocat"}}}},{{"url":"other","headRepositoryOwner":{{"login":"someone"}}}}]"#
                    ),
                    _ => r#"[{"url":"other","headRepositoryOwner":{"login":"someone"}}]"#.into(),
                })
            }
            ["pr", "create", ..] => Ok("Creating…\nhttps://github.com/pr/1".into()),
            ["ls-remote", _, _, reference] => Ok(self
                .taken_branches
                .iter()
                .find(|branch| reference.ends_with(**branch))
                .map_or_else(String::new, |branch| format!("abc\trefs/heads/{branch}"))),
            ["clone", .., target] => {
                fs::create_dir_all(Path::new(target).join("extensions/hello"))
                    .map_err(|error| error.to_string())?;
                fs::write(Path::new(target).join("extensions/hello/stale.js"), "old")
                    .map_err(|error| error.to_string())?;
                Ok(String::new())
            }
            ["add", .., folder] => {
                let root = directory.unwrap_or(Path::new(".")).join(folder);
                let (files, _) = files_to_publish(&root).map_err(|error| error.to_string())?;
                *self.added.borrow_mut() = files
                    .iter()
                    .map(|file| file.display().to_string())
                    .collect();
                Ok(String::new())
            }
            ["status", ..] if self.nothing_changed => Ok(String::new()),
            ["status", ..] => Ok("A  extensions/hello/package.json".into()),
            ["config", ..] if self.has_identity => Ok("me@example.com".into()),
            ["config", ..] => Err("git failed: unset".into()),
            [.., "push", _, _] | [.., "push", "--force", _, _] => {
                let mut failing = self.pushes_failing.borrow_mut();
                if *failing > 0 {
                    *failing -= 1;
                    return Err("git failed: repository not found".into());
                }
                Ok(String::new())
            }
            _ => Ok(String::new()),
        }
    }
}

fn readiness() -> Readiness {
    Readiness {
        name: "hello".into(),
        version: "1.2.0".into(),
        description: "Says hello".into(),
        permissions: vec!["notifications:write".into()],
        ..Readiness::default()
    }
}

fn publish_with(tools: &FakeTools) -> Result<PullRequest, String> {
    let source = tempfile::tempdir().unwrap();
    ready_package(source.path());
    fs::create_dir_all(source.path().join("dist")).unwrap();
    fs::write(source.path().join("dist/built.js"), "x").unwrap();
    let approved = Readiness {
        files: check(source.path()).files,
        ..readiness()
    };
    let staged = stage(source.path(), &approved.files).unwrap();
    publish(tools, staged.path(), &approved, "octocat", false)
}

#[test]
fn what_gets_sent_is_what_was_there_when_the_user_was_asked() {
    let source = tempfile::tempdir().unwrap();
    ready_package(source.path());
    let approved = Readiness {
        files: check(source.path()).files,
        ..readiness()
    };
    let staged = stage(source.path(), &approved.files).unwrap();
    fs::write(source.path().join("notes-with-token.txt"), "secret").unwrap();
    fs::write(source.path().join("README.md"), "changed while asking").unwrap();

    let tools = FakeTools::default();
    publish(&tools, staged.path(), &approved, "octocat", false).unwrap();

    let added = tools.added.borrow();
    assert!(added.iter().any(|file| file == "README.md"), "{added:?}");
    assert!(
        !added.iter().any(|file| file.contains("notes-with-token")),
        "{added:?}"
    );
    assert!(
        !added.iter().any(|file| file.contains("stale")),
        "the store's old copy is replaced: {added:?}"
    );
    assert_eq!(
        fs::read_to_string(staged.path().join("README.md")).unwrap(),
        "# Hello"
    );
}

#[test]
fn staging_refuses_what_changed_since_it_was_listed() {
    let linked_files = |change: &str| {
        let source = tempfile::tempdir().unwrap();
        ready_package(source.path());
        let files = check(source.path()).files;
        let readme = source.path().join("README.md");
        fs::remove_file(&readme).unwrap();
        if change == "link" {
            std::os::unix::fs::symlink("/etc/hosts", &readme).unwrap();
        }
        stage(source.path(), &files).unwrap_err()
    };
    assert!(linked_files("removed").contains("changed while checking"));
    assert!(linked_files("link").contains("changed while checking"));

    let folder = tempfile::tempdir().unwrap();
    ready_package(&folder.path().join("real"));
    let files = check(&folder.path().join("real")).files;
    let link = folder.path().join("link");
    std::os::unix::fs::symlink(folder.path().join("real"), &link).unwrap();
    assert!(stage(&link, &files).is_err(), "a linked folder is refused");
}

#[test]
fn a_link_doesnt_count_as_a_required_file_and_pipes_are_never_sent() {
    let folder = tempfile::tempdir().unwrap();
    ready_package(folder.path());
    fs::remove_file(folder.path().join("README.md")).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", folder.path().join("README.md")).unwrap();
    let pipe = folder.path().join("pipe");
    let made = std::process::Command::new("mkfifo")
        .arg(&pipe)
        .status()
        .unwrap();
    assert!(made.success());
    let readiness = check(folder.path());
    assert!(readiness.problems.join(" ").contains("README.md"));
    assert!(!readiness.files.iter().any(|file| file == Path::new("pipe")));
}

#[test]
fn a_screenshot_entry_must_be_a_path() {
    let folder = tempfile::tempdir().unwrap();
    ready_package(folder.path());
    package(
        folder.path(),
        &json!({
            "name": "hello",
            "version": "1.0.0",
            "scripts": { "build": "x" },
            "muxy": { "marketplace": { "icon": "assets/icon.svg", "screenshots": [null, 123] } }
        }),
    );
    let problems = check(folder.path()).problems;
    let about_entries = problems
        .iter()
        .filter(|problem| problem.contains("must be a file path"))
        .count();
    assert_eq!(about_entries, 2, "{problems:?}");
}

#[test]
fn every_github_request_goes_to_github_com() {
    let tools = FakeTools::default();
    publish_with(&tools).unwrap();
    github_account(&tools);
    let _ = published_version(&tools, "hello");
    let calls = tools.calls.borrow();
    for call in calls.iter().filter(|call| call.starts_with("gh api")) {
        assert!(call.contains("--hostname github.com"), "{call}");
    }
    for call in calls.iter().filter(|call| call.starts_with("gh pr")) {
        assert!(
            call.contains("--repo github.com/muxy-app/extensions"),
            "{call}"
        );
    }
}

fn position(calls: &[String], text: &str) -> usize {
    calls
        .iter()
        .position(|call| call.contains(text))
        .unwrap_or_else(|| panic!("{text} never ran: {calls:#?}"))
}

#[test]
fn publishing_forks_commits_pushes_and_opens_a_pull_request_in_order() {
    let tools = FakeTools::default();
    let request = publish_with(&tools).unwrap();
    assert_eq!(
        request,
        PullRequest {
            url: "https://github.com/pr/1".into(),
            branch: "hello-1.2.0".into(),
            updated: false,
        }
    );
    let calls = tools.calls.borrow();
    let order = [
        "repos/muxy-app/extensions/forks",
        "git clone --filter=blob:none --sparse https://github.com/muxy-app/extensions",
        "sparse-checkout set extensions/hello",
        "checkout -b hello-1.2.0",
        "add --all -- extensions/hello",
        "commit --allow-empty -m hello 1.2.0",
        "https://github.com/octocat/extensions.git HEAD:refs/heads/hello-1.2.0",
        "pr create --repo github.com/muxy-app/extensions --base main --head octocat:hello-1.2.0 --title hello 1.2.0",
    ];
    let positions: Vec<_> = order.iter().map(|text| position(&calls, text)).collect();
    assert!(positions.is_sorted(), "{calls:#?}");
    assert!(
        !calls.iter().any(|call| call.contains("--force")),
        "{calls:#?}"
    );
}

#[test]
fn the_commit_uses_a_github_identity_only_when_git_has_none() {
    let tools = FakeTools::default();
    publish_with(&tools).unwrap();
    assert!(tools.calls.borrow().iter().any(|call| {
        call.contains("user.email=octocat@users.noreply.github.com") && call.contains("commit")
    }));
    let configured = FakeTools {
        has_identity: true,
        ..FakeTools::default()
    };
    publish_with(&configured).unwrap();
    assert!(
        !configured
            .calls
            .borrow()
            .iter()
            .any(|call| call.contains("user.email="))
    );
}

#[test]
fn publishing_the_same_version_again_updates_its_open_pull_request() {
    let tools = FakeTools {
        open_pull_request: Some("https://github.com/pr/9"),
        ..FakeTools::default()
    };
    let request = publish_with(&tools).unwrap();
    assert_eq!(request.url, "https://github.com/pr/9");
    assert!(request.updated);
    let calls = tools.calls.borrow();
    assert!(
        calls.iter().any(|call| call.contains("push --force")),
        "{calls:#?}"
    );
    assert!(
        !calls.iter().any(|call| call.contains("pr create")),
        "{calls:#?}"
    );
}

#[test]
fn another_persons_pull_request_is_not_mistaken_for_the_users() {
    let tools = FakeTools::default();
    assert!(!publish_with(&tools).unwrap().updated);
}

#[test]
fn a_branch_the_user_pushed_by_hand_is_never_overwritten() {
    let tools = FakeTools {
        taken_branches: vec!["hello-1.2.0"],
        ..FakeTools::default()
    };
    let request = publish_with(&tools).unwrap();
    assert_eq!(request.branch, "hello-1.2.0-2");
    assert!(
        !tools
            .calls
            .borrow()
            .iter()
            .any(|call| call.contains("--force"))
    );
}

#[test]
fn a_push_is_retried_while_a_new_fork_settles() {
    let tools = FakeTools {
        pushes_failing: RefCell::new(2),
        ..FakeTools::default()
    };
    publish_with(&tools).unwrap();
    assert_eq!(
        tools
            .calls
            .borrow()
            .iter()
            .filter(|call| call.contains(" push "))
            .count(),
        3
    );
    let gone = FakeTools {
        pushes_failing: RefCell::new(100),
        ..FakeTools::default()
    };
    assert!(
        publish_with(&gone)
            .unwrap_err()
            .contains("repository not found")
    );
    assert!(
        !gone
            .calls
            .borrow()
            .iter()
            .any(|call| call.contains("pr create"))
    );
}

#[test]
fn nothing_new_opens_no_pull_request() {
    let tools = FakeTools {
        nothing_changed: true,
        ..FakeTools::default()
    };
    assert!(publish_with(&tools).is_err());
    let calls = tools.calls.borrow();
    assert!(
        !calls.iter().any(|call| call.contains(" push ")),
        "{calls:#?}"
    );
}

#[test]
fn the_account_says_why_publishing_needs_help() {
    let missing = FakeTools {
        gh_missing: true,
        ..FakeTools::default()
    };
    assert_eq!(github_account(&missing), GitHub::Missing);
    let signed_out = FakeTools {
        signed_out: true,
        ..FakeTools::default()
    };
    assert_eq!(github_account(&signed_out), GitHub::SignedOut);
    assert_eq!(
        github_account(&FakeTools::default()),
        GitHub::User("octocat".into())
    );
    let offline = FakeTools {
        offline: true,
        ..FakeTools::default()
    };
    assert!(matches!(github_account(&offline), GitHub::Unreachable(_)));
    let folder = tempfile::tempdir().unwrap();
    ready_package(folder.path());
    assert!(matches!(
        plan(&offline, folder.path()),
        Err(PublishFailure::Failed(reason)) if reason.contains("resolve host")
    ));
}

#[test]
fn a_store_without_the_extension_is_a_new_listing() {
    assert_eq!(published_version(&FakeTools::default(), "hello"), Ok(None));
}

#[test]
fn the_body_follows_the_stores_template() {
    let body = pull_request_body(&readiness(), true);
    assert!(body.contains("- **Name:** hello"));
    assert!(body.contains("- **New or update:** update"));
    assert!(body.contains("- `notifications:write`"));
    assert!(body.contains("- [x] Bumped `version`"));
    assert!(body.contains("- [ ] Only permissions actually used"));
}

#[test]
fn a_package_that_isnt_ready_never_reaches_github() {
    let folder = tempfile::tempdir().unwrap();
    package(
        folder.path(),
        &json!({"name": "a/b", "version": "1.0.0", "muxy": {}}),
    );
    let tools = FakeTools::default();
    assert!(matches!(
        plan(&tools, folder.path()),
        Err(PublishFailure::NotReady(_))
    ));
    assert!(
        tools.calls.borrow().is_empty(),
        "{:?}",
        tools.calls.borrow()
    );
}

#[test]
fn a_ready_package_without_gh_asks_for_guidance() {
    let folder = tempfile::tempdir().unwrap();
    ready_package(folder.path());
    let missing = FakeTools {
        gh_missing: true,
        ..FakeTools::default()
    };
    assert!(matches!(
        plan(&missing, folder.path()),
        Err(PublishFailure::NeedsGitHub(reason)) if reason.contains("isn't installed")
    ));
    let signed_out = FakeTools {
        signed_out: true,
        ..FakeTools::default()
    };
    assert!(matches!(
        plan(&signed_out, folder.path()),
        Err(PublishFailure::NeedsGitHub(reason)) if reason.contains("gh auth login")
    ));
}

#[test]
fn a_plan_changes_nothing_and_says_what_the_user_is_approving() {
    let folder = tempfile::tempdir().unwrap();
    ready_package(folder.path());
    let tools = FakeTools::default();
    let plan = plan(&tools, folder.path()).unwrap();
    assert!(!plan.update);
    assert!(!tools.calls.borrow().iter().any(|call| {
        call.contains("POST") || call.starts_with("git ") || call.contains("pr create")
    }));
    let message = plan.confirmation();
    for expected in [
        "public pull request against muxy-app/extensions",
        "octocat",
        "Branch: hello-1.2.0",
        "Pull request: hello 1.2.0",
        "public/assets/icon.svg",
    ] {
        assert!(message.contains(expected), "{expected}: {message}");
    }
}

#[test]
fn updating_someone_elses_extension_is_called_out() {
    let folder = tempfile::tempdir().unwrap();
    ready_package(folder.path());
    let tools = FakeTools {
        published: Some(r#"{"version":"1.0.0","muxy":{"marketplace":{"github":"someone"}}}"#),
        ..FakeTools::default()
    };
    let plan = plan(&tools, folder.path()).unwrap();
    assert!(plan.update);
    assert_eq!(plan.other_author.as_deref(), Some("someone"));
    assert!(plan.confirmation().contains("already published by someone"));
}

#[test]
fn an_unbumped_version_stops_before_any_approval() {
    let folder = tempfile::tempdir().unwrap();
    ready_package(folder.path());
    let tools = FakeTools {
        published: Some(r#"{"version":"1.2.0","muxy":{}}"#),
        ..FakeTools::default()
    };
    assert!(matches!(
        plan(&tools, folder.path()),
        Err(PublishFailure::NotReady(problems)) if problems[0].contains("Bump")
    ));
}

#[test]
fn problems_read_as_a_list() {
    assert_eq!(
        not_ready_message(&["Add a README.".into(), "Add an icon.".into()]),
        "Fix these before publishing:\n• Add a README.\n• Add an icon."
    );
}
