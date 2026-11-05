#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "Tests fail immediately on fixture errors"
)]
use super::*;
use serde_json::json;
use std::fs;

fn package(root: &Path, name: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("index.html"), "<!doctype html>").unwrap();
    fs::write(root.join("package.json"),json!({"name":name,"version":"1.0.0","muxy":{"permissions":["files:read","panels:write"],"tabTypes":[{"id":"editor","title":"Editor","entry":"index.html"}],"commands":[{"id":"open","title":"Open","action":{"kind":"openTab","tabType":"editor"}}]}}).to_string()).unwrap();
}

#[test]
fn unpacked_packages_require_explicit_enable_and_unload_keeps_sources() {
    let profile = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    package(&source.path().join("dist"), "files");
    let mut registry = Registry::load(profile.path());
    assert_eq!(registry.load_unpacked(source.path()).unwrap(), "files");
    assert!(registry.enabled("files").is_none());
    assert!(registry.load_unpacked(source.path()).is_err());
    registry.set_enabled("files", true).unwrap();
    let mut restored = Registry::load(profile.path());
    assert!(restored.enabled("files").is_some());
    restored.remove("files").unwrap();
    assert!(source.path().join("dist/package.json").is_file());
    assert!(Registry::load(profile.path()).extensions.is_empty());
}

#[test]
fn reloading_replaces_old_load_errors_instead_of_piling_them_up() {
    let profile = tempfile::tempdir().unwrap();
    let broken = Registry::load(profile.path()).directory().join("broken");
    fs::create_dir_all(&broken).unwrap();
    fs::write(broken.join("package.json"), "not json").unwrap();
    let mut registry = Registry::load(profile.path());
    assert_eq!(registry.errors.len(), 1);
    registry.reload();
    registry.reload();
    assert_eq!(registry.errors.len(), 1);
    fs::remove_dir_all(&broken).unwrap();
    registry.reload();
    assert!(registry.errors.is_empty());
}

#[test]
fn a_saved_preference_error_outlives_a_reload() {
    let profile = tempfile::tempdir().unwrap();
    fs::write(profile.path().join("extension-enabled.json"), "{}").unwrap();
    let mut registry = Registry::load(profile.path());
    assert_eq!(registry.errors.len(), 1);
    registry.reload();
    assert_eq!(registry.errors.len(), 1);
}

#[test]
fn reloading_one_extension_reads_only_it_and_keeps_it_when_broken() {
    let profile = tempfile::tempdir().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    package(first.path(), "first");
    package(second.path(), "second");
    let mut registry = Registry::load(profile.path());
    registry.load_unpacked(first.path()).unwrap();
    registry.load_unpacked(second.path()).unwrap();
    assert_eq!(
        registry.source_folder("first"),
        Some(first.path().canonicalize().unwrap().as_path())
    );

    let bump = |root: &Path, name: &str, version: &str| {
        fs::write(
            root.join("package.json"),
            json!({"name": name, "version": version, "muxy": {}}).to_string(),
        )
        .unwrap();
    };
    bump(first.path(), "first", "2.0.0");
    bump(second.path(), "second", "2.0.0");
    registry.reload_one("first").unwrap();
    assert_eq!(registry.extensions["first"].version, "2.0.0");
    assert_eq!(registry.extensions["second"].version, "1.0.0");

    fs::write(first.path().join("package.json"), "not json").unwrap();
    assert!(registry.reload_one("first").is_err());
    assert_eq!(registry.extensions["first"].version, "2.0.0");

    bump(first.path(), "renamed", "3.0.0");
    assert!(
        registry
            .reload_one("first")
            .unwrap_err()
            .contains("renamed")
    );
    assert_eq!(registry.extensions["first"].version, "2.0.0");
    assert!(registry.reload_one("missing").is_err());
}

#[test]
fn reloading_a_folder_that_failed_at_startup_loads_it_and_clears_its_error() {
    let profile = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    package(source.path(), "hello");
    Registry::load(profile.path())
        .load_unpacked(source.path())
        .unwrap();
    let good = fs::read(source.path().join("package.json")).unwrap();
    fs::write(source.path().join("package.json"), "not json").unwrap();
    let mut registry = Registry::load(profile.path());
    assert!(registry.extensions.is_empty());
    assert_eq!(registry.errors.len(), 1);
    fs::write(source.path().join("package.json"), good).unwrap();
    registry.reload_one("hello").unwrap();
    assert!(registry.extensions.contains_key("hello"));
    assert!(registry.errors.is_empty(), "{:?}", registry.errors);
}

#[test]
fn a_folder_that_no_longer_loads_can_still_be_unloaded() {
    let profile = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    package(source.path(), "gone");
    let mut registry = Registry::load(profile.path());
    registry.load_unpacked(source.path()).unwrap();
    registry.set_enabled("gone", true).unwrap();
    fs::remove_file(source.path().join("package.json")).unwrap();

    let mut restarted = Registry::load(profile.path());
    assert!(restarted.extensions.is_empty());
    assert_eq!(restarted.errors.len(), 1);
    assert!(restarted.set_enabled("gone", true).is_err());
    restarted.remove("gone").unwrap();
    assert!(restarted.errors.is_empty(), "{:?}", restarted.errors);

    let later = Registry::load(profile.path());
    assert!(later.errors.is_empty(), "{:?}", later.errors);
    assert!(!later.is_unpacked("gone"));
}

#[test]
fn a_folder_cant_be_loaded_twice_under_two_names() {
    let profile = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    package(source.path(), "hello");
    let mut registry = Registry::load(profile.path());
    registry.load_unpacked(source.path()).unwrap();
    package(source.path(), "renamed");
    let error = registry.load_unpacked(source.path()).unwrap_err();
    assert!(error.contains("'hello'"), "{error}");
    assert!(!registry.extensions.contains_key("renamed"));
}

#[test]
fn manifests_reject_escaping_resources_and_invalid_commands() {
    let root = tempfile::tempdir().unwrap();
    package(root.path(), "files");
    let extension = Extension::load(root.path()).unwrap();
    assert!(extension.resource("../package.json").is_err());
    assert!(extension.resource("/etc/passwd").is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/etc/passwd", root.path().join("outside")).unwrap();
        assert!(extension.resource("outside").is_err());
    }
    let path = root.path().join("package.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["muxy"]["commands"][0]["action"]["tabType"] = json!("absent");
    fs::write(path, value.to_string()).unwrap();
    assert!(Extension::load(root.path()).is_err());
}

#[test]
fn grants_scope_exact_shell_and_program_and_storage_is_per_extension() {
    let root = tempfile::tempdir().unwrap();
    let mut grants = Grants::load(root.path()).unwrap();
    let request = |args: serde_json::Value| Request::for_call("files", "exec", &args, "").unwrap();
    let status = request(json!({"argv":["git","status"]}));
    grants
        .remember("files", &status, Choice::AllowAndRemember)
        .unwrap();
    assert_eq!(
        Grants::load(root.path())
            .unwrap()
            .decision("files", &status),
        Consent::Allow
    );
    assert_eq!(grants.decision("git", &status), Consent::Ask);
    assert_eq!(
        grants.decision("files", &request(json!({"argv":["git","push"]}))),
        Consent::Allow
    );
    let shell = request(json!({"shell":"git status"}));
    let reset = request(json!({"shell":"git reset --hard"}));
    grants
        .remember("files", &shell, Choice::AllowAndRemember)
        .unwrap();
    assert_eq!(grants.decision("files", &reset), Consent::Ask);
    grants.remember("files", &reset, Choice::Block).unwrap();
    assert_eq!(grants.decision("files", &status), Consent::Blocked);
    assert_eq!(grants.decision("files", &shell), Consent::Blocked);
    grants.clear("files").unwrap();
    assert_eq!(grants.decision("files", &status), Consent::Ask);
    package(&root.path().join("first"), "first");
    package(&root.path().join("second"), "second");
    let first = Extension::load(&root.path().join("first")).unwrap();
    let second = Extension::load(&root.path().join("second")).unwrap();
    let storage = Storage::new(root.path());
    storage
        .call(
            &first,
            "storage.set",
            &json!({"key":"cache","value":{"number":42}}),
        )
        .unwrap();
    assert_eq!(
        storage
            .call(&first, "storage.get", &json!({"key":"cache"}))
            .unwrap(),
        json!({"number":42})
    );
    assert_eq!(
        storage
            .call(&second, "storage.get", &json!({"key":"cache"}))
            .unwrap(),
        serde_json::Value::Null
    );
}

#[test]
fn files_and_git_bridge_payloads_match_main_contract() {
    use muxy_protocol::*;
    assert_eq!(
        api::files_action(
            "files.write",
            &json!({"path":"readme.md","contents":"hello"})
        )
        .unwrap(),
        FilesAction::Write {
            path: ServerPath(b"readme.md".to_vec()),
            content: "hello".into()
        }
    );
    assert_eq!(
        api::files_reply(FilesReply::Content(FileContent {
            path: ServerPath(b"a.rs".to_vec()),
            content: "fn main() {}".into(),
            size: 12
        }))
        .unwrap(),
        json!({"path":"a.rs","content":"fn main() {}","size":12})
    );
    assert_eq!(
        api::git_action(
            "git.diff",
            &json!({"filePath":"src/main.rs","staged":true,"raw":true})
        )
        .unwrap(),
        GitAction::Diff(GitDiffRequest {
            path: Some(ServerPath(b"src/main.rs".to_vec())),
            raw: true,
            staged: true,
            line_limit: None
        })
    );
    assert!(api::files_action("files.move", &json!({"paths":[5],"into":"src"})).is_err());
    assert!(api::git_action("git.pr.merge", &json!({"number":0})).is_err());
    assert!(api::files_reply(FilesReply::Path(ServerPath(vec![0xff]))).is_err());
    assert_eq!(
        required_permission("modal.openWebview", &json!({})),
        Some("panels:write")
    );
    assert_eq!(
        required_permission("exec.start", &json!({})),
        Some("commands:exec")
    );
    for (args, permission) in [
        (
            json!({"tabId": "t", "args": [{"text": "Done"}]}),
            "browser:read",
        ),
        (json!({"tabId": "t", "function": "go()"}), "browser:write"),
        (
            json!({"tabId": "t", "args": [{"function": "go()"}]}),
            "browser:write",
        ),
    ] {
        assert_eq!(
            required_permission("browser.wait", &args),
            Some(permission),
            "{args}"
        );
    }
    assert_eq!(
        api::git_reply("git.commit", GitReply::Commit("abc123".into())).unwrap(),
        json!({"hash":"abc123"})
    );
}

fn write_package(root: &Path, muxy: &serde_json::Value, files: &[(&str, &str)]) {
    for (relative, contents) in files {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    fs::write(
        root.join("package.json"),
        json!({"name":"sample","version":"1.0.0","muxy":muxy}).to_string(),
    )
    .unwrap();
}

#[test]
fn earlier_grant_keys_keep_their_decisions() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("extension-grants.json"),
        json!({"git:git.commit":"Allow","files:files.write":"Deny","files:exec:argv:rg":"Allow"})
            .to_string(),
    )
    .unwrap();
    let grants = Grants::load(root.path()).unwrap();
    let call = |owner: &str, verb: &str, args: serde_json::Value| {
        grants.decision(
            owner,
            &Request::for_call(owner, verb, &args, "/repo").unwrap(),
        )
    };
    assert_eq!(call("git", "git.commit", json!({})), Consent::Allow);
    assert_eq!(call("git", "git.push", json!({})), Consent::Ask);
    assert_eq!(
        call("files", "files.write", json!({"path":"a"})),
        Consent::Deny
    );
    assert_eq!(
        call("files", "exec", json!({"argv":["rg","needle"]})),
        Consent::Allow
    );
    assert!(Request::for_call("git", "git.status", &json!({}), "").is_none());
    assert!(Request::for_call("git", "git.worktree.switch", &json!({}), "").is_none());
    let own = json!({"kind":"extensionWebView","extension":{"id":"git","tabType":"diff"}});
    assert!(Request::for_call("git", "tabs.open", &own, "").is_none());
    let browser = json!({"kind":"browser","url":"localhost:3000"});
    assert!(Request::for_call("git", "tabs.open", &browser, "").is_none());
    let foreign = Request::for_call(
        "git",
        "tabs.open",
        &json!({"kind":"extensionWebView","extension":{"id":"files","tabType":"code-editor"}}),
        "",
    )
    .unwrap();
    assert_eq!(foreign.gate, Gate::TabsOpenForeign);
    let command = Request::for_call(
        "git",
        "tabs.open",
        &json!({"kind":"terminal","command":" npm test "}),
        "",
    )
    .unwrap();
    assert_eq!(
        (command.gate, command.summary.as_str()),
        (Gate::TabsRunCommand, "npm test")
    );
    assert!(
        Request::for_call(
            "git",
            "tabs.open",
            &json!({"kind":"terminal","directory":"src"}),
            ""
        )
        .is_none()
    );
}

#[test]
fn event_subscriptions_need_their_read_permissions() {
    let root = tempfile::tempdir().unwrap();
    write_package(
        root.path(),
        &json!({"events":["agent.status","file.changed","projects.changed"],"permissions":["files:read"]}),
        &[],
    );
    let extension = Extension::load(root.path()).unwrap();
    assert!(extension.allows_event("file.changed"));
    assert!(!extension.allows_event("agent.status"));
    assert!(!extension.allows_event("projects.changed"));
    assert!(extension.allows_event("extension.sample.refresh"));
    assert!(!extension.allows_event("extension."));
    assert!(!extension.allows_event("extension.bad name"));
}

#[test]
fn manifests_load_whatever_main_accepts() {
    let root = tempfile::tempdir().unwrap();
    write_package(
        root.path(),
        &json!({
            "permissions": null,
            "description": null,
            "commands": [
                {"id": "a", "title": "A", "action": null, "subtitle": null},
                {"id": "a", "title": "Again"},
            ],
            "tabTypes": [{"id": "", "title": "Unnamed", "entry": "index.html"}],
            "panels": [{"id": "p", "entry": "index.html", "hiddenControls": null, "defaultData": {"keep": null}}],
            "fileOpeners": [{"id": "o", "tabType": "", "patterns": ["*", "a*b", "Read?e.MD"], "singleton": null}],
        }),
        &[("index.html", "")],
    );
    let extension = Extension::load(root.path()).unwrap();
    let manifest = &extension.manifest;
    assert_eq!(manifest.commands[0].action, Action::Event);
    assert_eq!(manifest.command("a").map(|c| c.title.as_str()), Some("A"));
    assert_eq!(manifest.panels[0].default_data, json!({"keep": null}));
    let opener = &manifest.file_openers[0];
    assert!(opener.singleton, "a null singleton keeps main's default");
    for path in ["*a", "a*xb", "readme.md", "notes/any"] {
        assert!(opener.matches(path), "{path}");
    }
    let only = |pattern: &str| FileOpener {
        patterns: vec![pattern.into()],
        ..opener.clone()
    };
    assert!(!only("a*b").matches("a*x"));
    assert!(!only("?").matches("ab"));
    assert!(only("src/*.rs").matches("SRC/lib.RS"));
}

#[test]
fn a_one_x_sidebar_manifest_loads_unchanged() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("sidebar")).unwrap();
    fs::write(root.path().join("sidebar/index.html"), "<!doctype html>").unwrap();
    fs::write(
        root.path().join("package.json"),
        json!({
            "name": "workspace-nav",
            "version": "0.1.0",
            "private": true,
            "type": "module",
            "scripts": { "dev": "vite", "build": "vite build" },
            "devDependencies": { "vite": "^5.0.0" },
            "muxy": {
                "permissions": ["projects:read", "projects:write"],
                "events": ["project.switched"],
                "sidebar": {
                    "id": "main",
                    "title": "Workspace",
                    "icon": "sidebar.left",
                    "entry": "sidebar/index.html"
                }
            }
        })
        .to_string(),
    )
    .unwrap();
    let extension = Registry::unpacked_extension(root.path()).unwrap();
    let sidebar = extension.manifest.sidebar.unwrap();
    assert_eq!(sidebar.id, "main");
    assert_eq!(sidebar.title.as_deref(), Some("Workspace"));
    assert_eq!(sidebar.entry, "sidebar/index.html");
}
