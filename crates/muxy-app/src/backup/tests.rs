#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::io::Write;

use muxy_app_core::{AppState, ServerId, settings::Settings};

use super::*;

fn profile() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    Settings::load(&directory.path().join("settings.toml")).unwrap();
    fs::write(directory.path().join("ghostty.conf"), "font-size = 17\n").unwrap();
    directory
}

#[test]
fn backup_round_trip_is_portable_and_restore_waits_for_restart() {
    let source = profile();
    let target = profile();
    let project = tempfile::tempdir().unwrap();
    let mut state = AppState::bootstrap().unwrap();
    let id = state
        .add_project(ServerId::local(), project.path().into())
        .unwrap();
    state.open_terminal_tab(id).unwrap();
    fs::create_dir(source.path().join("themes")).unwrap();
    fs::write(source.path().join("themes/custom"), "background = 111111").unwrap();
    fs::create_dir(source.path().join("extension-settings")).unwrap();
    fs::write(
        source.path().join("extension-settings/example.json"),
        r#"{"enabled":true}"#,
    )
    .unwrap();
    fs::write(source.path().join("identity.pem"), "private identity").unwrap();
    fs::write(
        source.path().join("server.toml"),
        "history_budget_bytes = 1048576\nshell_integration = false\n",
    )
    .unwrap();
    let archive = source.path().join("test.muxy");
    export(source.path(), &archive, &state).unwrap();
    let (files, legacy, complete) = archive::read(&archive).unwrap();
    assert!(!legacy);
    assert!(complete);
    assert!(!files.contains_key("identity.pem"));
    let saved: AppState = serde_json::from_slice(&files["desktop-state.json"]).unwrap();
    assert!(saved.project_intents(ServerId::local()).is_empty());
    fs::write(target.path().join("ghostty.conf"), "font-size = 12\n").unwrap();
    let import = prepare(target.path(), &archive).unwrap();
    stage(target.path(), &import).unwrap();
    assert_eq!(
        fs::read_to_string(target.path().join("ghostty.conf")).unwrap(),
        "font-size = 12\n"
    );
    apply_pending(target.path()).unwrap();
    assert_eq!(
        fs::read_to_string(target.path().join("ghostty.conf")).unwrap(),
        "font-size = 17\n"
    );
    assert!(target.path().join("themes/custom").exists());
    assert!(
        target
            .path()
            .join("extension-settings/example.json")
            .exists()
    );
    let restored = muxy_app_core::store::load(target.path().join("desktop-state.json")).unwrap();
    assert!(restored.project(id).is_some());
    let creates = restored
        .project_intents(ServerId::local())
        .iter()
        .filter(|intent| matches!(intent.mutation, muxy_protocol::ProjectMutation::Create(_)))
        .count();
    assert_eq!(creates, 1);
    let recovery = fs::read_dir(target.path().join("Backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        fs::read_to_string(recovery.join("ghostty.conf")).unwrap(),
        "font-size = 12\n"
    );
    assert!(!target.path().join(PENDING).exists());
    apply_pending(target.path()).unwrap();
}

#[test]
fn invalid_archive_and_cancel_leave_current_configuration_untouched() {
    let directory = profile();
    let archive = directory.path().join("test.muxy");
    export(directory.path(), &archive, &AppState::bootstrap().unwrap()).unwrap();
    stage(
        directory.path(),
        &prepare(directory.path(), &archive).unwrap(),
    )
    .unwrap();
    cancel_pending(directory.path()).unwrap();
    apply_pending(directory.path()).unwrap();
    assert!(!directory.path().join("Backups").exists());
    let (mut files, _, _) = archive::read(&archive).unwrap();
    files.insert(
        "settings.toml".into(),
        b"[window]\ndefault_size = [0, 0]".to_vec(),
    );
    archive::write(&archive, &files, true).unwrap();
    let before = fs::read(directory.path().join("settings.toml")).unwrap();
    assert!(prepare(directory.path(), &archive).is_err());
    assert_eq!(
        fs::read(directory.path().join("settings.toml")).unwrap(),
        before
    );
}

fn legacy_archive(path: &Path, extra: &str) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(br#"{"schemaVersion":1,"appVersion":"1.0","files":["settings.json"]}"#)
        .unwrap();
    zip.start_file("settings.json", options).unwrap();
    zip.write_all(br#"{"muxy.theme.dark":"Dracula","muxy.quickTerminal.width":900,"browser.homePageURL":"https://example.com"}"#).unwrap();
    if !extra.is_empty() {
        zip.start_file(extra, options).unwrap();
        zip.write_all(b"unsafe").unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn native_one_x_archive_and_manual_json_use_the_same_conversion() {
    let directory = profile();
    let archive = directory.path().join("legacy.muxy");
    legacy_archive(&archive, "");
    let import = prepare(directory.path(), &archive).unwrap();
    assert!(import.summary.contains("Skipped 1"));
    stage(directory.path(), &import).unwrap();
    apply_pending(directory.path()).unwrap();
    let settings = Settings::load(&directory.path().join("settings.toml")).unwrap();
    assert_eq!(settings.appearance.dark_theme, "Dracula");
    assert_eq!(settings.quick_terminal.width, 900);
    assert_eq!(
        fs::read_to_string(directory.path().join("ghostty.conf")).unwrap(),
        "font-size = 17\n"
    );
    let json = directory.path().join("settings.json");
    fs::write(
        &json,
        r#"{"muxy.quickTerminal.height":650,"mobile.approvedDevices":["secret"]}"#,
    )
    .unwrap();
    let import = prepare(directory.path(), &json).unwrap();
    assert!(!String::from_utf8_lossy(&import.files["settings.toml"]).contains("secret"));
    stage(directory.path(), &import).unwrap();
    apply_pending(directory.path()).unwrap();
    assert_eq!(
        Settings::load(&directory.path().join("settings.toml"))
            .unwrap()
            .quick_terminal
            .height,
        650
    );
}

#[test]
fn traversal_symlinks_missing_manifest_and_unknown_versions_are_rejected() {
    let directory = profile();
    let archive = directory.path().join("unsafe.muxy");
    for name in [
        "../escape",
        "/absolute",
        "themes/../../escape",
        "themes\\escape",
    ] {
        legacy_archive(&archive, name);
        assert!(prepare(directory.path(), &archive).is_err(), "{name}");
    }
    let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
    zip.start_file("manifest.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(
        br#"{"schemaVersion":2,"format":"muxy.configuration","files":["settings.toml"]}"#,
    )
    .unwrap();
    zip.add_symlink(
        "settings.toml",
        "/etc/passwd",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.finish().unwrap();
    assert!(prepare(directory.path(), &archive).is_err());
    let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
    zip.start_file("manifest.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(br#"{"schemaVersion":99,"files":[]}"#)
        .unwrap();
    zip.finish().unwrap();
    assert!(prepare(directory.path(), &archive).is_err());
    fs::write(&archive, b"not a ZIP").unwrap();
    assert!(prepare(directory.path(), &archive).is_err());
    std::os::unix::fs::symlink("/etc/passwd", directory.path().join("themes")).unwrap();
    assert!(export(directory.path(), &archive, &AppState::bootstrap().unwrap()).is_err());
}

#[test]
fn ghostty_includes_are_resolved_and_untrusted_imports_cannot_read_external_files() {
    let directory = profile();
    fs::write(directory.path().join("included.conf"), "font-size = 21\n").unwrap();
    fs::write(
        directory.path().join("ghostty.conf"),
        "font-size = 12\nconfig-file = included.conf\n",
    )
    .unwrap();
    let archive = directory.path().join("test.muxy");
    export(directory.path(), &archive, &AppState::bootstrap().unwrap()).unwrap();
    let (mut files, _, _) = archive::read(&archive).unwrap();
    let source = String::from_utf8_lossy(&files["ghostty.conf"]);
    assert!(!source.contains("config-file"));
    assert!(source.contains("font-size = 21"));
    files.insert("ghostty.conf".into(), b"config-file = /etc/passwd".to_vec());
    archive::write(&archive, &files, true).unwrap();
    assert!(prepare(directory.path(), &archive).is_err());
}

#[test]
fn recovery_archive_resolves_includes_and_keeps_original_files_for_rollback() {
    let source = profile();
    let target = profile();
    let original = "font-size = 12\nconfig-file = included.conf\n";
    fs::write(target.path().join("ghostty.conf"), original).unwrap();
    fs::write(target.path().join("included.conf"), "font-size = 21\n").unwrap();
    let archive = source.path().join("test.muxy");
    export(source.path(), &archive, &AppState::bootstrap().unwrap()).unwrap();
    stage(target.path(), &prepare(target.path(), &archive).unwrap()).unwrap();
    assert!(apply_pending(target.path()).unwrap().is_none());
    let recovery = fs::read_dir(target.path().join("Backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        fs::read_to_string(recovery.join("ghostty.conf")).unwrap(),
        original
    );
    fs::remove_file(target.path().join("included.conf")).unwrap();
    let import = prepare(target.path(), &recovery.join("recovery.muxy")).unwrap();
    stage(target.path(), &import).unwrap();
    assert!(apply_pending(target.path()).unwrap().is_none());
    let terminal = TerminalSettings::load(&target.path().join("ghostty.conf")).unwrap();
    assert_eq!(terminal.font_size.to_bits(), 21.0_f32.to_bits());
}

#[test]
fn interrupted_restore_recovers_original_files_before_loading_the_profile() {
    let directory = profile();
    let recovery = directory.path().join("Backups/pre-import-test");
    fs::create_dir_all(&recovery).unwrap();
    fs::write(recovery.join("ghostty.conf"), "font-size = 17\n").unwrap();
    fs::write(recovery.join("roots.json"), r#"["ghostty.conf","themes"]"#).unwrap();
    fs::write(
        directory.path().join("restore-in-progress.json"),
        serde_json::to_vec(&recovery).unwrap(),
    )
    .unwrap();
    fs::write(directory.path().join("ghostty.conf"), "font-size = 23\n").unwrap();
    fs::create_dir(directory.path().join("themes")).unwrap();
    fs::write(directory.path().join("themes/partial"), "incomplete").unwrap();
    fs::write(directory.path().join(PENDING), "invalid backup").unwrap();
    let error = apply_pending(directory.path()).unwrap().unwrap();
    assert!(error.contains("Could not restore backup"));
    assert_eq!(
        fs::read_to_string(directory.path().join("ghostty.conf")).unwrap(),
        "font-size = 17\n"
    );
    assert!(!directory.path().join("themes").exists());
    assert!(!directory.path().join("restore-in-progress.json").exists());
    assert!(recovery.join("ghostty.conf").exists());
    cancel_pending(directory.path()).unwrap();
    assert!(apply_pending(directory.path()).unwrap().is_none());
}

#[test]
fn failed_rollback_still_prevents_loading_a_partially_restored_profile() {
    let directory = profile();
    let recovery = directory.path().join("Backups/pre-import-test");
    fs::create_dir_all(&recovery).unwrap();
    fs::write(recovery.join("roots.json"), "invalid journal").unwrap();
    let marker = directory.path().join("restore-in-progress.json");
    fs::write(&marker, serde_json::to_vec(&recovery).unwrap()).unwrap();
    let before = fs::read(directory.path().join("ghostty.conf")).unwrap();
    assert!(apply_pending(directory.path()).is_err());
    assert!(marker.exists());
    assert_eq!(
        fs::read(directory.path().join("ghostty.conf")).unwrap(),
        before
    );
}

#[test]
fn partial_terminal_import_preserves_unmentioned_preferences_and_replaces_lists() {
    let directory = profile();
    let legacy = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("ghostty.conf"), "font-family = Old Font\nfont-family = Old Fallback\nfont-size = 15\nkeybind = ctrl+alt+b=copy_to_clipboard\npalette = 0=#111111\n").unwrap();
    fs::write(legacy.path().join("ghostty.conf"), "font-size = 22\n").unwrap();
    let import = prepare(directory.path(), legacy.path()).unwrap();
    let source = String::from_utf8_lossy(&import.files["ghostty.conf"]);
    assert!(source.contains("font-family = Old Font"));
    assert!(source.contains("keybind = ctrl+alt+b=copy_to_clipboard"));
    assert!(source.contains("font-size = 22"));
    assert!(!source.contains("font-size = 15"));
    fs::write(
        legacy.path().join("ghostty.conf"),
        "font-family = New Font\nfont-family = New Fallback\npalette = 1=#222222\n",
    )
    .unwrap();
    let import = prepare(directory.path(), legacy.path()).unwrap();
    let source = String::from_utf8_lossy(&import.files["ghostty.conf"]);
    assert!(!source.contains("Old Font"));
    assert!(!source.contains("Old Fallback"));
    assert!(source.contains("New Font\nfont-family = New Fallback"));
    assert!(source.contains("palette = 0=#111111"));
    assert!(source.contains("palette = 1=#222222"));
    fs::write(
        legacy.path().join("ghostty.conf"),
        "unsupported-option = true\n",
    )
    .unwrap();
    fs::write(
        legacy.path().join("settings.json"),
        r#"{"muxy.quickTerminal.width":900}"#,
    )
    .unwrap();
    let import = prepare(directory.path(), legacy.path()).unwrap();
    assert!(!import.files.contains_key("ghostty.conf"));
}

#[test]
fn full_restore_resets_omitted_configuration_and_partial_import_keeps_it() {
    let source = profile();
    let target = profile();
    fs::write(
        target.path().join("server.toml"),
        "shell_integration = false\n",
    )
    .unwrap();
    fs::write(
        target.path().join("extension-enabled.json"),
        r#"["custom"]"#,
    )
    .unwrap();
    fs::create_dir(target.path().join("themes")).unwrap();
    fs::write(target.path().join("themes/custom"), "background = 111111").unwrap();
    let json = source.path().join("settings.json");
    fs::write(&json, r#"{"muxy.quickTerminal.width":900}"#).unwrap();
    stage(target.path(), &prepare(target.path(), &json).unwrap()).unwrap();
    apply_pending(target.path()).unwrap();
    assert!(target.path().join("server.toml").exists());
    assert!(target.path().join("themes/custom").exists());
    let archive = source.path().join("test.muxy");
    export(source.path(), &archive, &AppState::bootstrap().unwrap()).unwrap();
    stage(target.path(), &prepare(target.path(), &archive).unwrap()).unwrap();
    apply_pending(target.path()).unwrap();
    for name in ["server.toml", "extension-enabled.json", "themes"] {
        assert!(!target.path().join(name).exists(), "{name}");
    }
    assert!(
        fs::read_dir(target.path().join("Backups"))
            .unwrap()
            .any(|entry| entry.unwrap().path().join("themes/custom").exists())
    );
}

#[test]
fn mobile_preferences_exclude_credentials_and_apply_through_the_server() {
    let source = profile();
    let target = profile();
    let raw = br#"{"enabled":true,"port":7420,"certificate":"secret-certificate","private_key":"secret-key","devices":["secret-device"]}"#;
    fs::write(source.path().join("remote.json"), raw).unwrap();
    fs::write(target.path().join("remote.json"), raw).unwrap();
    let archive = source.path().join("test.muxy");
    export(source.path(), &archive, &AppState::bootstrap().unwrap()).unwrap();
    let import = prepare(target.path(), &archive).unwrap();
    assert!(!import.files.contains_key("remote.json"));
    let mobile = &import.files[mobile::FILE];
    assert!(!String::from_utf8_lossy(mobile).contains("secret"));
    stage(target.path(), &import).unwrap();
    apply_pending(target.path()).unwrap();
    assert!(apply_mobile_settings(target.path(), |_| Err("Server unavailable".into())).is_err());
    assert!(target.path().join(mobile::FILE).exists());
    let mut applied = None;
    apply_mobile_settings(target.path(), |settings| {
        applied = Some(settings);
        Ok(())
    })
    .unwrap();
    assert_eq!(
        applied,
        Some(muxy_protocol::RemoteAccessSettings {
            enabled: true,
            port: 7420
        })
    );
    assert!(!target.path().join(mobile::FILE).exists());
    assert_eq!(fs::read(target.path().join("remote.json")).unwrap(), raw);
}
