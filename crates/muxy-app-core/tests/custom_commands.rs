#![allow(clippy::unwrap_used)]

use muxy_app_core::settings::{CustomCommand, Keymap, Settings};

fn command() -> CustomCommand {
    CustomCommand::new("Tests".into(), "cargo test && echo 'done'".into())
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
