use std::error::Error;
use std::process::{Command, Output};

fn muxy(arguments: &[&str]) -> Result<Output, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let output = Command::new(env!("CARGO_BIN_EXE_muxy"))
        .args(arguments)
        .env("MUXY_DIR", directory.path())
        .env("NO_COLOR", "1")
        .output()?;
    assert!(!directory.path().join("server.sock").exists());
    Ok(output)
}

#[test]
fn help_is_grouped_specific_and_plain_when_piped() -> Result<(), Box<dyn Error>> {
    let root = muxy(&["--help"])?;
    assert!(root.status.success());
    assert!(root.stderr.is_empty());
    let root = String::from_utf8(root.stdout)?;
    for heading in ["Common commands:", "Resources:", "Options:", "Examples:"] {
        assert!(root.contains(heading));
    }
    assert!(!root.contains('\u{1b}'));
    let by_name = muxy(&["help", "session", "send"])?;
    let by_flag = muxy(&["session", "send", "--help"])?;
    assert!(by_name.status.success());
    assert_eq!(by_name.stdout, by_flag.stdout);
    assert!(String::from_utf8(by_name.stdout)?.contains("--stdin"));
    assert!(
        muxy(&["help", "session", "not-a-command"])?
            .status
            .code()
            .is_some_and(|code| code != 0)
    );
    Ok(())
}

#[test]
fn completions_and_version_work_offline() -> Result<(), Box<dyn Error>> {
    for shell in ["bash", "zsh", "fish"] {
        let result = muxy(&["completion", shell])?;
        assert!(result.status.success());
        assert!(String::from_utf8(result.stdout)?.contains("muxy __complete"));
    }
    let result = muxy(&["__complete", "session", "in"])?;
    assert!(result.status.success());
    assert_eq!(result.stdout, b"inspect\n");
    assert_eq!(muxy(&["version"])?.stdout, muxy(&["--version"])?.stdout);
    Ok(())
}

#[test]
fn bash_completion_script_supplies_candidates() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let script = muxy(&["completion", "bash"])?;
    let path = directory.path().join("muxy.bash");
    std::fs::write(&path, script.stdout)?;
    let binary = std::path::Path::new(env!("CARGO_BIN_EXE_muxy"));
    let mut paths = vec![binary.parent().ok_or("binary directory")?.to_path_buf()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let output = Command::new("bash")
        .args(["--noprofile", "--norc", "-c", "source \"$1\"; COMP_WORDS=(muxy session in); COMP_CWORD=2; _muxy_complete; printf '%s\\n' \"${COMPREPLY[@]}\"", "bash"])
        .arg(path)
        .env("PATH", std::env::join_paths(paths)?)
        .env("MUXY_DIR", directory.path())
        .output()?;
    assert!(output.status.success());
    assert_eq!(output.stdout, b"inspect\n");
    Ok(())
}

#[test]
fn invalid_arguments_fail_before_connecting() -> Result<(), Box<dyn Error>> {
    for arguments in [
        vec!["session", "rename", "42", ""],
        vec!["session", "send", "42", "text", "--stdin"],
        vec!["session", "send-keys", "42", "Enter", "unknown"],
        vec!["ls", "--quiet", "--json"],
    ] {
        let result = muxy(&arguments)?;
        assert!(!result.status.success(), "{arguments:?}");
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8(result.stderr)?.starts_with("muxy:"));
    }
    Ok(())
}
