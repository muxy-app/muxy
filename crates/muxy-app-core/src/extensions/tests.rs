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
    let key = Grants::key("files", "exec", &json!({"argv":["git","status"]})).unwrap();
    grants.remember(&key, Consent::Allow).unwrap();
    assert_eq!(
        Grants::load(root.path()).unwrap().decision(&key),
        Consent::Allow
    );
    assert_ne!(
        key,
        Grants::key("git", "exec", &json!({"argv":["git","status"]})).unwrap()
    );
    assert_ne!(
        Grants::key("files", "exec", &json!({"shell":"git status"})),
        Grants::key("files", "exec", &json!({"shell":"git reset --hard"}))
    );
    grants.clear("files").unwrap();
    assert_eq!(grants.decision(&key), Consent::Ask);
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
        required_permission("modal.openWebview"),
        Some("panels:write")
    );
    assert_eq!(required_permission("exec.start"), Some("commands:exec"));
    assert_eq!(
        api::git_reply("git.commit", GitReply::Commit("abc123".into())).unwrap(),
        json!({"hash":"abc123"})
    );
}
