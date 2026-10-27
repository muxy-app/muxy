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
