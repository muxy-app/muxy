#![allow(clippy::unwrap_used)]

use muxy_app_core::settings::{CustomCommand, Keymap, Settings};
use muxy_core::shortcuts::ShortcutSettings;

fn command() -> CustomCommand {
    CustomCommand::new("Tests".into(), "cargo test && echo 'done'".into())
}

#[test]
fn custom_commands_roundtrip_and_delete_without_losing_other_settings() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.toml");
    std::fs::write(&path, "[clipboard]\ncopy_on_select = true\n").unwrap();
    let mut settings = Settings::load(&path).unwrap();
    assert!(settings.commands.is_empty());
    let command = command();
    let id = command.shortcut_id();
    settings.commands.push(command.clone());
    settings.keymap = settings
        .keymap
        .with_binding(&id, Some("cmd-alt-y".parse().unwrap()))
        .unwrap();
    settings.save_commands(&path).unwrap();
    let loaded = Settings::load(&path).unwrap();
    assert_eq!(loaded.commands, vec![command]);
    assert!(loaded.clipboard.copy_on_select);
    assert_eq!(
        loaded.keymap.keys(&id, Some("WorkspaceTabs")),
        ["cmd-alt-y"]
    );
    settings.commands.clear();
    settings.keymap = settings.keymap.with_binding(&id, None).unwrap();
    settings.save_commands(&path).unwrap();
    let loaded = Settings::load(&path).unwrap();
    assert!(loaded.commands.is_empty());
    assert!(loaded.keymap.binding(&id).is_none());
    assert!(loaded.clipboard.copy_on_select);
}

#[test]
fn custom_commands_reject_invalid_definitions_without_writing() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.toml");
    std::fs::write(&path, "[clipboard]\ncopy_on_select = true\n").unwrap();
    let before = std::fs::read(&path).unwrap();
    for text in [
        "",
        "   ",
        "echo a\necho b",
        "echo a\recho b",
        "echo\0",
        "echo\x1b[A",
    ] {
        let mut settings = Settings::default();
        let mut command = command();
        command.command = text.into();
        settings.commands.push(command);
        assert!(settings.save_commands(&path).is_err(), "{text:?}");
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    let mut command = command();
    command.name = " ".into();
    assert!(command.validate().is_err());
    command.name = "Tests".into();
    command.id = "bad.id".into();
    assert!(command.validate().is_err());
    command.id = "tests".into();
    command.command = "x".repeat(64 * 1024);
    assert!(command.validate().is_err());
    let command = self::command();
    let settings = Settings {
        commands: vec![command.clone(), command],
        ..Settings::default()
    };
    assert!(settings.validate().is_err());
}

#[test]
fn custom_shortcuts_require_modifiers_and_reject_conflicts_in_both_directions() {
    let keymap = Keymap::default();
    for key in [
        "y",
        "shift-y",
        "enter",
        "cmd-t",
        "cmd-g",
        "ctrl-shift-tab",
        "cmd-c",
    ] {
        assert!(
            keymap
                .with_binding("command.tests", Some(key.parse().unwrap()))
                .is_err(),
            "{key}"
        );
    }
    let keymap = keymap
        .with_binding("command.tests", Some("cmd-alt-y".parse().unwrap()))
        .unwrap();
    for id in ["command.other", "new_tab", "extension.test.run"] {
        assert!(
            keymap
                .with_binding(id, Some("cmd-alt-y".parse().unwrap()))
                .is_err(),
            "{id}"
        );
    }
    assert!(
        keymap
            .with_binding("command.", Some("cmd-alt-u".parse().unwrap()))
            .is_err()
    );
    assert!(
        keymap
            .with_binding("command.bad.id", Some("cmd-alt-u".parse().unwrap()))
            .is_err()
    );
    let settings = "[keymap]\n'command.tests' = 'shift-y'\n";
    assert!(toml::from_str::<Settings>(settings).is_err());
}

#[test]
fn custom_commands_reject_quick_terminal_conflicts_in_settings_files() {
    use muxy_core::quick_terminal::{
        QuickTerminalShortcut,
        keys::{CONTROL, KeyCombo, OPTION},
    };
    let mut settings = Settings::default();
    let command = command();
    settings.keymap = settings
        .keymap
        .with_binding(&command.shortcut_id(), Some("ctrl-alt-y".parse().unwrap()))
        .unwrap();
    settings.commands.push(command);
    settings.quick_terminal.shortcut = QuickTerminalShortcut::KeyCombo {
        key_combo: KeyCombo::new("y", CONTROL | OPTION),
        virtual_key_code: 16,
    };
    assert!(settings.validate().is_err());
}

#[test]
fn custom_commands_loaded_from_disk_reject_explicit_terminal_shortcut_conflicts() {
    use muxy_app_core::settings::TerminalSettings;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.toml");
    let terminal_path = directory.path().join("ghostty.conf");
    let mut settings = Settings::default();
    let command = command();
    settings.keymap = settings
        .keymap
        .with_binding(&command.shortcut_id(), Some("cmd-alt-y".parse().unwrap()))
        .unwrap();
    settings.commands.push(command);
    settings.save_commands(&path).unwrap();
    std::fs::write(
        directory.path().join("bindings.conf"),
        "keybind = cmd+alt+y=ignore\n",
    )
    .unwrap();
    for config in [
        "keybind = cmd+alt+y=ignore\n",
        "keybind = cmd+alt+y=text:hello\n",
        "keybind = cmd+alt+y=unbind\n",
        "config-file = bindings.conf\n",
    ] {
        std::fs::write(&terminal_path, config).unwrap();
        let settings = Settings::load(&path).unwrap();
        let terminal = TerminalSettings::load_with_seed(&terminal_path, None).unwrap();
        let error = settings
            .validate_command_shortcuts(&terminal)
            .unwrap_err()
            .to_string();
        assert!(error.contains("cmd-alt-y"), "{error}");
        assert!(error.contains("Tests"), "{error}");
        assert!(error.contains("ghostty.conf"), "{error}");
    }
    std::fs::write(&terminal_path, "keybind = cmd+alt+u=ignore\n").unwrap();
    let terminal = TerminalSettings::load_with_seed(&terminal_path, None).unwrap();
    assert!(settings.validate_command_shortcuts(&terminal).is_ok());
}
