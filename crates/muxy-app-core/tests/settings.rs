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

#[test]
fn retired_worktree_sorting_preference_is_ignored() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture.write(
        "settings.toml",
        "[appearance]\nworktree_order_by_mru = true\ndark_theme = 'Dracula'\n",
    )?;
    let appearance = Settings::load(&path)?.appearance;
    assert_eq!(appearance.dark_theme, "Dracula");
    assert!(!toml::to_string(&appearance)?.contains("worktree_order_by_mru"));
    appearance.save(&path)?;
    assert_eq!(Settings::load(&path)?.appearance, appearance);
    Ok(())
}

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
    assert!(
        settings
            .diagnostics
            .iter()
            .any(|warning| warning.contains("theme is not supported by Muxy"))
    );
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
fn existing_bindings_take_precedence_over_new_project_defaults_and_round_trip() -> Result {
    let fixture = Fixture::new()?;
    for (project_action, chord) in [
        (ShortcutId::AddProject, "cmd-o"),
        (ShortcutId::PreviousProject, "ctrl-["),
        (ShortcutId::NextProject, "ctrl-]"),
        (ShortcutId::SelectProject1, "ctrl-1"),
        (ShortcutId::SelectProject9, "ctrl-9"),
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
fn retired_color_picker_bindings_still_load_and_are_dropped() -> Result {
    use muxy_core::shortcuts::ShortcutSettings;
    let settings = toml::from_str::<Settings>(
        "[keymap]\n'project_colors.next_color' = 'ctrl-n'\n'menu.close_submenu' = 'ctrl-b'",
    )?;
    assert_eq!(
        settings.keymap.keys("menu.close_submenu", Some("Menu")),
        ["ctrl-b"]
    );
    assert!(
        settings
            .keymap
            .keys("project_colors.next_color", Some("ProjectColors"))
            .is_empty()
    );
    assert_eq!(
        settings.keymap.keys("menu.open_submenu", Some("Menu")),
        ["right"]
    );
    let arrows = toml::from_str::<Settings>(
        "[keymap]\n'menu.highlight_next' = 'right'\n'menu.highlight_previous' = 'left'",
    )?;
    assert_eq!(
        arrows.keymap.keys("menu.highlight_next", Some("Menu")),
        ["right"]
    );
    assert!(
        arrows
            .keymap
            .keys("menu.open_submenu", Some("Menu"))
            .is_empty()
    );
    assert!(
        arrows
            .keymap
            .keys("menu.close_submenu", Some("Menu"))
            .is_empty()
    );
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
    let claimed = Keymap::default().with_binding("new_tab", Some("cmd-n".parse()?))?;
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
            identity_file: None,
            password_login: false,
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

    settings.remove_server(first, &path)?;
    assert_eq!(server_names(&settings), ["builder", "hand"]);
    settings.remove_server(build.id, &path)?;
    settings.remove_server(hand.id, &path)?;
    assert!(settings.servers.is_empty());
    assert!(!fs::read_to_string(&path)?.contains("servers"));
    assert!(!Settings::load(&path)?.window.confirm_running_process);
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
