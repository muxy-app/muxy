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

#[test]
fn the_current_executable_has_a_plain_canonical_path() -> Result {
    let path = muxy_core::executable::current_path()?;
    assert!(path.is_absolute());
    assert!(!path.to_string_lossy().starts_with(r"\\?\"));
    assert!(muxy_core::files::same_file(
        &path,
        &std::env::current_exe()?
    )?);
    Ok(())
}

#[cfg(windows)]
#[test]
fn programs_are_found_by_their_pathext_extensions() -> Result {
    use muxy_core::executable::find_with_extensions;
    let first = tempfile::tempdir()?;
    let second = tempfile::tempdir()?;
    std::fs::write(first.path().join("code"), "")?;
    std::fs::write(second.path().join("code.cmd"), "")?;
    std::fs::write(second.path().join("zed.ps1"), "")?;
    let directories = [first.path().to_owned(), second.path().to_owned()];
    assert_eq!(
        find_with_extensions(&directories, "code", ".COM;.EXE;.BAT;.CMD"),
        Some(second.path().join("code.cmd")),
        "a file without an extension doesn't run"
    );
    assert_eq!(
        find_with_extensions(&directories, "code", ""),
        Some(second.path().join("code.cmd")),
        "an empty PATHEXT falls back to the usual extensions"
    );
    assert_eq!(find_with_extensions(&directories, "zed", ".EXE;.CMD"), None);
    Ok(())
}
