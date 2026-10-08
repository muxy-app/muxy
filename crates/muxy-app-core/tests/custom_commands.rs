#![allow(clippy::unwrap_used)]

use muxy_app_core::settings::{CustomCommand, Settings};

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
