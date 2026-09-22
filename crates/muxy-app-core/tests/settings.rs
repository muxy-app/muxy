#![allow(
    clippy::float_cmp,
    reason = "Configuration values and integer zoom steps must round-trip exactly"
)]

use muxy_core::shortcuts::ShortcutId;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use muxy_app_core::settings::{CellHeight, KeyChord, Keymap, Settings, TerminalSettings};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[test]
fn sidebar_vibrancy_defaults_and_changes_preserve_other_preferences() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write("settings.toml", "[appearance]\ndark_theme = 'Dracula'\n")?;
    let original = Settings::load(&path)?.appearance;
    assert!(original.sidebar_vibrancy);
    assert_eq!(original.sidebar_vibrancy_level, 50);
    let mut changed = original.clone();
    changed.sidebar_vibrancy = false;
    changed.sidebar_vibrancy_level = 35;
    changed.save_changes(&original, &path)?;
    let mut collapsed = original.clone();
    collapsed.sidebar_expanded = true;
    let saved = collapsed.save_changes(&original, &path)?;
    assert!(!saved.sidebar_vibrancy);
    assert_eq!(saved.sidebar_vibrancy_level, 35);
    assert!(saved.sidebar_expanded);
    assert_eq!(saved.dark_theme, "Dracula");
    assert_eq!(Settings::load(&path)?.appearance, saved);
    Ok(())
}

#[test]
fn panel_pins_survive_reload_and_override_defaults_independently() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write(
        "settings.toml",
        "[composer]\npinned = true\n[keymap]\nnew_tab = 'cmd-n'\n",
    )?;
    let mut settings = Settings::load(&path)?;
    assert!(settings.panel_pins.is_empty());
    assert!(settings.panel_pinned("git.tools", "changes", true));
    assert!(!settings.panel_pinned("git.tools", "changes", false));

    settings.set_panel_pinned("git.tools", "changes", true, &path)?;
    settings = Settings::load(&path)?;
    assert!(settings.panel_pinned("git.tools", "changes", false));
    assert!(!settings.panel_pinned("files", "changes", false));
    assert!(!settings.panel_pinned("git.tools", "history", false));

    settings.set_panel_pinned("files", "changes", true, &path)?;
    settings.set_panel_pinned("git.tools", "history", true, &path)?;
    settings.set_panel_pinned("git.tools", "changes", false, &path)?;
    settings = Settings::load(&path)?;
    assert!(!settings.panel_pinned("git.tools", "changes", true));
    assert!(settings.panel_pinned("files", "changes", false));
    assert!(settings.panel_pinned("git.tools", "history", false));
    assert!(settings.composer.pinned);
    assert_eq!(
        settings
            .keymap
            .chord(ShortcutId::NewTab)
            .map(KeyChord::as_str),
        Some("cmd-n")
    );
    settings.save_composer(&path)?;
    assert_eq!(Settings::load(&path)?, settings);
    Ok(())
}

#[test]
fn failed_panel_pin_save_preserves_the_previous_preference() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write("settings.toml", "")?;
    let mut settings = Settings::load(&path)?;
    settings.set_panel_pinned("git", "changes", true, &path)?;
    let previous = settings.clone();
    fs::write(&path, "invalid toml")?;
    assert!(
        settings
            .set_panel_pinned("git", "changes", false, &path)
            .is_err()
    );
    assert_eq!(settings, previous);
    assert_eq!(fs::read_to_string(&path)?, "invalid toml");
    Ok(())
}

#[test]
fn close_behavior_defaults_to_close_and_persists_without_changing_other_preferences() -> Result {
    use muxy_app_core::settings::CloseBehavior;
    let fixture = Fixture::new()?;
    let path = fixture.write(
        "settings.toml",
        "[window]\nconfirm_running_process = false\n[keymap]\nnew_tab = 'cmd-n'\n",
    )?;
    let mut settings = Settings::load(&path)?;
    assert_eq!(settings.window.close_behavior, CloseBehavior::CloseSession);
    settings.window.close_behavior = CloseBehavior::Detach;
    settings.save_window(&path)?;
    assert_eq!(Settings::load(&path)?, settings);
    settings.set_confirm_running_process(true, &path)?;
    assert_eq!(Settings::load(&path)?, settings);
    assert!(toml::from_str::<Settings>("[window]\nclose_behavior = 'invalid'").is_err());
    Ok(())
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "muxy-settings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn write(&self, name: &str, source: &str) -> Result<PathBuf> {
        let path = self.0.join(name);
        fs::write(&path, source)?;
        Ok(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn missing_file_writes_editable_defaults_and_empty_file_loads_defaults() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.0.join("nested/settings.toml");
    assert_eq!(Settings::load(&path)?, Settings::default());
    let source = fs::read_to_string(&path)?;
    assert!(source.contains("new_tab = \"cmd-t\""));
    let stored: toml::Value = toml::from_str(&source)?;
    assert!(stored.get("terminal").is_none());
    assert_eq!(
        stored["keymap"]["terminal.close_find"].as_str(),
        Some("escape")
    );
    assert_eq!(Settings::load(&path)?, Settings::default());
    fs::write(&path, "")?;
    assert_eq!(Settings::load(&path)?, Settings::default());
    assert_eq!(fs::read_to_string(path)?, "");
    Ok(())
}

#[test]
fn partial_settings_keep_defaults_and_appearance_saves_preserve_bindings() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write("settings.toml", "[appearance]\ndark_theme = 'Dracula'\n[window]\ndefault_size = [1000, 700]\n[keymap]\nnew_tab = 'cmd-n'\n")?;
    let settings = Settings::load(&path)?;
    assert_eq!(settings.window.default_size, [1000.0, 700.0]);
    assert_eq!(
        settings.appearance,
        muxy_app_core::settings::Appearance {
            dark_theme: "Dracula".into(),
            ..Default::default()
        }
    );
    assert_eq!(
        settings
            .keymap
            .chord(ShortcutId::NewTab)
            .map(KeyChord::as_str),
        Some("cmd-n")
    );
    assert_eq!(
        settings
            .keymap
            .chord(ShortcutId::CloseTab)
            .map(KeyChord::as_str),
        Some("cmd-shift-w")
    );
    let mut appearance = settings.appearance;
    appearance.sidebar_expanded = true;
    appearance.save(&path)?;
    let reloaded = Settings::load(&path)?;
    assert_eq!(reloaded.appearance, appearance);
    assert_eq!(reloaded.keymap, settings.keymap);
    assert_eq!(reloaded.window, settings.window);
    Ok(())
}

#[test]
fn close_confirmation_defaults_to_enabled_and_round_trips_without_replacing_other_settings()
-> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write("settings.toml", "[window]\ndefault_size = [1000, 700]\n[keymap]\nnew_tab = 'cmd-n'\n[appearance]\ndark_theme = 'Dracula'\n")?;
    let mut settings = Settings::load(&path)?;
    assert!(settings.window.confirm_running_process);
    settings.set_confirm_running_process(false, &path)?;
    assert!(!settings.window.confirm_running_process);
    let mut reloaded = Settings::load(&path)?;
    assert_eq!(settings, reloaded);
    reloaded.appearance.sidebar_expanded = true;
    reloaded.appearance.save(&path)?;
    assert_eq!(Settings::load(&path)?, reloaded);
    reloaded.set_confirm_running_process(true, &path)?;
    assert!(Settings::load(&path)?.window.confirm_running_process);
    assert_eq!(reloaded.window.default_size, [1000.0, 700.0]);
    assert_eq!(
        reloaded
            .keymap
            .chord(ShortcutId::NewTab)
            .map(KeyChord::as_str),
        Some("cmd-n")
    );
    assert_eq!(reloaded.appearance.dark_theme, "Dracula");
    Ok(())
}

#[test]
fn saving_close_confirmation_preserves_invalid_files_and_in_memory_preferences() -> Result {
    let fixture = Fixture::new()?;
    for source in ["not valid TOML", "window = 'invalid'"] {
        let path = fixture.write("settings.toml", source)?;
        let mut settings = Settings::default();
        assert!(settings.set_confirm_running_process(false, &path).is_err());
        assert!(settings.window.confirm_running_process);
        assert_eq!(fs::read_to_string(&path)?, source);
    }
    let blocked = fixture.write("blocked", "keep this file")?;
    let mut settings = Settings::default();
    assert!(
        settings
            .set_confirm_running_process(false, &blocked.join("settings.toml"))
            .is_err()
    );
    assert!(settings.window.confirm_running_process);
    assert_eq!(fs::read_to_string(blocked)?, "keep this file");
    Ok(())
}

#[test]
fn every_default_binding_round_trips_and_resolves_both_directions() -> Result {
    let keymap = Keymap::default();
    for action in Keymap::ACTIONS {
        if matches!(
            action,
            ShortcutId::SelectCommandOutput | ShortcutId::DetachTerminal
        ) {
            assert!(keymap.chord(action).is_none());
            continue;
        }
        let chord: KeyChord = keymap
            .chord(action)
            .ok_or("default is unbound")?
            .to_string()
            .parse()?;
        assert_eq!(Some(&chord), keymap.chord(action));
        assert_eq!(keymap.action(&chord), Some(action));
    }
    assert_eq!(keymap.action(&"ctrl-alt-f24".parse()?), None);
    let settings: Settings = toml::from_str(&toml::to_string(&Settings::default())?)?;
    assert_eq!(settings, Settings::default());
    Ok(())
}

#[test]
fn chords_support_all_modifiers_named_keys_and_printable_characters() -> Result {
    for modifier in ["cmd", "ctrl", "alt", "shift", "fn"] {
        for name in [
            "enter",
            "escape",
            "space",
            "tab",
            "backspace",
            "delete",
            "insert",
            "home",
            "end",
            "pageup",
            "pagedown",
            "up",
            "down",
            "left",
            "right",
            "]",
            "[",
            "-",
            "+",
            "=",
            "a",
            "7",
            "é",
        ] {
            let value = format!("{modifier}-{name}");
            assert_eq!(value.parse::<KeyChord>()?.as_str(), value);
        }
        for number in 1..=24 {
            let value = format!("{modifier}-f{number}");
            assert_eq!(value.parse::<KeyChord>()?.as_str(), value);
        }
    }
    for (source, canonical) in [
        ("alt-CMD-ctrl-shift-fn-left", "cmd-ctrl-alt-shift-fn-left"),
        ("cmd-T", "cmd-shift-t"),
        ("cmd-plus", "cmd-+"),
        ("cmd-minus", "cmd--"),
        ("return", "enter"),
        ("esc", "escape"),
        ("CMD-ENTER", "cmd-enter"),
    ] {
        assert_eq!(source.parse::<KeyChord>()?.as_str(), canonical);
    }
    Ok(())
}

#[test]
fn invalid_chords_and_keymap_errors_name_the_problem() -> Result {
    for value in [
        "",
        "cmd",
        "cmd-",
        "cmd-cmd-t",
        "cmd-x-y",
        "ctrl--x",
        "cmd t",
        "cmd-unknown",
        "f0",
        "f25",
        "f01",
        "\n",
        "cmd- ",
        " cmd-t",
        "cmd---",
    ] {
        assert!(value.parse::<KeyChord>().is_err(), "{value:?}");
    }
    for (source, names) in [
        (
            "[keymap]\nnew_tab = 'cmd-nope'",
            vec!["new_tab", "cmd-nope"],
        ),
        (
            "[keymap]\nnew_tabb = 'cmd-t'",
            vec!["new_tabb", "unknown action"],
        ),
        (
            "[keymap]\nnew_tab = 'cmd-c'",
            vec!["new_tab", "copy", "cmd-c"],
        ),
        (
            "[keymap]\nnew_tab = 'shift-cmd-x'\nclose_tab = 'cmd-shift-x'",
            vec!["new_tab", "close_tab"],
        ),
    ] {
        let error = toml::from_str::<Settings>(source)
            .err()
            .ok_or("invalid settings accepted")?
            .to_string();
        for name in names {
            assert!(error.contains(name), "{error}");
        }
    }
    let settings: Settings = toml::from_str("[keymap]\nnew_tab = 'cmd-w'\nclose_tab = 'cmd-t'")?;
    assert_eq!(
        settings.keymap.action(&"cmd-w".parse()?),
        Some(ShortcutId::NewTab)
    );
    Ok(())
}

#[test]
fn invalid_settings_are_reported_without_overwriting_the_file() -> Result {
    let fixture = Fixture::new()?;
    for source in [
        "[broken",
        "[window]\ndefault_size = [0, 800]",
        "[window]\ndefault_size = [1200, inf]",
        "[window]\ndefault_size = [nan, 800]",
        "[terminal]\nfont_size = 16",
    ] {
        let path = fixture.write("settings.toml", source)?;
        assert!(Settings::load(&path).is_err());
        assert_eq!(fs::read_to_string(path)?, source);
    }
    Ok(())
}

#[test]
fn ghostty_defaults_seed_once_and_existing_config_is_preserved() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.0.join("ghostty.conf");
    assert_eq!(
        TerminalSettings::load_with_seed(&path, None)?,
        TerminalSettings::default()
    );
    let source = "font-family = Monaco\nfont-size = 16\n";
    let seed = fixture.write("seed", source)?;
    assert_eq!(
        TerminalSettings::load_with_seed(&path, Some(&seed))?,
        TerminalSettings::default()
    );
    fs::remove_file(&path)?;
    let settings = TerminalSettings::load_with_seed(&path, Some(&seed))?;
    assert_eq!(settings.font_size, 16.0);
    assert_eq!(settings.font_families, ["Monaco"]);
    assert_eq!(fs::read_to_string(path)?, source);
    Ok(())
}

#[test]
fn ghostty_values_support_comments_quotes_resets_fallbacks_and_height_adjustments() -> Result {
    let fixture = Fixture::new()?;
    let source = "# terminal settings\nfont-size = 16.5\nfont-family = Discarded\nfont-family = \"\"\nfont-family = \"Menlo\"\nfont-family = PingFang SC\nadjust-cell-height = 20%\nbackground = 112233\n";
    let path = fixture.write("ghostty.conf", source)?;
    let settings = TerminalSettings::load(&path)?;
    assert_eq!(settings.font_families, ["Menlo", "PingFang SC"]);
    assert_eq!(settings.font_size, 16.5);
    assert_eq!(settings.cell_height, CellHeight::Percent(20.0));
    assert_eq!(settings.cell_height.apply(20.0, 2.0), 24.0);
    assert_eq!(CellHeight::Pixels(4).apply(20.0, 2.0), 22.0);
    assert_eq!(CellHeight::Pixels(-100).apply(20.0, 2.0), 0.5);
    assert_eq!(fs::read_to_string(&path)?, source);
    fs::write(
        &path,
        "font-size = 20\nfont-size =\nadjust-cell-height = 10%\nadjust-cell-height =\nfont-family =\n",
    )?;
    assert_eq!(TerminalSettings::load(&path)?, TerminalSettings::default());
    Ok(())
}

#[test]
fn ghostty_includes_apply_last_and_detect_cycles() -> Result {
    let fixture = Fixture::new()?;
    fixture.write("fonts", "font-size = 17\n")?;
    let path = fixture.write(
        "ghostty.conf",
        "config-file = ?missing\nconfig-file = fonts\nfont-size = 15\n",
    )?;
    assert_eq!(TerminalSettings::load(&path)?.font_size, 17.0);
    fixture.write("fonts", "config-file = ghostty.conf\n")?;
    let error = TerminalSettings::load(&path)
        .err()
        .ok_or("cycle accepted")?
        .to_string();
    assert!(error.contains("cycle"), "{error}");
    Ok(())
}

#[test]
fn ghostty_accepts_bare_font_thickening_and_ignores_unrelated_booleans() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write(
        "ghostty.conf",
        "font-size = 16\nfont-thicken\nbold-is-bright\n",
    )?;
    let settings = TerminalSettings::load(&path)?;
    assert_eq!(settings.font_size, 16.0);
    assert!(settings.font.thicken);
    Ok(())
}

#[test]
fn ghostty_accepts_fractional_height_percentages() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write("ghostty.conf", "adjust-cell-height = 12.5%\n")?;
    assert_eq!(
        TerminalSettings::load(&path)?.cell_height.apply(16.0, 1.0),
        18.0
    );
    Ok(())
}

#[test]
fn ghostty_loads_quoted_optional_paths() -> Result {
    let fixture = Fixture::new()?;
    fixture.write("font settings", "font-size = 17\n")?;
    let path = fixture.write(
        "ghostty.conf",
        "config-file = ?\"missing\"\nconfig-file = ?\"font settings\"\n",
    )?;
    assert_eq!(TerminalSettings::load(&path)?.font_size, 17.0);
    Ok(())
}

#[test]
fn ghostty_loads_nested_includes_in_discovery_order() -> Result {
    let fixture = Fixture::new()?;
    fixture.write("a", "config-file = c\n")?;
    fixture.write("b", "font-size = 18\n")?;
    fixture.write("c", "font-size = 17\n")?;
    let path = fixture.write("ghostty.conf", "config-file = a\nconfig-file = b\n")?;
    assert_eq!(TerminalSettings::load(&path)?.font_size, 17.0);
    Ok(())
}

#[test]
fn ghostty_errors_name_the_key_and_line_and_zoom_is_only_in_memory() -> Result {
    let fixture = Fixture::new()?;
    for (key, value) in [
        ("font-size", "nan"),
        ("font-size", "0"),
        ("font-size", "huge"),
        ("font-family", "\"Menlo"),
        ("adjust-cell-height", "-100%"),
        ("adjust-cell-height", "tall"),
    ] {
        let path = fixture.write("ghostty.conf", &format!("# test\n{key} = {value}"))?;
        let error = TerminalSettings::load(&path)
            .err()
            .ok_or("bad Ghostty setting accepted")?
            .to_string();
        assert!(error.contains(key) && error.contains(":2"), "{error}");
    }
    let path = fixture.write("ghostty.conf", "font-size = 16\n")?;
    let mut active = TerminalSettings::load(&path)?;
    let other = active.clone();
    active.zoom(1.0);
    assert_eq!(active.font_size, 17.0);
    assert_eq!(other.font_size, 16.0);
    active.zoom(-1.0);
    assert_eq!(active.font_size, 16.0);
    active.zoom(-1000.0);
    assert_eq!(active.font_size, 1.0);
    active.zoom(1000.0);
    assert_eq!(active.font_size, 256.0);
    assert_eq!(TerminalSettings::load(&path)?.font_size, 16.0);
    Ok(())
}

#[test]
fn project_search_location_round_trips_and_open_project_uses_command_o() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write("settings.toml", "[keymap]\nnew_tab = 'cmd-n'\n")?;
    let mut settings = Settings::load(&path)?;
    assert_eq!(
        settings
            .keymap
            .chord(ShortcutId::AddProject)
            .map(KeyChord::as_str),
        Some("cmd-o")
    );
    assert!(settings.projects.search_root.is_none());
    settings.set_project_search_root(fixture.0.clone(), &path)?;
    assert_eq!(Settings::load(&path)?, settings);
    assert_eq!(
        settings
            .keymap
            .chord(ShortcutId::NewTab)
            .map(KeyChord::as_str),
        Some("cmd-n")
    );
    Ok(())
}

#[test]
fn existing_bindings_take_precedence_over_new_project_defaults_and_round_trip() -> Result {
    let fixture = Fixture::new()?;
    for (project_action, chord) in [
        (ShortcutId::AddProject, "cmd-o"),
        (ShortcutId::PreviousProject, "cmd-alt-["),
        (ShortcutId::NextProject, "cmd-alt-]"),
    ] {
        for existing_action in [ShortcutId::NewTab, ShortcutId::Copy] {
            let source = format!("[keymap]\n{} = '{chord}'\n", existing_action.name());
            let path = fixture.write("settings.toml", &source)?;
            let mut settings = Settings::load(&path)?;
            let chord: KeyChord = chord.parse()?;
            assert_eq!(settings.keymap.chord(existing_action), Some(&chord));
            assert_eq!(settings.keymap.action(&chord), Some(existing_action));
            assert_eq!(settings.keymap.chord(project_action), None);
            assert_eq!(fs::read_to_string(&path)?, source);

            settings.set_project_search_root(fixture.0.clone(), &path)?;
            assert_eq!(Settings::load(&path)?, settings);
            fs::write(&path, toml::to_string(&settings)?)?;
            assert_eq!(Settings::load(&path)?, settings);
        }
    }
    Ok(())
}

#[test]
fn explicit_project_bindings_still_require_unique_chords() -> Result {
    for (project_action, chord) in [
        (ShortcutId::AddProject, "cmd-o"),
        (ShortcutId::PreviousProject, "cmd-alt-["),
        (ShortcutId::NextProject, "cmd-alt-]"),
    ] {
        let source = format!(
            "[keymap]\nnew_tab = '{chord}'\n{} = '{chord}'\n",
            project_action.name()
        );
        let error = toml::from_str::<Settings>(&source)
            .expect_err("explicit collision")
            .to_string();
        assert!(error.contains("new_tab"), "{error}");
        assert!(error.contains(project_action.name()), "{error}");

        let source = format!(
            "[keymap]\nnew_tab = '{chord}'\n{} = 'ctrl-alt-f24'\n",
            project_action.name()
        );
        let settings: Settings = toml::from_str(&source)?;
        assert_eq!(
            settings.keymap.action(&chord.parse()?),
            Some(ShortcutId::NewTab)
        );
        assert_eq!(
            settings.keymap.action(&"ctrl-alt-f24".parse()?),
            Some(project_action)
        );
    }
    Ok(())
}

#[test]
fn pane_directory_and_all_split_shortcuts_load_with_defaults_and_overrides() -> Result {
    let settings: Settings = toml::from_str("")?;
    assert_eq!(
        settings.panes.new_pane_directory,
        muxy_app_core::settings::NewPaneDirectory::Project
    );
    for (action, chord) in [
        (ShortcutId::SplitRight, "cmd-d"),
        (ShortcutId::SplitDown, "cmd-shift-d"),
        (ShortcutId::FocusPaneLeft, "cmd-alt-left"),
        (ShortcutId::FocusPaneRight, "cmd-alt-right"),
        (ShortcutId::FocusPaneUp, "cmd-alt-up"),
        (ShortcutId::FocusPaneDown, "cmd-alt-down"),
        (ShortcutId::ToggleZoomPane, "cmd-shift-enter"),
        (ShortcutId::ClosePane, "cmd-w"),
        (ShortcutId::CloseTab, "cmd-shift-w"),
    ] {
        assert_eq!(
            settings.keymap.chord(action).map(KeyChord::as_str),
            Some(chord)
        );
    }
    let customized: Settings = toml::from_str(
        "[panes]\nnew_pane_directory = 'current'\n[keymap]\nsplit_right = 'ctrl-alt-d'",
    )?;
    assert_eq!(
        customized.panes.new_pane_directory,
        muxy_app_core::settings::NewPaneDirectory::Current
    );
    assert_eq!(
        customized.keymap.action(&"ctrl-alt-d".parse()?),
        Some(ShortcutId::SplitRight)
    );
    assert_eq!(customized.keymap.action(&"cmd-d".parse()?), None);
    assert!(toml::from_str::<Settings>("[panes]\nnew_pane_directory = 'invalid'").is_err());
    assert_eq!(
        toml::from_str::<Settings>(&toml::to_string(&customized)?)?,
        customized
    );
    Ok(())
}

#[test]
fn new_pane_defaults_preserve_explicit_existing_shortcuts_without_collisions() -> Result {
    for (action, chord, displaced) in [
        (ShortcutId::CloseTab, "cmd-w", ShortcutId::ClosePane),
        (ShortcutId::NewTab, "cmd-d", ShortcutId::SplitRight),
        (ShortcutId::NextTab, "cmd-shift-d", ShortcutId::SplitDown),
        (
            ShortcutId::PreviousTab,
            "cmd-alt-left",
            ShortcutId::FocusPaneLeft,
        ),
        (
            ShortcutId::Copy,
            "cmd-shift-enter",
            ShortcutId::ToggleZoomPane,
        ),
        (ShortcutId::Paste, "cmd-shift-w", ShortcutId::CloseTab),
    ] {
        let settings: Settings =
            toml::from_str(&format!("[keymap]\n{} = '{chord}'", action.name()))?;
        assert_eq!(settings.keymap.action(&chord.parse()?), Some(action));
        assert!(settings.keymap.chord(displaced).is_none());
    }
    assert!(
        toml::from_str::<Settings>("[keymap]\nclose_tab = 'cmd-w'\nclose_pane = 'cmd-w'").is_err()
    );
    Ok(())
}

#[test]
fn every_module_shortcut_is_configurable_with_contexts_and_aliases() -> Result {
    use muxy_core::shortcuts::{ALL, ShortcutSettings};
    let defaults = Keymap::default();
    let mut names = std::collections::BTreeSet::new();
    for shortcut in ALL {
        assert!(names.insert(shortcut.id), "duplicate shortcut ID");
        assert_eq!(shortcut.keys.len(), shortcut.key_contexts.len());
        for key in shortcut.keys {
            let _: KeyChord = key.parse()?;
        }
        let settings = toml::from_str::<Settings>(&format!(
            "[keymap]\n\"{}\" = \"ctrl-alt-f24\"",
            shortcut.id
        ))?;
        for context in shortcut.contexts {
            assert_eq!(
                settings.keymap.keys(shortcut.id, *context),
                ["ctrl-alt-f24"]
            );
            let expected: Vec<_> = shortcut
                .keys
                .iter()
                .zip(shortcut.key_contexts)
                .filter(|(_, scopes)| scopes.contains(context))
                .map(|(key, _)| key.parse::<KeyChord>().map(|chord| chord.to_string()))
                .collect::<std::result::Result<_, _>>()?;
            assert_eq!(defaults.keys(shortcut.id, *context), expected);
        }
    }
    Ok(())
}

#[test]
fn aliases_yield_to_explicit_bindings_and_conflicts_are_scoped() -> Result {
    use muxy_core::shortcuts::ShortcutSettings;
    let settings = toml::from_str::<Settings>(
        "[keymap]\nnew_tab = 'ctrl-tab'\n'popover.dismiss' = 'ctrl-k'\n'menu.dismiss_menu' = 'ctrl-k'",
    )?;
    assert_eq!(
        settings.keymap.keys("next_tab", Some("WorkspaceTabs")),
        ["cmd-]"]
    );
    assert_eq!(
        settings.keymap.keys("popover.dismiss", Some("Picker")),
        ["ctrl-k"]
    );
    assert!(
        toml::from_str::<Settings>(
            "[keymap]\n'popover.dismiss' = 'ctrl-k'\n'popover.confirm' = 'ctrl-k'"
        )
        .is_err()
    );
    assert!(
        toml::from_str::<Settings>("[keymap]\nquit = 'cmd-k'\n'popover.confirm' = 'cmd-k'")
            .is_err()
    );
    let remapped = toml::from_str::<Settings>("[keymap]\n'popover.secondary_confirm' = 'ctrl-k'")?;
    assert_eq!(
        remapped
            .keymap
            .keys("popover.secondary_confirm", Some("Picker")),
        ["ctrl-k"]
    );
    Ok(())
}

#[test]
fn alias_overrides_only_displace_matching_contexts() -> Result {
    use muxy_core::shortcuts::ShortcutSettings;
    let settings: Settings = toml::from_str(
        r#"[keymap]
"text_input.submit" = "cmd-up"
"text_input.cancel" = "cmd-down"
"#,
    )?;
    assert!(
        settings
            .keymap
            .keys("text_input.document_start", Some("MultilineInput"))
            .contains(&"cmd-up".into())
    );
    assert!(
        settings
            .keymap
            .keys("text_input.document_end", Some("MultilineInput"))
            .contains(&"cmd-down".into())
    );
    assert_eq!(
        settings.keymap.keys("text_input.submit", Some("TextInput")),
        ["cmd-up"]
    );
    Ok(())
}

#[test]
fn clipboard_and_opener_preferences_load_without_losing_unavailable_ids() -> Result {
    let defaults: Settings = toml::from_str("")?;
    assert!(!defaults.clipboard.copy_on_select);
    assert_eq!(defaults.openers.file, "system.editor");
    assert_eq!(defaults.openers.url, "system.browser");
    let settings: Settings = toml::from_str(
        "[clipboard]\ncopy_on_select = true\n[openers]\nfile = 'extension:editor'\nurl = 'system.browser'\nproject_target = 'com.apple.finder'\n",
    )?;
    assert!(settings.clipboard.copy_on_select);
    assert_eq!(settings.openers.file, "extension:editor");
    assert_eq!(
        settings.openers.project_target.as_deref(),
        Some("com.apple.finder")
    );
    assert_eq!(
        toml::from_str::<Settings>(&toml::to_string(&settings)?)?,
        settings
    );
    assert!(toml::from_str::<Settings>("[clipboard]\ncopy_on_select = 'yes'").is_err());
    Ok(())
}

#[test]
fn prompt_shortcuts_match_main_and_command_selection_can_be_bound() -> Result {
    use muxy_core::shortcuts::{ShortcutSettings, WORKSPACE_CLIPBOARD_CONTEXT};
    let defaults = Keymap::default();
    assert_eq!(
        defaults.keys("previous_prompt", Some(WORKSPACE_CLIPBOARD_CONTEXT)),
        ["cmd-up", "cmd-shift-up"]
    );
    assert_eq!(
        defaults.keys("next_prompt", Some(WORKSPACE_CLIPBOARD_CONTEXT)),
        ["cmd-down", "cmd-shift-down"]
    );
    assert!(defaults.chord(ShortcutId::SelectCommandOutput).is_none());
    let customized: Settings = toml::from_str(
        "[keymap]\nprevious_prompt = 'ctrl-alt-p'\nselect_command_output = 'cmd-shift-a'",
    )?;
    assert_eq!(
        customized
            .keymap
            .keys("previous_prompt", Some(WORKSPACE_CLIPBOARD_CONTEXT)),
        ["ctrl-alt-p"]
    );
    assert_eq!(
        customized
            .keymap
            .chord(ShortcutId::SelectCommandOutput)
            .map(KeyChord::as_str),
        Some("cmd-shift-a")
    );
    let claimed: Settings = toml::from_str("[keymap]\nnew_tab = 'cmd-up'")?;
    assert!(claimed.keymap.chord(ShortcutId::PreviousPrompt).is_none());
    Ok(())
}

#[test]
fn terminal_save_preserves_comments_unknown_keys_and_includes_and_returns_effective_values()
-> Result {
    let fixture = Fixture::new()?;
    fixture.write("included.conf", "font-size = 22\n")?;
    let path = fixture.write("ghostty.conf", "# keep this comment\r\nunknown-option = keep\r\nconfig-file = included.conf\r\nfont-family = Menlo\r\nfont-size = 13\r\nfont-size = 14\r\nadjust-cell-height = 0")?;
    let requested = TerminalSettings {
        font: muxy_app_core::settings::FontOptions::default(),
        font_families: vec!["SF Mono".into(), "Menlo".into()],
        font_size: 18.0,
        cell_height: CellHeight::Percent(10.0),
        macos_option_as_alt: muxy_app_core::settings::OptionAsAlt::default(),
        ..TerminalSettings::default()
    };
    let effective = requested.save(&path)?;
    assert_eq!(effective.font_size, 22.0);
    assert_eq!(effective.font_families, requested.font_families);
    assert_eq!(effective.cell_height, requested.cell_height);
    let source = fs::read_to_string(&path)?;
    assert!(source.starts_with(
        "# keep this comment\r\nunknown-option = keep\r\nconfig-file = included.conf\r\n"
    ));
    assert_eq!(source.matches("font-size =").count(), 1);
    assert_eq!(TerminalSettings::load_with_seed(&path, None)?, effective);
    Ok(())
}

#[test]
fn invalid_terminal_values_or_includes_leave_the_original_file_unchanged() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write(
        "ghostty.conf",
        "# original\nfont-size = 13\nconfig-file = missing.conf\n",
    )?;
    let original = fs::read(&path)?;
    assert!(TerminalSettings::default().save(&path).is_err());
    assert_eq!(fs::read(&path)?, original);
    for settings in [
        TerminalSettings {
            font_size: f32::NAN,
            ..TerminalSettings::default()
        },
        TerminalSettings {
            cell_height: CellHeight::Percent(f32::INFINITY),
            ..TerminalSettings::default()
        },
        TerminalSettings {
            font_families: vec!["Menlo\nfont-size = 32".into()],
            ..TerminalSettings::default()
        },
    ] {
        assert!(settings.save(&path).is_err());
        assert_eq!(fs::read(&path)?, original);
    }
    Ok(())
}

#[test]
fn runtime_keymap_rebinding_reset_and_persistence_preserve_contexts_and_other_sections() -> Result {
    use muxy_core::shortcuts::ShortcutSettings;
    let fixture = Fixture::new()?;
    let path = fixture.0.join("settings.toml");
    let settings = Settings::load(&path)?;
    let custom = settings
        .keymap
        .with_binding("new_tab", Some("cmd-n".parse()?))?;
    assert_eq!(custom.binding("new_home_tab"), None);
    assert!(
        custom
            .with_binding("close_tab", Some("cmd-n".parse()?))
            .is_err()
    );
    custom.save(&path)?;
    let loaded = Settings::load(&path)?;
    assert_eq!(loaded.keymap, custom);
    assert_eq!(loaded.window, settings.window);
    assert_eq!(
        loaded.keymap.keys("text_input.copy", Some("TextInput")),
        vec!["cmd-c"]
    );
    assert_eq!(
        loaded.keymap.keys("popover.dismiss", Some("Picker")),
        vec!["escape"]
    );
    let reset = loaded.keymap.with_binding("new_tab", None)?;
    reset.save(&path)?;
    assert_eq!(Settings::load(&path)?.keymap, Keymap::default());
    Ok(())
}

#[test]
fn preference_sections_preserve_unrelated_values_and_validate_window_size_before_saving() -> Result
{
    let fixture = Fixture::new()?;
    let path = fixture.0.join("settings.toml");
    let mut settings = Settings::load(&path)?;
    settings.window.default_size = [960.0, 720.0];
    settings.save_window(&path)?;
    settings.clipboard.copy_on_select = true;
    settings.save_clipboard(&path)?;
    settings.panes.new_pane_directory = muxy_app_core::settings::NewPaneDirectory::Current;
    settings.save_panes(&path)?;
    assert_eq!(Settings::load(&path)?, settings);
    let original = fs::read(&path)?;
    settings.window.default_size[0] = 200.0;
    assert!(settings.save_window(&path).is_err());
    assert_eq!(fs::read(path)?, original);
    Ok(())
}

#[test]
fn editing_terminal_values_does_not_copy_included_fonts_into_the_root() -> Result {
    let fixture = Fixture::new()?;
    fixture.write("included.conf", "font-family = Monaco\nfont-size = 13\n")?;
    let path = fixture.write(
        "ghostty.conf",
        "font-family = Menlo\nfont-size = 13\nconfig-file = included.conf\n",
    )?;
    let mut settings = TerminalSettings::load_with_seed(&path, None)?;
    assert_eq!(settings.font_families, ["Menlo", "Monaco"]);
    let keys = TerminalSettings::included_keys(&path)?;
    assert!(keys.contains("font-family") && keys.contains("font-size"));
    for height in [CellHeight::Pixels(2), CellHeight::Pixels(4)] {
        settings.cell_height = height;
        settings = settings.save(&path)?;
        assert_eq!(settings.font_families, ["Menlo", "Monaco"]);
        assert!(!fs::read_to_string(&path)?.contains("font-family = Monaco"));
    }
    let claimed = Keymap::default().with_binding("new_tab", Some("cmd-n".parse()?))?;
    assert!(claimed.with_binding("new_home_tab", None).is_err());
    Ok(())
}

#[test]
fn terminal_font_controls_and_retina_metrics_are_preserved() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write("fonts.conf", "font-family-bold = Custom Bold\nfont-family-italic = Custom Italic\nfont-family-bold-italic = Custom Bold Italic\nfont-feature = -calt\nfont-feature = ss01=2\nfont-codepoint-map = U+2500-U+257F,U+E0B0=Symbols\nfont-thicken = true\nfont-thicken-strength = 128\nadjust-cell-width = 1\n")?;
    let settings = TerminalSettings::load_with_seed(&path, None)?;
    assert_eq!(settings.font.bold, ["Custom Bold"]);
    assert_eq!(
        settings.font.features,
        [("calt".into(), 0), ("ss01".into(), 2)]
    );
    assert_eq!(settings.font.codepoints.len(), 2);
    assert!(settings.font.thicken);
    assert_eq!(settings.font.thicken_strength, 128);
    assert_eq!(CellHeight::Natural.apply(7.4, 2.0), 7.5);
    assert_eq!(CellHeight::Pixels(1).apply(7.4, 2.0), 8.0);
    assert_eq!(CellHeight::Pixels(-1).apply(7.4, 2.0), 7.0);
    Ok(())
}

#[test]
fn advanced_font_settings_round_trip_and_preserve_includes() -> Result {
    let fixture = Fixture::new()?;
    fixture.write(
        "included.conf",
        "font-feature = ss02\nfont-family-bold = Alternate Bold\n",
    )?;
    let path = fixture.write("ghostty.conf", "# retained\nconfig-file = included.conf\nfont-feature = \"calt\" 0\nfont-codepoint-map = U+2500=Symbols\n")?;
    let mut settings = TerminalSettings::load_with_seed(&path, None)?;
    assert_eq!(
        settings.font.features,
        [("calt".into(), 0), ("ss02".into(), 1)]
    );
    settings.font_size = 16.0;
    assert_eq!(settings.save(&path)?, settings);
    assert!(!fs::read_to_string(&path)?.contains("font-family-bold"));
    let other = fixture.0.join("standalone.conf");
    assert_eq!(settings.save(&other)?, settings);
    let before = fs::read(&other)?;
    settings.font.bold.push("bad\nfont-size=99".into());
    assert!(settings.save(&other).is_err());
    assert_eq!(fs::read(other)?, before);
    Ok(())
}

#[test]
fn option_as_alt_parses_resets_and_preserves_settings_on_save() -> Result {
    use muxy_app_core::settings::OptionAsAlt;
    let fixture = Fixture::new()?;
    for (source, expected) in [
        ("macos-option-as-alt = true", OptionAsAlt::True),
        ("macos-option-as-alt = false", OptionAsAlt::False),
        ("macos-option-as-alt = \"left\"", OptionAsAlt::Left),
        ("macos-option-as-alt = right", OptionAsAlt::Right),
        ("macos-option-as-alt", OptionAsAlt::True),
        (
            "macos-option-as-alt = false\nmacos-option-as-alt =",
            OptionAsAlt::True,
        ),
    ] {
        let path = fixture.write("ghostty.conf", source)?;
        let mut settings = TerminalSettings::load(&path)?;
        assert_eq!(settings.macos_option_as_alt, expected);
        settings.font_size = 17.0;
        assert_eq!(settings.save(&path)?.macos_option_as_alt, expected);
        assert!(fs::read_to_string(&path)?.starts_with(source));
        settings.macos_option_as_alt = OptionAsAlt::False;
        assert_eq!(
            settings.save(&path)?.macos_option_as_alt,
            OptionAsAlt::False
        );
        assert_eq!(TerminalSettings::load(&path)?, settings);
    }
    let path = fixture.write("ghostty.conf", "# example\nmacos-option-as-alt = invalid\n")?;
    let error = TerminalSettings::load(&path)
        .err()
        .ok_or("accepted invalid option")?
        .to_string();
    assert!(
        error.contains("ghostty.conf:2 macos-option-as-alt"),
        "{error}"
    );
    Ok(())
}

#[test]
fn included_option_as_alt_applies_and_is_not_replaced_by_font_edits() -> Result {
    use muxy_app_core::settings::OptionAsAlt;
    let fixture = Fixture::new()?;
    fixture.write("input.conf", "macos-option-as-alt = right\n")?;
    let path = fixture.write(
        "ghostty.conf",
        "macos-option-as-alt = true\nconfig-file = input.conf\n",
    )?;
    let mut settings = TerminalSettings::load(&path)?;
    assert_eq!(settings.macos_option_as_alt, OptionAsAlt::Right);
    assert!(TerminalSettings::included_keys(&path)?.contains("macos-option-as-alt"));
    settings.font_size = 18.0;
    assert_eq!(
        settings.save(&path)?.macos_option_as_alt,
        OptionAsAlt::Right
    );
    assert_eq!(
        fs::read_to_string(fixture.0.join("input.conf"))?,
        "macos-option-as-alt = right\n"
    );
    for (left, right) in [(false, false), (true, false), (false, true), (true, true)] {
        assert!(OptionAsAlt::True.enabled(left, right));
        assert!(!OptionAsAlt::False.enabled(left, right));
        assert_eq!(OptionAsAlt::Left.enabled(left, right), left);
        assert_eq!(OptionAsAlt::Right.enabled(left, right), right);
    }
    Ok(())
}

#[test]
fn ghostty_terminal_options_bindings_and_diagnostics_round_trip() -> Result {
    use muxy_app_core::settings::{TerminalAction, TerminalColor};
    let fixture = Fixture::new()?;
    let path = fixture.write(
        "ghostty.conf",
        r#"# keep this comment
theme = dark:"Muxy",light:"Muxy Light"
background = #123456
foreground = abcdef
palette = 1=111111,196=234567
cursor-color = cell-foreground
cursor-text = cell-background
cursor-opacity = 0.6
cursor-style = bar
cursor-style-blink = false
selection-background = cell-foreground
selection-foreground = cell-background
selection-clear-on-typing = true
selection-clear-on-copy = true
background-opacity = 0.8
background-opacity-cells = true
bold-is-bright = true
copy-on-select = clipboard
mouse-reporting = false
mouse-scroll-multiplier = precision:2,discrete:4
scroll-to-bottom = no-keystroke,output
window-padding-x = 4,8
window-padding-y = 6
window-padding-balance = true
window-padding-color = background
keybind = shift+enter=text:\x1b\r
keybind = alt+arrow_left=csi:1;3D
keybind = super+c=ignore
keybind = global:super+a=new_window
window-save-state = always
"#,
    )?;
    let mut settings = TerminalSettings::load_with_seed(&path, None)?;
    let options = &settings.options;
    assert_eq!(options.background, Some(0x12_34_56));
    assert_eq!(options.palette[&196], 0x23_45_67);
    assert_eq!(options.cursor_style, Some(muxy_protocol::CursorShape::Bar));
    assert_eq!(options.cursor_blink, Some(false));
    assert_eq!(options.cursor_text, Some(TerminalColor::CellBackground));
    assert_eq!(options.padding_x, [4.0, 8.0]);
    assert_eq!(options.padding_y, [6.0; 2]);
    assert_eq!(
        (options.scroll_precision, options.scroll_discrete),
        (2.0, 4.0)
    );
    assert!(!options.scroll_on_keystroke && options.scroll_on_output);
    assert_eq!(
        settings.keybindings.bindings[&"shift-enter".parse()?],
        TerminalAction::Text(b"\x1b\r".to_vec())
    );
    assert_eq!(
        settings.keybindings.bindings[&"alt-left".parse()?],
        TerminalAction::Text(b"\x1b[1;3D".to_vec())
    );
    assert_eq!(settings.diagnostics.len(), 2);
    assert!(
        settings
            .diagnostics
            .iter()
            .all(|warning| warning.contains("ghostty.conf:"))
    );
    settings.font_size = 21.0;
    assert_eq!(settings.save(&path)?, settings);
    assert!(fs::read_to_string(&path)?.contains("keybind = shift+enter=text:\\x1b\\r"));
    settings.options.background = Some(0x65_43_21);
    settings.options.palette.remove(&1);
    settings
        .keybindings
        .bindings
        .insert("ctrl--".parse()?, TerminalAction::Text(vec![0, 0xff, 0x1b]));
    let saved = settings.save(&path)?;
    assert_eq!(saved.options, settings.options);
    assert_eq!(saved.keybindings, settings.keybindings);
    assert!(fs::read_to_string(&path)?.contains("# keep this comment"));
    Ok(())
}

#[test]
fn terminal_alias_defaults_can_be_overridden_unbound_and_cleared() -> Result {
    use muxy_app_core::settings::TerminalAction;
    let fixture = Fixture::new()?;
    let path = fixture.write("ghostty.conf", "font-size = 14\n")?;
    let newline = "shift-enter".parse()?;
    let paste = "cmd-shift-v".parse()?;
    let settings = TerminalSettings::load_with_seed(&path, None)?;
    assert_eq!(
        settings.keybindings.action(&newline),
        Some(&TerminalAction::Text(b"\n".to_vec()))
    );
    assert_eq!(
        settings.keybindings.action(&paste),
        Some(&TerminalAction::Paste)
    );
    assert!(settings.keybindings.bindings.is_empty());
    for (config, expected) in [
        (
            "keybind = shift+enter=text:\\x1b\\r",
            Some(TerminalAction::Text(b"\x1b\r".to_vec())),
        ),
        ("keybind = shift+enter=unbind", Some(TerminalAction::Unbind)),
        ("keybind = shift+enter=ignore", Some(TerminalAction::Ignore)),
        ("keybind = clear", None),
        (
            "keybind = clear\nkeybind = shift+enter=text:\\n",
            Some(TerminalAction::Text(b"\n".to_vec())),
        ),
        (
            "keybind = clear\nkeybind =",
            Some(TerminalAction::Text(b"\n".to_vec())),
        ),
    ] {
        fs::write(&path, config)?;
        let mut settings = TerminalSettings::load_with_seed(&path, None)?;
        assert_eq!(
            settings.keybindings.action(&newline),
            expected.as_ref(),
            "{config}"
        );
        settings
            .keybindings
            .bindings
            .insert("alt-enter".parse()?, TerminalAction::Text(b"\n".to_vec()));
        let saved = settings.save(&path)?;
        assert_eq!(saved.keybindings, settings.keybindings);
        assert_eq!(saved.keybindings.action(&newline), expected.as_ref());
        assert!(!fs::read_to_string(&path)?.contains("cmd+shift+v"));
    }
    Ok(())
}

#[test]
fn ghostty_option_resets_and_errors_keep_source_locations() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write("ghostty.conf", "background = 123456\nbackground =\npalette = 200=123456\npalette =\nkeybind = shift+enter=text:hello\nkeybind =\nbackground-opacity = 2\nconfig-file = included.conf\n")?;
    fixture.write(
        "included.conf",
        "mouse-reporting = false\nwindow-padding-x = 10,12\n",
    )?;
    let settings = TerminalSettings::load_with_seed(&path, None)?;
    assert_eq!(settings.options.background, None);
    assert!(settings.options.palette.is_empty());
    assert!(settings.keybindings.bindings.is_empty());
    assert_eq!(settings.options.background_opacity, Some(1.0));
    assert!(!settings.options.mouse_reporting);
    assert_eq!(settings.options.padding_x, [10.0, 12.0]);
    for line in [
        "background = invalid",
        "palette = 256=123456",
        "cursor-style = invalid",
        "cursor-opacity = NaN",
        "window-padding-x = -1",
        "mouse-scroll-multiplier = infinity",
        "scroll-to-bottom = invalid",
        "keybind = shift+enter=text:\\xZZ",
    ] {
        fs::write(&path, format!("# comment\n{line}\n"))?;
        let error = TerminalSettings::load_with_seed(&path, None)
            .expect_err(line)
            .to_string();
        assert!(error.contains("ghostty.conf:2"), "{error}");
        assert!(
            error.contains(line.split('=').next().unwrap_or_default().trim()),
            "{error}"
        );
    }
    Ok(())
}
