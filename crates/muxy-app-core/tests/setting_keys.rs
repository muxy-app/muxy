use muxy_app_core::settings::keys::{Profile, display, parse_key, render_key};
use muxy_app_core::settings::{NewPaneDirectory, Settings, TerminalSettings};

fn profile() -> std::io::Result<(tempfile::TempDir, Profile)> {
    let directory = tempfile::tempdir()?;
    let profile = Profile::new(directory.path());
    Ok((directory, profile))
}

#[test]
fn keys_split_at_dots_outside_quotes_and_render_back() {
    let segments = parse_key(r#"keymap."command.build""#).expect("key");
    assert_eq!(segments, ["keymap", "command.build"]);
    assert_eq!(render_key(&segments), r#"keymap."command.build""#);
    for invalid in ["", "appearance.", ".theme", "a..b", r#"keymap."open"#] {
        assert!(parse_key(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn every_setting_is_listed_with_its_default_including_the_terminals() {
    let (_directory, profile) = profile().expect("profile");
    let entries = profile.list().expect("list");
    let value = |key: &str| {
        entries
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
    };
    assert_eq!(
        value("panes.new_pane_directory"),
        Some(toml::Value::String("project".into()))
    );
    assert_eq!(
        value("terminal.font_size"),
        Some(toml::Value::Float(f64::from(
            TerminalSettings::default().font_size
        )))
    );
    assert!(value("appearance.dark_theme").is_some());
    assert!(entries.iter().all(|(key, _)| parse_key(key).is_ok()));
}

#[test]
fn set_reads_toml_or_text_and_saves_only_what_the_app_can_start_with() {
    let (directory, profile) = profile().expect("profile");
    profile
        .set("panes.new_pane_directory", "current")
        .expect("set");
    let saved = Settings::load(&directory.path().join("settings.toml")).expect("load");
    assert_eq!(saved.panes.new_pane_directory, NewPaneDirectory::Current);

    profile
        .set("appearance.dark_theme", "Builtin Dark")
        .expect("text without quotes");
    assert_eq!(
        display(&profile.get("appearance.dark_theme").expect("get")),
        "Builtin Dark"
    );
    profile
        .set("terminal.font_size", "16")
        .expect("whole number");
    let terminal =
        TerminalSettings::load_native(&directory.path().join("terminal.toml")).expect("load");
    assert!((terminal.font_size - 16.0).abs() < f32::EPSILON);

    let before = std::fs::read_to_string(directory.path().join("settings.toml")).expect("file");
    for (key, value) in [
        ("panes.new_pane_directory", "sideways"),
        ("appearance.not_a_setting", "1"),
        ("not_a_section.key", "1"),
        ("window.default_size", "[10, 10]"),
        ("servers", "[]"),
    ] {
        assert!(profile.set(key, value).is_err(), "{key} = {value}");
    }
    let after = std::fs::read_to_string(directory.path().join("settings.toml")).expect("file");
    assert_eq!(before, after);
    assert!(profile.set("terminal.font_size", "1000").is_err());
    assert!(profile.set("terminal.no_such_option", "1").is_err());
}

#[test]
fn unset_returns_a_setting_to_its_default() {
    let (directory, profile) = profile().expect("profile");
    profile
        .set("panes.new_pane_directory", "current")
        .expect("set");
    profile.set("terminal.font_size", "20").expect("set");
    profile.unset("panes.new_pane_directory").expect("unset");
    profile.unset("terminal.font_size").expect("unset");
    let saved = Settings::load(&directory.path().join("settings.toml")).expect("load");
    assert_eq!(saved.panes.new_pane_directory, NewPaneDirectory::Project);
    let terminal =
        TerminalSettings::load_native(&directory.path().join("terminal.toml")).expect("load");
    assert!((terminal.font_size - TerminalSettings::default().font_size).abs() < f32::EPSILON);
    let before = std::fs::read_to_string(directory.path().join("settings.toml")).expect("file");
    assert!(profile.unset("not_a_section.key").is_err());
    assert!(profile.unset("terminal.not_an_option").is_err());
    let after = std::fs::read_to_string(directory.path().join("settings.toml")).expect("file");
    assert_eq!(before, after, "a refused unset leaves the file alone");
}

#[test]
fn optional_and_empty_settings_can_be_set_and_cleared() {
    let (_directory, profile) = profile().expect("profile");
    assert!(profile.get("projects.search_root").is_err());
    profile
        .set("projects.search_root", "/tmp")
        .expect("an optional setting");
    assert_eq!(
        display(&profile.get("projects.search_root").expect("get")),
        "/tmp"
    );
    profile
        .set("projects.search_skip", r#"["node_modules"]"#)
        .expect("a list");
    profile
        .set("projects.search_skip", "[]")
        .expect("an empty list");
    profile.unset("projects.search_root").expect("unset");
    assert!(profile.get("projects.search_root").is_err());
}

#[test]
fn tables_read_whole_and_print_as_toml() {
    let (_directory, profile) = profile().expect("profile");
    let panes = profile.get("panes").expect("table");
    assert_eq!(display(&panes), r#"new_pane_directory = "project""#);
    assert!(profile.get("panes.missing").is_err());
}
