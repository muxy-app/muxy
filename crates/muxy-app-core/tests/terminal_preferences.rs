use muxy_app_core::settings::{CellHeight, TerminalSettings};

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
