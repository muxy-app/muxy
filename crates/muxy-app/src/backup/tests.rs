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
    let (files, legacy, restore) = archive::read(&archive).unwrap();
    assert!(!legacy);
    assert_eq!(restore, Restore::Complete);
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
        TerminalSettings::load_native(&target.path().join("terminal.toml"))
            .unwrap()
            .font_size
            .to_bits(),
        17.0_f32.to_bits()
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
    archive::write(&archive, &files, Restore::Complete).unwrap();
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
    assert!(import.summary.contains("Not available in Muxy 2 (1)"));
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
    let source = String::from_utf8_lossy(&files["terminal.toml"]);
    assert!(!source.contains("config-file"));
    assert_eq!(
        TerminalSettings::from_native_source(&source)
            .unwrap()
            .font_size
            .to_bits(),
        21.0_f32.to_bits()
    );
    files.remove("terminal.toml");
    files.insert("ghostty.conf".into(), b"config-file = /etc/passwd".to_vec());
    archive::write(&archive, &files, Restore::Complete).unwrap();
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
    let terminal = TerminalSettings::load_native(&target.path().join("terminal.toml")).unwrap();
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
    fs::write(recovery.join("extensions.json"), r#"["added"]"#).unwrap();
    for name in ["added", "kept"] {
        fs::create_dir_all(directory.path().join("extensions").join(name)).unwrap();
    }
    fs::write(directory.path().join(PENDING), "invalid backup").unwrap();
    let error = apply_pending(directory.path()).unwrap().unwrap();
    assert!(error.contains("Could not restore backup"));
    assert!(!directory.path().join("extensions/added").exists());
    assert!(directory.path().join("pending-extensions/added").exists());
    assert!(directory.path().join("extensions/kept").exists());
    assert_eq!(
        fs::read_to_string(directory.path().join("ghostty.conf")).unwrap(),
        "font-size = 17\n"
    );
    assert!(!directory.path().join("themes").exists());
    assert!(!directory.path().join("restore-in-progress.json").exists());
    assert!(recovery.join("ghostty.conf").exists());
    cancel_pending(directory.path()).unwrap();
    assert!(!directory.path().join("pending-extensions").exists());
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
fn native_backup_ignores_stale_legacy_includes_and_old_backups_replace_native_settings() {
    let source = profile();
    let target = profile();
    let native = source.path().join("terminal.toml");
    let mut terminal = TerminalSettings::load_native(&native).unwrap();
    terminal
        .set_preference("adjust-cursor-thickness", "2")
        .unwrap();
    terminal.set_preference("background-opacity", "65").unwrap();
    terminal.save_native(&native).unwrap();
    fs::write(
        source.path().join("ghostty.conf"),
        "config-file = missing.conf",
    )
    .unwrap();
    let archive = source.path().join("native.muxy");
    export(source.path(), &archive, &AppState::bootstrap().unwrap()).unwrap();
    let (files, _, _) = archive::read(&archive).unwrap();
    assert!(!files.contains_key("ghostty.conf"));
    stage(target.path(), &prepare(target.path(), &archive).unwrap()).unwrap();
    assert!(apply_pending(target.path()).unwrap().is_none());
    assert_eq!(
        TerminalSettings::load_native(&target.path().join("terminal.toml")).unwrap(),
        terminal
    );

    let old = source.path().join("old.muxy");
    let files = Files::from([
        (
            "settings.toml".into(),
            fs::read(source.path().join("settings.toml")).unwrap(),
        ),
        (
            "ghostty.conf".into(),
            b"font-size = 24\nkeybind = ctrl+alt+x=text:hello\n".to_vec(),
        ),
    ]);
    archive::write(&old, &files, Restore::Partial).unwrap();
    stage(target.path(), &prepare(target.path(), &old).unwrap()).unwrap();
    assert!(apply_pending(target.path()).unwrap().is_none());
    let restored = TerminalSettings::load_native(&target.path().join("terminal.toml")).unwrap();
    assert_eq!(restored.font_size.to_bits(), 24.0_f32.to_bits());
    assert_eq!(
        restored.keybindings.action(&"ctrl-alt-x".parse().unwrap()),
        Some(&muxy_app_core::settings::TerminalAction::Text(
            b"hello".to_vec()
        ))
    );
}

#[test]
fn recovery_of_first_partial_terminal_migration_restores_legacy_preferences() {
    let directory = profile();
    let legacy = tempfile::tempdir().unwrap();
    fs::write(legacy.path().join("ghostty.conf"), "font-size = 26\n").unwrap();
    let import = prepare(directory.path(), legacy.path()).unwrap();
    stage(directory.path(), &import).unwrap();
    assert!(apply_pending(directory.path()).unwrap().is_none());
    let recovery = fs::read_dir(directory.path().join("Backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
        .join("recovery.muxy");
    let native = directory.path().join("terminal.toml");
    assert_eq!(
        TerminalSettings::load_native(&native)
            .unwrap()
            .font_size
            .to_bits(),
        26.0_f32.to_bits()
    );
    stage(
        directory.path(),
        &prepare(directory.path(), &recovery).unwrap(),
    )
    .unwrap();
    assert!(apply_pending(directory.path()).unwrap().is_none());
    assert_eq!(
        TerminalSettings::load_native(&native)
            .unwrap()
            .font_size
            .to_bits(),
        17.0_f32.to_bits()
    );
}

#[test]
fn partial_terminal_import_preserves_unmentioned_preferences_and_replaces_lists() {
    let directory = profile();
    let legacy = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("ghostty.conf"), "font-family = Old Font\nfont-family = Old Fallback\nfont-size = 15\nkeybind = ctrl+alt+b=copy_to_clipboard\npalette = 0=#111111\n").unwrap();
    fs::write(legacy.path().join("ghostty.conf"), "font-size = 22\n").unwrap();
    let import = prepare(directory.path(), legacy.path()).unwrap();
    let terminal = TerminalSettings::from_native_source(&String::from_utf8_lossy(
        &import.files["terminal.toml"],
    ))
    .unwrap();
    assert_eq!(terminal.font_families, ["Old Font", "Old Fallback"]);
    assert_eq!(
        terminal.keybindings.action(&"ctrl-alt-b".parse().unwrap()),
        Some(&muxy_app_core::settings::TerminalAction::Copy)
    );
    assert_eq!(terminal.font_size.to_bits(), 22.0_f32.to_bits());
    TerminalSettings::load_native(&directory.path().join("terminal.toml")).unwrap();
    fs::write(
        legacy.path().join("ghostty.conf"),
        "font-family = New Font\nfont-family = New Fallback\npalette = 1=#222222\n",
    )
    .unwrap();
    let import = prepare(directory.path(), legacy.path()).unwrap();
    let terminal = TerminalSettings::from_native_source(&String::from_utf8_lossy(
        &import.files["terminal.toml"],
    ))
    .unwrap();
    assert_eq!(terminal.font_families, ["New Font", "New Fallback"]);
    assert_eq!(terminal.options.palette.get(&0), Some(&0x11_1111));
    assert_eq!(terminal.options.palette.get(&1), Some(&0x22_2222));
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
    assert!(!import.files.contains_key("terminal.toml"));
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

fn legacy_package(folder: &Path, name: &str) {
    fs::create_dir_all(folder).unwrap();
    fs::write(
        folder.join("package.json"),
        serde_json::json!({"name":name,"version":"1.0.0","muxy":{}}).to_string(),
    )
    .unwrap();
}

#[test]
fn installed_one_x_is_imported_once_into_a_new_profile_and_later_imports_merge() {
    let directory = profile();
    let legacy = tempfile::tempdir().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let first_id = "01000000-0000-0000-0000-000000000001";
    let second_id = "01000000-0000-0000-0000-000000000002";
    let extensions = tempfile::tempdir().unwrap();
    legacy_package(&extensions.path().join("reader"), "reader");
    legacy_package(&extensions.path().join("renamed"), "other");
    let reader = extensions.path().join("reader");
    std::os::unix::fs::symlink("package.json", reader.join("inside")).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", reader.join("outside")).unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    legacy_package(elsewhere.path(), "locked");
    fs::create_dir(extensions.path().join("locked")).unwrap();
    std::os::unix::fs::symlink(
        elsewhere.path().join("package.json"),
        extensions.path().join("locked/package.json"),
    )
    .unwrap();
    let enabled = |name: &str| name == "reader" || name == "locked";
    let legacy_extensions = Some((extensions.path(), &enabled as &dyn Fn(&str) -> bool));
    let projects = |projects: serde_json::Value| {
        fs::write(
            legacy.path().join("projects.json"),
            serde_json::to_vec(&projects).unwrap(),
        )
        .unwrap();
    };
    projects(serde_json::json!([
        {"id":first_id,"name":"First","path":first.path(),"sortOrder":0},
        {"id":second_id,"name":"Unmounted","path":"/missing-muxy-project","sortOrder":1}
    ]));
    let notice = migrate_once(directory.path(), legacy.path(), legacy_extensions)
        .unwrap()
        .unwrap();
    for item in ["Unmounted", "renamed", "locked", "reader (1 links"] {
        assert!(notice.contains(item), "{item}");
    }
    assert!(apply_pending(directory.path()).unwrap().is_none());
    let installed = directory.path().join("extensions/reader");
    assert!(installed.join("inside").exists());
    assert!(fs::symlink_metadata(installed.join("outside")).is_err());
    assert!(!directory.path().join("extensions/locked").exists());
    assert!(!directory.path().join("pending-extensions").exists());
    let enabled: Vec<String> =
        serde_json::from_slice(&fs::read(directory.path().join("extension-enabled.json")).unwrap())
            .unwrap();
    assert_eq!(enabled, ["reader"]);
    fs::write(installed.join("settings-made-in-2.json"), "{}").unwrap();
    let path = directory.path().join("desktop-state.json");
    let mut state = muxy_app_core::store::load(&path).unwrap();
    let first_id = first_id.parse().unwrap();
    state.rename_project(first_id, "Renamed").unwrap();
    state.open_terminal_tab(first_id).unwrap();
    muxy_app_core::store::save(&path, &state).unwrap();
    assert!(
        migrate_once(directory.path(), legacy.path(), legacy_extensions)
            .unwrap()
            .is_none()
    );
    assert!(!directory.path().join(PENDING).exists());
    projects(serde_json::json!([
        {"id":first_id,"name":"First","path":first.path(),"sortOrder":0},
        {"id":second_id,"name":"Second","path":second.path(),"sortOrder":1}
    ]));
    legacy_package(&extensions.path().join("other"), "other");
    let backups = directory.path().join("Backups");
    fs::rename(&backups, directory.path().join("Backups.saved")).unwrap();
    fs::write(&backups, "").unwrap();
    let import = prepare_from(directory.path(), legacy.path(), legacy_extensions).unwrap();
    stage(directory.path(), &import).unwrap();
    assert!(apply_pending(directory.path()).unwrap().is_some());
    assert!(!directory.path().join("extensions/other").exists());
    cancel_pending(directory.path()).unwrap();
    assert!(!directory.path().join("pending-extensions").exists());
    fs::remove_file(&backups).unwrap();
    fs::rename(directory.path().join("Backups.saved"), &backups).unwrap();
    let import = prepare_from(directory.path(), legacy.path(), legacy_extensions).unwrap();
    stage(directory.path(), &import).unwrap();
    assert!(apply_pending(directory.path()).unwrap().is_none());
    assert!(installed.join("settings-made-in-2.json").exists());
    assert!(directory.path().join("extensions/other").exists());
    let merged = muxy_app_core::store::load(&path).unwrap();
    assert_eq!(merged.project(first_id).unwrap().name, "Renamed");
    assert_eq!(
        merged.project(first_id).unwrap().tabs,
        state.project(first_id).unwrap().tabs
    );
    assert!(merged.project(second_id.parse().unwrap()).is_some());
}

fn legacy_projects(legacy: &Path, folders: &[&Path]) -> Vec<muxy_app_core::ProjectId> {
    let ids: Vec<_> = (1..=folders.len())
        .map(|index| format!("01000000-0000-0000-0000-00000000000{index}"))
        .collect();
    let projects: Vec<_> = ids
        .iter()
        .zip(folders)
        .enumerate()
        .map(|(index, (id, folder))| {
            serde_json::json!({"id":id,"name":format!("Project {index}"),"path":folder,"sortOrder":index})
        })
        .collect();
    fs::write(
        legacy.join("projects.json"),
        serde_json::to_vec(&projects).unwrap(),
    )
    .unwrap();
    fs::write(
        legacy.join("project-groups.json"),
        serde_json::json!([{"id":"02000000-0000-0000-0000-000000000001","name":"Work","sortOrder":0,"projectIDs":ids,"type":"local"}])
            .to_string(),
    )
    .unwrap();
    ids.iter().map(|id| id.parse().unwrap()).collect()
}

#[test]
fn profile_an_earlier_two_x_left_empty_imports_installed_one_x_once() {
    let legacy = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let id = legacy_projects(legacy.path(), &[folder.path()])[0];
    let opened = tempfile::tempdir().unwrap();
    let path = opened.path().join("desktop-state.json");
    let empty = AppState::bootstrap().unwrap();
    muxy_app_core::store::save(&path, &empty).unwrap();
    assert!(
        migrate_once(opened.path(), legacy.path(), None)
            .unwrap()
            .is_none()
    );
    assert!(apply_pending(opened.path()).unwrap().is_none());
    let imported = muxy_app_core::store::load(&path).unwrap();
    assert!(imported.project(id).is_some());
    assert_eq!(imported.workspaces()[0].projects, [id].into());
    muxy_app_core::store::save(&path, &empty).unwrap();
    migrate_once(opened.path(), legacy.path(), None).unwrap();
    assert!(!opened.path().join(PENDING).exists());
}

#[test]
fn automatic_one_x_import_leaves_used_imported_and_staged_profiles_alone() {
    let legacy = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    legacy_projects(legacy.path(), &[folder.path()]);
    let empty = AppState::bootstrap().unwrap();
    let mut with_project = empty.clone();
    with_project
        .add_project(ServerId::local(), folder.path().into())
        .unwrap();
    let mut with_workspace = empty.clone();
    with_workspace.create_workspace("Mine").unwrap();
    for used in [with_project, with_workspace] {
        let profile = tempfile::tempdir().unwrap();
        for state in [&used, &empty] {
            muxy_app_core::store::save(profile.path().join("desktop-state.json"), state).unwrap();
            migrate_once(profile.path(), legacy.path(), None).unwrap();
            assert!(!profile.path().join(PENDING).exists());
        }
    }

    let imported_by_two_one = tempfile::tempdir().unwrap();
    fs::create_dir_all(
        imported_by_two_one
            .path()
            .join("Backups/pre-import-earlier"),
    )
    .unwrap();
    migrate_once(imported_by_two_one.path(), legacy.path(), None).unwrap();
    assert!(!imported_by_two_one.path().join(PENDING).exists());

    let staged = tempfile::tempdir().unwrap();
    fs::write(staged.path().join(PENDING), "staged by the user").unwrap();
    migrate_once(staged.path(), legacy.path(), None).unwrap();
    assert_eq!(
        fs::read_to_string(staged.path().join(PENDING)).unwrap(),
        "staged by the user"
    );
}
