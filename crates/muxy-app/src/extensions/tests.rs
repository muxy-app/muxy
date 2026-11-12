#![allow(
    clippy::unwrap_used,
    reason = "Compatibility fixtures fail immediately"
)]

use serde_json::Value;

#[test]
fn unchanged_manifests_load_without_api_changes() {
    for source in [
        include_str!("../../fixtures/extensions/git.json"),
        include_str!("../../fixtures/extensions/files.json"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let package: Value = serde_json::from_str(source).unwrap();
        std::fs::write(root.path().join("package.json"), source).unwrap();
        let manifest = &package["muxy"];
        let resources = manifest["panels"]
            .as_array()
            .unwrap()
            .iter()
            .chain(manifest["tabTypes"].as_array().unwrap())
            .map(|item| item["entry"].as_str().unwrap())
            .chain(
                manifest["commands"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|command| {
                        command["action"]["entry"]
                            .as_str()
                            .or(command["action"]["script"].as_str())
                    }),
            );
        for relative in resources {
            let path = root.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        let extension = muxy_app_core::extensions::Extension::load(root.path()).unwrap();
        assert_eq!(extension.name, package["name"]);
        assert!(!extension.manifest.commands.is_empty());
    }
}

const TYPES: &str = include_str!("../../../../docs/extensions/types/muxy.d.ts");

/// Whether the types declare `name` as a member.
fn declared(name: &str) -> bool {
    [
        format!("{name}("),
        format!("{name}:"),
        format!("\"{name}\"("),
    ]
    .iter()
    .any(|form| TYPES.contains(form.as_str()))
}

/// The names that open lines indented by exactly `indent` spaces.
fn members(block: &str, indent: usize) -> Vec<&str> {
    block
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix(&" ".repeat(indent))?;
            let rest = rest.strip_prefix("get ").unwrap_or(rest);
            let end = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_')?;
            let next = rest.as_bytes().get(end + 1).copied();
            let method = rest.as_bytes()[end] == b'(' && !matches!(next, Some(b'\'' | b'"'));
            (end > 0 && (method || matches!(rest.as_bytes()[end], b':' | b',')))
                .then(|| &rest[..end])
        })
        .collect()
}

#[test]
fn the_published_types_declare_every_api_member() {
    let bridge = include_str!("bridge.js");
    let mut missing = Vec::new();
    let mut checked = 0;
    for (index, _) in bridge.match_indices('\n') {
        let line = &bridge[index + 1..];
        let indent = line.len() - line.trim_start_matches(' ').len();
        let Some(rest) = line[indent..].strip_prefix("muxy.") else {
            continue;
        };
        let Some((namespace, after)) = rest.split_once(" = ") else {
            continue;
        };
        if namespace.contains(|c: char| !c.is_ascii_alphanumeric()) {
            continue;
        }
        if !declared(namespace) {
            missing.push(namespace.to_owned());
        }
        if after.starts_with('{') {
            let close = format!("\n{}}};", " ".repeat(indent));
            let block = &after[..after.find(&close).unwrap_or(0)];
            for member in members(block, indent + 2) {
                checked += 1;
                if !declared(member) {
                    missing.push(format!("{namespace}.{member}"));
                }
            }
        }
    }
    let page = include_str!("../views/webview/bridge.js");
    let start = page.find("window.muxy = Object.freeze({").unwrap();
    let end = start + page[start..].find("\n  });").unwrap();
    for member in members(&page[start..end], 4) {
        checked += 1;
        if !declared(member) {
            missing.push(format!("page {member}"));
        }
    }
    assert!(missing.is_empty(), "muxy.d.ts lacks {missing:?}");
    assert!(checked > 150, "only {checked} members were found to check");
}
