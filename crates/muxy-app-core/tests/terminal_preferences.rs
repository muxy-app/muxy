use muxy_app_core::settings::{
    CellHeight, FontMap, PaddingColor, TerminalAction, TerminalColor, TerminalEdit,
    TerminalSettings,
};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn migration_preserves_includes_bindings_and_fallbacks_then_uses_native_preferences() -> Result {
    let directory = tempfile::tempdir()?;
    let legacy = directory.path().join("ghostty.conf");
    let included = directory.path().join("fonts.conf");
    let path = directory.path().join("terminal.toml");
    let source = "config-file = fonts.conf\nbackground-opacity = 0.7\npalette = 1=#123456\nkeybind = ctrl+alt+x=text:\\x1b[A\nadjust-cursor-thickness = 200%\nbackground-blur = true\n";
    std::fs::write(&legacy, source)?;
    std::fs::write(
        &included,
        "font-family = First\nfont-family = Fallback\nfont-size = 19\nfont-feature = ss01=1\nfont-codepoint-map = U+E000-U+E001=Icons\n",
    )?;
    let mut settings = TerminalSettings::load_native(&path)?;
    assert_eq!(settings, TerminalSettings::load(&legacy)?);
    assert_eq!(
        settings,
        TerminalSettings::from_native_source(&settings.native_source()?)?
    );
    assert_eq!(
        settings.options.cursor_thickness,
        CellHeight::Percent(200.0)
    );
    assert_eq!(settings.options.background_vibrancy, 70);
    let bindings = settings.keybindings.clone();
    settings.set_preference("font-family", "Replacement")?;
    settings.set_preference("font-size", "21")?;
    settings.set_preference("font-ligatures", "false")?;
    settings.save_native(&path)?;
    std::fs::remove_file(included)?;
    let restored = TerminalSettings::load_native(&path)?;
    assert_eq!(restored, settings);
    assert_eq!(restored.font_families, ["Replacement", "Fallback"]);
    assert_eq!(restored.keybindings, bindings);
    assert!(restored.font.features.contains(&("ss01".into(), 1)));
    assert_eq!(std::fs::read_to_string(legacy)?, source);
    Ok(())
}

#[test]
fn invalid_edits_and_corrupted_native_files_preserve_saved_preferences() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("terminal.toml");
    let mut settings = TerminalSettings::load_native(&path)?;
    settings.set_preference("background-opacity", "75")?;
    settings.save_native(&path)?;
    let original = settings.clone();
    let source = std::fs::read_to_string(&path)?;
    for (id, value) in [
        ("background-transparency", "NaN"),
        ("background-vibrancy", "101"),
        ("padding-right", "-1"),
        ("background-opacity", "NaN"),
        ("background-opacity", "101"),
        ("font-size", "0"),
        ("window-padding-x", "-1"),
        ("scroll-precision", "inf"),
        ("adjust-cursor-thickness", "-100%"),
        ("font-family", ""),
    ] {
        assert!(settings.set_preference(id, value).is_err());
        assert_eq!(settings, original);
    }
    settings.options.scroll_discrete = f32::NAN;
    assert!(settings.save_native(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path)?, source);
    std::fs::write(&path, "font_size = 'broken'")?;
    assert!(TerminalSettings::load_native(&path).is_err());
    assert!(original.save_native(&path).is_err());
    assert_eq!(std::fs::read_to_string(path)?, "font_size = 'broken'");
    Ok(())
}

#[test]
fn failed_migration_keeps_legacy_source_and_does_not_create_native_settings() -> Result {
    let directory = tempfile::tempdir()?;
    let legacy = directory.path().join("ghostty.conf");
    let path = directory.path().join("terminal.toml");
    let source = "config-file = missing.conf\nfont-size = 18\n";
    std::fs::write(&legacy, source)?;
    assert!(TerminalSettings::load_native(&path).is_err());
    assert!(!path.exists());
    assert_eq!(std::fs::read_to_string(legacy)?, source);
    assert!(TerminalSettings::from_legacy_source("config-file = /etc/passwd").is_err());
    Ok(())
}

#[test]
fn legacy_font_names_with_quotes_migrate_without_blocking_startup_or_import() -> Result {
    let directory = tempfile::tempdir()?;
    let legacy = directory.path().join("ghostty.conf");
    let path = directory.path().join("terminal.toml");
    let source = "font-family = Primary\"Name\nfont-family = Fallback\nfont-family = \"Unterminated\nfont-family-italic = Italic\"Name\nfont-family-bold = \"Bold\nfont-codepoint-map = U+E000-U+F8FF=\"Symbols Nerd Font\"\nfont-codepoint-map = U+F000=\n";
    std::fs::write(&legacy, source)?;
    let migrated = TerminalSettings::load_native(&path)?;
    assert_eq!(migrated.font_families, ["Fallback"]);
    assert!(migrated.font.italic.is_empty());
    assert!(migrated.font.bold.is_empty());
    assert_eq!(migrated.font.codepoints.len(), 1);
    assert_eq!(migrated.font.codepoints[0].family, "Symbols Nerd Font");
    assert_eq!(migrated.diagnostics.len(), 5);
    assert!(
        migrated
            .diagnostics
            .iter()
            .all(|note| note.starts_with("ghostty.conf:"))
    );
    assert_eq!(TerminalSettings::load_native(&path)?, migrated);
    let mut dismissed = migrated.clone();
    dismissed.set_preference("dismiss-import-notes", "")?;
    dismissed.save_native(&path)?;
    assert!(TerminalSettings::load_native(&path)?.diagnostics.is_empty());
    assert_eq!(std::fs::read_to_string(legacy)?, source);
    assert_eq!(TerminalSettings::from_legacy_source(source)?, migrated);
    assert_eq!(
        TerminalSettings::from_legacy_source("font-family = Only\"Name\n")?.font_families,
        TerminalSettings::default().font_families
    );
    Ok(())
}

#[test]
#[allow(clippy::float_cmp)]
fn native_background_and_asymmetric_padding_survive_legacy_merges() -> Result {
    let mut settings = TerminalSettings::from_legacy_source(
        "window-padding-x = 4,12\nwindow-padding-y = 8,20\nbackground-blur = true\n",
    )?;
    settings.set_preference("padding-left", "6")?;
    settings.set_preference("padding-bottom", "24")?;
    settings.set_preference("background-vibrancy", "45")?;
    settings.set_preference("background-transparency", "25")?;
    let restored = TerminalSettings::from_legacy_source(&settings.legacy_source())?;
    assert_eq!(restored.options.padding_x, [6.0, 12.0]);
    assert_eq!(restored.options.padding_y, [8.0, 24.0]);
    assert_eq!(restored.options.background_vibrancy, 45);
    assert_eq!(restored.options.background_opacity, Some(0.75));
    assert_eq!(
        TerminalSettings::from_native_source(&restored.native_source()?)?,
        restored
    );
    Ok(())
}

#[test]
fn legacy_blur_spellings_migrate_without_blocking_startup_or_import() -> Result {
    for (line, expected) in [
        ("background-blur", 70),
        ("background-blur = macos-glass-regular", 70),
        ("background-blur = macos-glass-clear", 70),
        ("background-blur = t", 70),
        ("background-blur = F", 0),
        ("background-blur = 1", 70),
        ("background-blur = 0x1", 1),
        ("background-blur = 0b10100", 20),
        ("background-blur = 0o24", 20),
        ("background-blur = 255", 100),
    ] {
        let directory = tempfile::tempdir()?;
        let legacy = directory.path().join("ghostty.conf");
        let native = directory.path().join("terminal.toml");
        let source = format!("{line}\nfont-size = 19\n");
        std::fs::write(&legacy, &source)?;
        let migrated = TerminalSettings::load_native(&native)?;
        assert_eq!(migrated.options.background_vibrancy, expected, "{line}");
        assert_eq!(TerminalSettings::from_legacy_source(&source)?, migrated);
        assert_eq!(
            TerminalSettings::from_legacy_source(&migrated.legacy_source())?,
            migrated
        );
        assert_eq!(TerminalSettings::load_native(&native)?, migrated);
        assert_eq!(std::fs::read_to_string(legacy)?, source);
    }
    Ok(())
}

#[test]
#[allow(clippy::float_cmp)]
fn every_legacy_option_is_editable_and_survives_saving_and_backup_merges() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("terminal.toml");
    let mut settings = TerminalSettings::load_native(&path)?;
    settings.edit(TerminalEdit::Fallbacks(vec![
        "Fallback".into(),
        "Emoji".into(),
    ]))?;
    settings.set_preference("font-family-bold", "Bold")?;
    settings.set_preference("font-family-italic", "Italic")?;
    settings.set_preference("font-family-bold-italic", "Bold Italic")?;
    settings.set_preference("font-feature", "ss01, -calt, cv01=2,")?;
    settings.set_preference("font-thicken-strength", "128")?;
    settings.edit(TerminalEdit::CodepointMap {
        previous: None,
        map: Some(("U+E000-U+E0FF".into(), "Icons".into())),
    })?;
    settings.edit(TerminalEdit::CodepointMap {
        previous: None,
        map: Some(("U+F000".into(), "Other".into())),
    })?;
    settings.edit(TerminalEdit::CodepointMap {
        previous: Some(FontMap {
            start: 0xe000,
            end: 0xe0ff,
            family: "Icons".into(),
        }),
        map: Some(("U+E000-U+E1FF".into(), "\"Nerd Icons\"".into())),
    })?;
    settings.set_preference("background", "#101010")?;
    settings.set_preference("foreground", "white")?;
    settings.set_preference("cursor-color", "cell-foreground")?;
    settings.set_preference("cursor-text", "#202020")?;
    settings.set_preference("selection-foreground", "#303030")?;
    settings.set_preference("selection-background", "cell-background")?;
    settings.set_preference("palette-1", "#ff0000")?;
    settings.set_preference("palette-2", "#00ff00")?;
    settings.set_preference("palette-2", "")?;
    settings.set_preference("cursor-opacity", "40")?;
    settings.set_preference("window-padding-color", "extend-always")?;
    settings.set_preference("keybind-clear-defaults", "true")?;
    let chord = "ctrl-alt-x".parse()?;
    settings.edit(TerminalEdit::Binding {
        previous: None,
        binding: Some((chord, "text:echo \\e[A\\\\ \\x20".into())),
    })?;
    let renamed = "ctrl-alt-y".parse()?;
    settings.edit(TerminalEdit::Binding {
        previous: Some("ctrl-alt-x".parse()?),
        binding: Some((renamed, "increase_font_size:2".into())),
    })?;
    settings.edit(TerminalEdit::Binding {
        previous: None,
        binding: Some(("cmd-shift-k".parse()?, "text:git checkout ".into())),
    })?;
    settings.save_native(&path)?;

    let restored = TerminalSettings::load_native(&path)?;
    assert_eq!(restored, settings);
    assert_eq!(restored.font_families, ["Menlo", "Fallback", "Emoji"]);
    assert_eq!(restored.font.feature_list(), "ss01, -calt, cv01=2");
    assert_eq!(
        restored.font.codepoints,
        [
            FontMap {
                start: 0xe000,
                end: 0xe1ff,
                family: "Nerd Icons".into(),
            },
            FontMap {
                start: 0xf000,
                end: 0xf000,
                family: "Other".into(),
            },
        ]
    );
    assert_eq!(restored.options.background, Some(0x10_1010));
    assert_eq!(
        restored.options.cursor_color,
        Some(TerminalColor::CellForeground)
    );
    assert_eq!(restored.options.palette.len(), 1);
    assert_eq!(restored.options.cursor_opacity, 0.4);
    assert_eq!(restored.options.padding_color, PaddingColor::ExtendAlways);
    assert!(restored.keybindings.clear_defaults);
    assert_eq!(restored.keybindings.action(&"ctrl-alt-x".parse()?), None);
    assert_eq!(
        restored.keybindings.action(&"ctrl-alt-y".parse()?),
        Some(&TerminalAction::IncreaseFontSize(2.0))
    );
    assert_eq!(
        restored.keybindings.action(&"cmd-shift-k".parse()?),
        Some(&TerminalAction::Text(b"git checkout ".to_vec()))
    );
    let mut merged = TerminalSettings::from_legacy_source(&restored.legacy_source())?;
    merged.diagnostics.clear();
    assert_eq!(merged, restored);
    Ok(())
}

#[test]
fn invalid_or_conflicting_terminal_edits_change_nothing() -> Result {
    let mut settings = TerminalSettings::default();
    settings.edit(TerminalEdit::Binding {
        previous: None,
        binding: Some(("ctrl-alt-x".parse()?, "copy_to_clipboard".into())),
    })?;
    settings.edit(TerminalEdit::CodepointMap {
        previous: None,
        map: Some(("U+E000".into(), "Icons".into())),
    })?;
    let original = settings.clone();
    for (id, value) in [
        ("background", "cell-foreground"),
        ("cursor-color", "#12345"),
        ("palette-256", "#ffffff"),
        ("palette-1", "not a color"),
        ("font-feature", "toolong"),
        ("font-family-bold", "Bad\"Name"),
        ("font-thicken-strength", "256"),
        ("cursor-opacity", "101"),
        ("window-padding-color", "sideways"),
        ("keybind-clear-defaults", "maybe"),
    ] {
        assert!(
            settings.set_preference(id, value).is_err(),
            "{id} = {value}"
        );
        assert_eq!(settings, original);
    }
    for edit in [
        TerminalEdit::Fallbacks(vec!["Bad\"Name".into()]),
        TerminalEdit::CodepointMap {
            previous: None,
            map: Some(("U+F000-U+E000".into(), "Icons".into())),
        },
        TerminalEdit::CodepointMap {
            previous: Some(FontMap {
                start: 1,
                end: 1,
                family: "Gone".into(),
            }),
            map: None,
        },
        TerminalEdit::Binding {
            previous: None,
            binding: Some(("ctrl-alt-z".parse()?, "new_window".into())),
        },
        TerminalEdit::Binding {
            previous: None,
            binding: Some(("ctrl-alt-x".parse()?, "paste_from_clipboard".into())),
        },
        TerminalEdit::Binding {
            previous: Some("ctrl-alt-x".parse()?),
            binding: Some(("ctrl-alt-z".parse()?, "text:\\q".into())),
        },
    ] {
        assert!(settings.edit(edit.clone()).is_err(), "{edit:?}");
        assert_eq!(settings, original);
    }
    Ok(())
}
