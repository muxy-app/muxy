type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn command_arguments_preserve_quoting_without_shell_expansion() -> Result {
    let arguments = muxy_core::executable::command_arguments(
        r#"'/Applications/VM Tool/bin/vm' ssh "dev vm" 'it'\''s mine' escaped\ space '' "$HOME" '*.txt' ';' '$(touch nope)' "a\qb""#,
    )?;
    assert_eq!(
        arguments,
        [
            "/Applications/VM Tool/bin/vm",
            "ssh",
            "dev vm",
            "it's mine",
            "escaped space",
            "",
            "$HOME",
            "*.txt",
            ";",
            "$(touch nope)",
            "a\\qb",
        ]
    );
    Ok(())
}

#[test]
fn malformed_commands_cannot_turn_into_a_different_invocation() {
    for source in [
        "", "  ", "'' ssh", "vm 'box", "vm \"box", "vm box\\", "vm\nssh", "vm\0ssh",
    ] {
        assert_eq!(
            muxy_core::executable::command_arguments(source)
                .err()
                .map(|error| error.kind()),
            Some(std::io::ErrorKind::InvalidInput),
            "{source:?}",
        );
    }
}
