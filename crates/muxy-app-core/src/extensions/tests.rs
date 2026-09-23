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
fn every_main_manifest_field_loads_with_main_defaults() {
    let root = tempfile::tempdir().unwrap();
    write_package(
        root.path(),
        &json!({
            "$schema": "ignored",
            "marketplace": {"icon": "icon.svg"},
            "background": "background.js",
            "permissions": ["panels:write", "tabs:write", "storage:read"],
            "events": ["panel.opened", "project.switched"],
            "tabTypes": [{"id":"tab","title":"Tab","entry":"tab.html","defaultData":{"surface":"tab"}}],
            "homeViews": [{"id":"home","title":"Home","entry":"home.html","icon":"house"}],
            "panels": [{"id":"panel","entry":"panel.html","hiddenControls":["pin","icon"],
                "headerButtons":[{"id":"refresh","icon":{"svg":"icon.svg"},"command":"refresh"}]}],
            "popovers": [{"id":"pop","entry":"pop.html"}],
            "sidebar": {"id":"nav","entry":"nav.html","icon":{"symbol":"sidebar.left"}},
            "fileOpeners": [{"id":"open","tabType":"tab"}],
            "localizations": [{"id":"de","language":"de","title":"Deutsch","bundle":"German.bundle"}],
            "commands": [
                {"id":"refresh","title":"Refresh"},
                {"id":"pop","title":"Pop","action":{"kind":"openPopover","popover":"pop"}},
                {"id":"modal","title":"Modal","action":{"kind":"openModal","entry":"modal.html"}},
                {"id":"event","title":"Event","action":{"kind":"event"}}
            ],
            "topbarItems": [{"id":"top","icon":"sparkles","command":"pop"}],
            "statusBarItems": [{"id":"status","icon":{"svg":"icon.svg"},"side":"right","command":"refresh","visible":false}],
            "settings": [{"key":"size","title":"Size","type":"number","defaultValue":200}],
            "remoteMethods": [{"id":"forecast"}]
        }),
        &[
            ("background.js", ""),
            ("tab.html", ""),
            ("home.html", ""),
            ("panel.html", ""),
            ("pop.html", ""),
            ("nav.html", ""),
            ("modal.html", ""),
            ("icon.svg", "<svg/>"),
            ("German.bundle/Info.plist", "<plist/>"),
            (
                "German.bundle/de.lproj/Localizable.strings",
                "\"A\" = \"B\";",
            ),
        ],
    );
    let extension = Extension::load(root.path()).unwrap();
    let manifest = &extension.manifest;
    assert_eq!(manifest.tab_types[0].default_data, json!({"surface":"tab"}));
    let panel = &manifest.panels[0];
    assert_eq!(
        (panel.position, panel.mode),
        (PanelPosition::Right, PanelMode::Floating)
    );
    assert!(panel.hides(PanelControl::Icon) && !panel.allows_mode_selection());
    assert_eq!(
        (manifest.popovers[0].width, manifest.popovers[0].height),
        (320.0, 360.0)
    );
    assert_eq!(manifest.command("refresh").unwrap().action, Action::Event);
    assert!(matches!(
        manifest.command("modal").unwrap().action,
        Action::OpenModal {
            dismiss_on_outside_click: true,
            ..
        }
    ));
    assert!(manifest.topbar_items[0].visible && !manifest.status_bar_items[0].visible);
    assert_eq!(manifest.status_bar_items[0].side, Side::Right);
    let opener = &manifest.file_openers[0];
    assert!(opener.singleton && opener.matches("src/Main.RS"));
    assert_eq!(manifest.settings[0].kind, SettingKind::Number);
    assert!(extension.allows_event("command.refresh") && extension.allows_event("panel.opened"));
    assert!(!extension.allows_event("tab.created") && !extension.allows_event("command.other"));
}

#[test]
fn packages_prefer_their_dist_build_output_like_main() {
    let profile = tempfile::tempdir().unwrap();
    let package = profile.path().join("extensions/sample");
    fs::create_dir_all(package.join("dist")).unwrap();
    fs::write(
        package.join("package.json"),
        json!({"name":"sample","version":"0.0.1","muxy":{"background":"missing.js"}}).to_string(),
    )
    .unwrap();
    write_package(
        &package.join("dist"),
        &json!({"background":"background.js"}),
        &[("background.js", "")],
    );
    let registry = Registry::load(profile.path());
    assert!(registry.errors.is_empty(), "{:?}", registry.errors);
    let extension = &registry.extensions["sample"];
    assert!(extension.directory.ends_with("dist"));
    assert_eq!(extension.version, "1.0.0");
    let root_only = tempfile::tempdir().unwrap();
    fs::create_dir_all(root_only.path().join("dist/panel")).unwrap();
    fs::write(root_only.path().join("dist/panel/index.html"), "").unwrap();
    fs::write(
        root_only.path().join("package.json"),
        json!({"name":"sample","version":"1.0.0","muxy":{"panels":[{"id":"p","entry":"panel/index.html"}]}})
            .to_string(),
    )
    .unwrap();
    assert!(Extension::load(root_only.path()).is_ok());
}

#[test]
fn manifest_validation_rejects_what_main_rejects() {
    let cases = [
        json!({"permissions":["files:execute"]}),
        json!({"commands":[{"id":"a","title":"A","action":{"kind":"openPopover","popover":"none"}}]}),
        json!({"commands":[{"id":"a","title":"A","action":{"kind":"launch"}}]}),
        json!({"statusBarItems":[{"id":"s","icon":"x","side":"left","command":"missing"}]}),
        json!({"statusBarItems":[{"id":"s","icon":"x","side":"middle","command":"a"}],"commands":[{"id":"a","title":"A"}]}),
        json!({"topbarItems":[{"id":"t","icon":{"symbol":"x","svg":"y.svg"},"command":"a"}],"commands":[{"id":"a","title":"A"}]}),
        json!({"topbarItems":[{"id":"t","icon":{"svg":"large.svg"},"command":"a"}],"commands":[{"id":"a","title":"A"}]}),
        json!({"topbarItems":[{"id":"t","icon":{"svg":"absent.svg"},"command":"a"}],"commands":[{"id":"a","title":"A"}]}),
        json!({"settings":[{"key":"k","title":"K","type":"bool"},{"key":"k","title":"K","type":"bool"}]}),
        json!({"settings":[{"key":"k","title":"K","type":"color"}]}),
        json!({"remoteMethods":[{"id":"a|b"}]}),
        json!({"background":"absent.js"}),
        json!({"localizations":[{"id":"de","language":"de","title":"Deutsch","bundle":"Empty.bundle"}]}),
        json!({"fileOpeners":[{"id":"o","tabType":"absent"}]}),
        json!({"homeViews":[{"id":"h","title":"","entry":"index.html"}]}),
    ];
    for muxy in cases {
        let root = tempfile::tempdir().unwrap();
        write_package(
            root.path(),
            &muxy,
            &[
                ("index.html", ""),
                ("large.svg", &"x".repeat(256 * 1024 + 1)),
                ("Empty.bundle/Info.plist", ""),
            ],
        );
        assert!(Extension::load(root.path()).is_err(), "accepted {muxy}");
    }
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
fn storage_limits_and_messages_match_main() {
    let root = tempfile::tempdir().unwrap();
    package(&root.path().join("first"), "first");
    let extension = Extension::load(&root.path().join("first")).unwrap();
    let storage = Storage::new(root.path());
    let error =
        |args: serde_json::Value| storage.call(&extension, "storage.set", &args).unwrap_err();
    assert_eq!(
        error(json!({"key":"","value":1})),
        "storage key must not be empty"
    );
    assert_eq!(
        error(json!({"key":"k".repeat(257),"value":1})),
        "storage key exceeds 256 characters"
    );
    assert_eq!(
        error(json!({"key":"big","value":"x".repeat(1_048_577)})),
        "storage value exceeds 1048576 bytes"
    );
    assert_eq!(error(json!({"key":"k"})), "storage.set requires a value");
    for key in ["b", "a"] {
        storage
            .call(&extension, "storage.set", &json!({"key":key,"value":null}))
            .unwrap();
    }
    assert_eq!(
        storage
            .call(&extension, "storage.keys", &json!({}))
            .unwrap(),
        json!(["a", "b"])
    );
}

#[test]
fn settings_overrides_are_declared_bounded_and_survive_reloads() {
    let root = tempfile::tempdir().unwrap();
    write_package(
        root.path(),
        &json!({"settings":[{"key":"size","title":"Size","type":"number","defaultValue":200}]}),
        &[],
    );
    let extension = Extension::load(root.path()).unwrap();
    let settings = Settings::new(root.path());
    assert!(settings.set(&extension, "other", Some(json!(1))).is_err());
    settings.set(&extension, "size", Some(json!(50))).unwrap();
    assert_eq!(settings.values("sample").unwrap()["size"], json!(50));
    settings.set(&extension, "size", None).unwrap();
    assert!(settings.values("sample").unwrap().is_empty());
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
