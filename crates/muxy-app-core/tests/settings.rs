#![allow(
    clippy::float_cmp,
    reason = "Configuration values and integer zoom steps must round-trip exactly"
)]

use muxy_core::shortcuts::ShortcutId;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use muxy_app_core::ServerId;
use muxy_app_core::settings::{
    CellHeight, KeyChord, Keymap, ServerEntry, Settings, TerminalSettings,
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

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
fn project_sort_modes_preserve_legacy_name_settings_and_unrelated_preferences() -> Result {
    use muxy_app_core::settings::{Appearance, ProjectOrder};

    let fixture = Fixture::new()?;
    let path = fixture.write(
        "settings.toml",
        "[appearance]\ntab_focused_project_order = \"name\"\ntips_visible = false\n[window]\nconfirm_running_process = false\n",
    )?;
    let mut appearance = Appearance::load(&path)?;
    assert_eq!(appearance.sidebar_project_order, ProjectOrder::Name);
    for order in [
        ProjectOrder::NameDescending,
        ProjectOrder::RecentlyActive,
        ProjectOrder::DateAdded,
        ProjectOrder::Manual,
        ProjectOrder::Name,
    ] {
        let previous = appearance.clone();
        appearance.sidebar_project_order = order;
        appearance = appearance.save_changes(&previous, &path)?;
        let saved = Settings::load(&path)?;
        assert_eq!(saved.appearance, appearance);
        assert!(!saved.appearance.tips_visible);
        assert!(!saved.window.confirm_running_process);
    }
    Ok(())
}

#[test]
fn agents_panel_visibility_preserves_saved_choices_and_older_settings() -> Result {
    let fixture = Fixture::new()?;
    for (source, visible) in [
        ("[window]\nconfirm_running_process = false\n", true),
        ("[appearance]\nstatus_bar_visible = false\n", true),
        ("[appearance]\nagents_panel_visible = false\n", false),
    ] {
        let path = fixture.write("settings.toml", source)?;
        let mut settings = Settings::load(&path)?;
        assert_eq!(settings.appearance.agents_panel_visible, visible);
        assert_eq!(fs::read_to_string(&path)?, source);

        let previous = settings.appearance.clone();
        settings.appearance.tips_visible = false;
        settings.appearance = settings.appearance.save_changes(&previous, &path)?;
        assert_eq!(Settings::load(&path)?, settings);
        assert_eq!(settings.appearance.agents_panel_visible, visible);
    }
    Ok(())
}

#[test]
fn agents_panel_docking_preserves_legacy_floating_settings_and_saved_geometry() -> Result {
    use muxy_app_core::settings::AgentsPanelDock;

    let fixture = Fixture::new()?;
    let path = fixture.write(
        "settings.toml",
        "[appearance]\nagents_panel_detached = true\nagents_panel_frame = [80.0, 90.0, 300.0, 400.0]\nagents_panel_docked_height = 200.0\n",
    )?;
    let mut settings = Settings::load(&path)?;
    assert_eq!(settings.appearance.agents_panel_dock, AgentsPanelDock::Left);
    assert!(settings.appearance.agents_panel_detached);
    let previous = settings.appearance.clone();
    settings.appearance.agents_panel_dock = AgentsPanelDock::Right;
    settings.appearance.agents_panel_right_width = Some(410.0);
    settings.appearance = settings.appearance.save_changes(&previous, &path)?;
    let loaded = Settings::load(&path)?;
    assert_eq!(loaded, settings);
    assert_eq!(
        loaded.appearance.agents_panel_frame,
        previous.agents_panel_frame
    );
    assert_eq!(loaded.appearance.agents_panel_docked_height, Some(200.0));
    assert!(loaded.appearance.agents_panel_detached);
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
            ShortcutId::SelectCommandOutput
                | ShortcutId::DetachTerminal
                | ShortcutId::ToggleAgentSessions
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
fn ghostty_defaults_are_created_once_and_existing_config_is_preserved() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.0.join("ghostty.conf");
    assert_eq!(TerminalSettings::load(&path)?, TerminalSettings::default());
    let source = "font-family = Monaco\nfont-size = 16\n";
    fs::write(&path, source)?;
    let settings = TerminalSettings::load(&path)?;
    assert_eq!(settings.font_size, 16.0);
    assert_eq!(settings.font_families, ["Monaco"]);
    assert_eq!(fs::read_to_string(path)?, source);
    Ok(())
}

#[test]
fn ghostty_theme_is_ignored_because_the_settings_theme_colors_terminals() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write("ghostty.conf", "theme = \"Cursor Dark\"\nfont-size = 16\n")?;
    let settings = TerminalSettings::load(&path)?;
    assert_eq!(settings.font_size, 16.0);
    assert_eq!(settings.options, TerminalSettings::default().options);
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
fn existing_bindings_take_precedence_over_new_project_defaults_and_round_trip() -> Result {
    let fixture = Fixture::new()?;
    for project_action in [
        ShortcutId::AddProject,
        ShortcutId::PreviousProject,
        ShortcutId::NextProject,
        ShortcutId::SelectProject1,
        ShortcutId::SelectProject9,
    ] {
        let chord = project_action.definition().keys[0];
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

#[cfg(not(target_os = "macos"))]
#[test]
fn default_shortcuts_avoid_shifted_digits_and_symbols_which_arrive_without_shift() -> Result {
    use muxy_app_core::settings::{TerminalAction, TerminalBindings};

    let terminal = TerminalBindings::default();
    let actions = [
        TerminalAction::Text(b"\n".to_vec()),
        TerminalAction::Paste,
        TerminalAction::SelectAll,
        TerminalAction::ScrollTop,
        TerminalAction::ScrollBottom,
        TerminalAction::ScrollPageUp,
        TerminalAction::ScrollPageDown,
        TerminalAction::ResetFontSize,
    ];
    let mut chords = Vec::new();
    for key in muxy_core::shortcuts::ALL
        .iter()
        .flat_map(|shortcut| shortcut.keys)
    {
        chords.push(key.parse::<KeyChord>()?);
    }
    for action in &actions {
        chords.extend(terminal.chords_for_action(action).cloned());
    }
    assert!(chords.len() > actions.len());
    for chord in chords {
        let chord = chord.as_str();
        let key = if chord.ends_with("--") || chord == "-" {
            "-"
        } else {
            chord.rsplit('-').next().ok_or("empty chord")?
        };
        let shifted = chord
            .strip_suffix(key)
            .is_some_and(|modifiers| modifiers.contains("shift-"));
        let unshifted = key.chars().count() == 1 && key.to_lowercase() == key.to_uppercase();
        assert!(!(shifted && unshifted), "{chord} can never be pressed");
    }
    Ok(())
}

#[test]
fn an_existing_binding_keeps_the_key_equalize_splits_defaults_to() -> Result {
    let fixture = Fixture::new()?;
    let key = ShortcutId::EqualizeSplits.definition().keys[0];
    let chord: KeyChord = key.parse()?;
    assert_eq!(
        Keymap::default().action(&chord),
        Some(ShortcutId::EqualizeSplits)
    );
    for existing in ["new_tab", "\"command.tests\""] {
        let source = format!("[keymap]\n{existing} = '{key}'\n");
        let path = fixture.write("settings.toml", &source)?;
        let keymap = Settings::load(&path)?.keymap;
        assert_eq!(keymap.binding(existing.trim_matches('"')), Some(&chord));
        assert_eq!(keymap.chord(ShortcutId::EqualizeSplits), None);
        assert_eq!(fs::read_to_string(&path)?, source);
    }
    Ok(())
}

#[test]
fn a_command_keeps_a_key_a_newer_default_claims_but_cannot_take_one_in_use() -> Result {
    let fixture = Fixture::new()?;
    let key = ShortcutId::NewTabInProject.definition().keys[0];
    let path = fixture.write(
        "settings.toml",
        &format!("[keymap]\n\"command.tests\" = '{key}'\n"),
    )?;
    let keymap = Settings::load(&path)?.keymap;
    let chord: KeyChord = key.parse()?;
    assert_eq!(keymap.binding("command.tests"), Some(&chord));
    assert_eq!(keymap.chord(ShortcutId::NewTabInProject), None);
    assert!(
        keymap
            .with_binding(
                "command.other",
                Some(ShortcutId::NewTab.definition().keys[0].parse()?)
            )
            .is_err()
    );

    let moved = keymap.with_binding("command.tests", Some("ctrl-alt-f24".parse()?))?;
    assert_eq!(moved.chord(ShortcutId::NewTabInProject), Some(&chord));
    assert!(moved.with_binding("command.tests", Some(chord)).is_err());
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
    assert_eq!(TerminalSettings::load(&path)?, effective);
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
    let custom = settings.keymap.with_binding(
        "new_tab",
        Some(ShortcutId::NewHomeTab.definition().keys[0].parse()?),
    )?;
    assert_eq!(custom.binding("new_home_tab"), None);
    assert!(
        custom
            .with_binding(
                "close_tab",
                Some(ShortcutId::NewHomeTab.definition().keys[0].parse()?)
            )
            .is_err()
    );
    custom.save(&path)?;
    let loaded = Settings::load(&path)?;
    assert_eq!(loaded.keymap, custom);
    assert_eq!(loaded.window, settings.window);
    assert_eq!(
        loaded.keymap.keys("text_input.copy", Some("TextInput")),
        vec![ShortcutId::TextInputCopy.definition().keys[0]]
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
fn editing_terminal_values_does_not_copy_included_fonts_into_the_root() -> Result {
    let fixture = Fixture::new()?;
    fixture.write("included.conf", "font-family = Monaco\nfont-size = 13\n")?;
    let path = fixture.write(
        "ghostty.conf",
        "font-family = Menlo\nfont-size = 13\nconfig-file = included.conf\n",
    )?;
    let mut settings = TerminalSettings::load(&path)?;
    assert_eq!(settings.font_families, ["Menlo", "Monaco"]);
    let keys = TerminalSettings::included_keys(&path)?;
    assert!(keys.contains("font-family") && keys.contains("font-size"));
    for height in [CellHeight::Pixels(2), CellHeight::Pixels(4)] {
        settings.cell_height = height;
        settings = settings.save(&path)?;
        assert_eq!(settings.font_families, ["Menlo", "Monaco"]);
        assert!(!fs::read_to_string(&path)?.contains("font-family = Monaco"));
    }
    let claimed = Keymap::default().with_binding(
        "new_tab",
        Some(ShortcutId::NewHomeTab.definition().keys[0].parse()?),
    )?;
    assert!(claimed.with_binding("new_home_tab", None).is_err());
    Ok(())
}

#[test]
fn ghostty_terminal_options_bindings_and_diagnostics_round_trip() -> Result {
    use muxy_app_core::settings::{TerminalAction, TerminalColor};
    let fixture = Fixture::new()?;
    let path = fixture.write(
        "ghostty.conf",
        r"# keep this comment
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
",
    )?;
    let mut settings = TerminalSettings::load(&path)?;
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

fn server_names(settings: &Settings) -> Vec<&str> {
    settings
        .servers
        .iter()
        .map(|server| server.name.as_str())
        .collect()
}

#[test]
fn servers_load_and_saves_keep_other_settings_and_hand_edits() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write(
        "settings.toml",
        "[window]\nconfirm_running_process = false\n\n[[servers]]\nid = \"6F9619FF-8B86-D011-B42D-00C04FC964FF\"\nname = \"box\"\nssh = \"dev@box\"\n",
    )?;
    let mut settings = Settings::load(&path)?;
    let first: ServerId = "6f9619ff8b86d011b42d00c04fc964ff".parse()?;
    assert_eq!(
        settings.servers,
        [ServerEntry {
            id: first,
            name: "box".into(),
            ssh: "dev@box".into(),
            ssh_command: None,
            identity_file: None,
            password_login: false,
            group: None,
        }]
    );
    assert_eq!(
        settings.server(first).map(|server| server.ssh.as_str()),
        Some("dev@box")
    );

    let build = ServerEntry::new("build".into(), "ci@build.example.com".into());
    settings.add_server(build.clone(), &path)?;
    let hand = ServerEntry::new("hand".into(), "hand@host".into());
    let source = fs::read_to_string(&path)?;
    fs::write(
        &path,
        format!(
            "{source}\n[[servers]]\nid = \"{}\"\nname = \"hand\"\nssh = \"hand@host\"\n",
            hand.id
        ),
    )?;
    settings.update_server(
        ServerEntry {
            name: "builder".into(),
            identity_file: Some("~/.ssh/build key".into()),
            password_login: true,
            ..build.clone()
        },
        &path,
    )?;
    assert_eq!(server_names(&settings), ["box", "builder", "hand"]);
    let source = fs::read_to_string(&path)?;
    assert!(source.contains("identity_file = \"~/.ssh/build key\""));
    assert!(source.contains("password_login = true"));
    assert_eq!(
        source.matches("password_login").count(),
        1,
        "false is left out"
    );
    let loaded = Settings::load(&path)?;
    assert_eq!(loaded.servers, settings.servers);
    assert!(!loaded.window.confirm_running_process);
    let backup: Settings = toml::from_str(&muxy_app_core::backup::settings_source(&loaded)?)?;
    assert_eq!(backup.servers, settings.servers);

    settings.remove_servers(&[first], &path)?;
    assert_eq!(server_names(&settings), ["builder", "hand"]);
    settings.remove_servers(&[build.id, hand.id], &path)?;
    assert!(settings.servers.is_empty());
    assert!(!fs::read_to_string(&path)?.contains("servers"));
    assert!(!Settings::load(&path)?.window.confirm_running_process);
    Ok(())
}

#[test]
fn server_groups_are_set_in_one_write_and_keep_hand_edits() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write(
        "settings.toml",
        "[window]\nconfirm_running_process = false\n",
    )?;
    let mut settings = Settings::load(&path)?;
    let api = ServerEntry::new("api".into(), "deploy@api".into());
    let db = ServerEntry::new("db".into(), "deploy@db".into());
    let devbox = ServerEntry {
        group: Some("personal".into()),
        ..ServerEntry::new("devbox".into(), "me@devbox".into())
    };
    for server in [&api, &db, &devbox] {
        settings.add_server(server.clone(), &path)?;
    }
    let source = fs::read_to_string(&path)?;
    assert_eq!(
        source.matches("group = ").count(),
        1,
        "no group is left out"
    );
    let hand = ServerEntry::new("hand".into(), "hand@host".into());
    fs::write(
        &path,
        format!(
            "{source}\n[[servers]]\nid = \"{}\"\nname = \"hand\"\nssh = \"hand@host\"\ngroup = \"Production\"\n",
            hand.id
        ),
    )?;

    settings.set_servers_group(&[api.id, db.id, ServerId::new()], Some("Production"), &path)?;
    let loaded = Settings::load(&path)?;
    assert_eq!(loaded.servers, settings.servers);
    assert_eq!(server_names(&loaded), ["api", "db", "devbox", "hand"]);
    assert_eq!(loaded.server_groups(), ["Production", "personal"]);
    let backup: Settings = toml::from_str(&muxy_app_core::backup::settings_source(&loaded)?)?;
    assert_eq!(backup.servers, loaded.servers);

    settings.set_servers_group(&[devbox.id, hand.id], None, &path)?;
    assert_eq!(Settings::load(&path)?.server_groups(), ["Production"]);
    let before = fs::read_to_string(&path)?;
    for invalid in ["", " ", "Prod\u{7}", &"g".repeat(257)] {
        assert!(
            settings
                .set_servers_group(&[api.id], Some(invalid), &path)
                .is_err(),
            "{invalid:?}"
        );
        assert_eq!(fs::read_to_string(&path)?, before);
    }
    settings.set_servers_group(&[api.id], Some(" Spaced "), &path)?;
    let spaced = Settings::load(&path)?;
    assert_eq!(
        spaced
            .server(api.id)
            .and_then(|server| server.group.as_deref()),
        Some(" Spaced "),
        "a hand-written group with spaces still loads"
    );
    assert!(!spaced.window.confirm_running_process);
    Ok(())
}

#[test]
fn servers_are_placed_and_regrouped_in_one_write_in_listed_order() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write(
        "settings.toml",
        "[window]\nconfirm_running_process = false\n",
    )?;
    let mut settings = Settings::load(&path)?;
    let server = |name: &str, group: Option<&str>| ServerEntry {
        group: group.map(str::to_owned),
        ..ServerEntry::new(name.into(), format!("dev@{name}"))
    };
    let (a, b, c, d) = (
        server("a", None),
        server("b", Some("Build")),
        server("c", Some("Build")),
        server("d", Some("Deploy")),
    );
    for entry in [&a, &b, &c, &d] {
        settings.add_server(entry.clone(), &path)?;
    }
    let listed = |settings: &Settings| -> Vec<(String, Option<String>)> {
        settings
            .servers
            .iter()
            .map(|server| (server.name.clone(), server.group.clone()))
            .collect()
    };
    let named = |name: &str, group: Option<&str>| (name.to_owned(), group.map(str::to_owned));

    settings.place_servers(&[d.id], Some("Build"), Some(c.id), &path)?;
    assert_eq!(
        listed(&settings),
        [
            named("a", None),
            named("b", Some("Build")),
            named("d", Some("Build")),
            named("c", Some("Build")),
        ]
    );
    settings.place_servers(&[a.id, b.id], Some("Deploy"), None, &path)?;
    assert_eq!(
        listed(&settings),
        [
            named("d", Some("Build")),
            named("c", Some("Build")),
            named("a", Some("Deploy")),
            named("b", Some("Deploy")),
        ]
    );
    assert_eq!(settings.server_groups(), ["Build", "Deploy"]);
    let loaded = Settings::load(&path)?;
    assert_eq!(loaded.servers, settings.servers);
    assert!(!loaded.window.confirm_running_process);
    Ok(())
}

#[test]
fn custom_commands_round_trip_beside_legacy_ssh_and_reject_ambiguous_logins() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write(
        "settings.toml",
        "[window]\nconfirm_running_process = false\n",
    )?;
    let mut settings = Settings::load(&path)?;
    settings.add_server(ServerEntry::new("box".into(), "dev@box".into()), &path)?;
    let custom = ServerEntry {
        ssh_command: Some("'/Applications/VM Tool/vm' ssh 'dev box' --".into()),
        ..ServerEntry::new("VM".into(), String::new())
    };
    settings.add_server(custom.clone(), &path)?;
    let loaded = Settings::load(&path)?;
    assert_eq!(loaded.servers, settings.servers);
    assert!(!loaded.window.confirm_running_process);
    let backup: Settings = toml::from_str(&muxy_app_core::backup::settings_source(&loaded)?)?;
    assert_eq!(backup.servers, loaded.servers);
    let source = fs::read_to_string(&path)?;
    assert!(!source.contains("ssh = \"\""));
    for invalid in [
        ServerEntry {
            ssh: "box".into(),
            ..custom.clone()
        },
        ServerEntry {
            identity_file: Some("key".into()),
            ..custom.clone()
        },
        ServerEntry {
            password_login: true,
            ..custom.clone()
        },
        ServerEntry {
            ssh_command: Some("vm 'unterminated".into()),
            ..custom.clone()
        },
        ServerEntry {
            ssh_command: Some(String::new()),
            ..custom.clone()
        },
    ] {
        assert!(settings.update_server(invalid, &path).is_err());
        assert_eq!(fs::read_to_string(&path)?, source);
        assert_eq!(settings.servers, loaded.servers);
    }
    settings.update_server(
        ServerEntry {
            ssh: "another@host".into(),
            ssh_command: None,
            ..custom
        },
        &path,
    )?;
    assert!(
        Settings::load(&path)?
            .servers
            .iter()
            .all(|server| server.ssh_command.is_none())
    );
    Ok(())
}

#[test]
fn invalid_servers_are_rejected_without_changing_the_file() -> Result {
    let fixture = Fixture::new()?;
    let entry = |id: &str, name: &str, ssh: &str| {
        format!("[[servers]]\nid = \"{id}\"\nname = \"{name}\"\nssh = \"{ssh}\"\n")
    };
    let local = ServerId::local().to_string();
    let other = ServerId::new().to_string();
    for source in [
        entry(&local, "This Mac", "me@localhost"),
        entry(&other, "a", "a@host") + &entry(&other, "b", "b@host"),
        entry(&other, " ", "dev@box"),
        entry(&other, "box", " "),
        entry(&other, "box", "dev@box\\u0007"),
        entry(&other, "box", "dev@box") + "port = 22\n",
        entry(&other, "box", "dev@box") + "identity_file = \" \"\n",
        entry("box", "box", "dev@box"),
    ] {
        let path = fixture.write("settings.toml", &source)?;
        assert!(Settings::load(&path).is_err(), "{source}");
        assert_eq!(fs::read_to_string(&path)?, source);
    }

    let path = fixture.write(
        "settings.toml",
        "[window]\nconfirm_running_process = false\n",
    )?;
    let mut settings = Settings::load(&path)?;
    let before = fs::read_to_string(&path)?;
    let this_mac = ServerEntry {
        id: ServerId::local(),
        ..ServerEntry::new("This Mac".into(), "me@localhost".into())
    };
    assert!(settings.add_server(this_mac, &path).is_err());
    let unknown = ServerEntry::new("box".into(), "dev@box".into());
    assert!(settings.update_server(unknown, &path).is_err());
    assert_eq!(fs::read_to_string(&path)?, before);
    assert!(settings.servers.is_empty());
    Ok(())
}
