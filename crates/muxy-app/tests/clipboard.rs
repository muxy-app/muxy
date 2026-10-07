#[path = "../src/views/terminal/clipboard.rs"]
#[allow(dead_code, reason = "only the path escaping is exercised here")]
mod clipboard;

use muxy_protocol::Modes;

#[test]
fn control_character_paths_round_trip_through_bash_and_zsh_without_terminal_controls()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::path::PathBuf;
    let paths = [
        PathBuf::from("/tmp/normal file"),
        PathBuf::from("/tmp/a\nb\t'\\a$()\r\x1b[201~\x7f\n"),
        PathBuf::from(std::ffi::OsString::from_vec(b"/tmp/\xff\n".to_vec())),
    ];
    let escaped = clipboard::paths(&paths, Modes::default()).ok_or("paths")?;
    assert!(!escaped.iter().any(u8::is_ascii_control));
    assert!(escaped.windows(4).any(|window| window == b"\\x0a"));
    let expected: Vec<u8> = paths
        .iter()
        .flat_map(|path| path.as_os_str().as_bytes().iter().copied().chain([0]))
        .collect();
    for shell in ["/bin/bash", "/bin/zsh"] {
        let command = [b"printf '%s\\0' ".as_slice(), &escaped].concat();
        let output = std::process::Command::new(shell)
            .args(["-f", "-c"])
            .arg(std::ffi::OsString::from_vec(command))
            .output()?;
        assert!(output.status.success(), "{shell}: {:?}", output.stderr);
        assert_eq!(output.stdout, expected, "{shell}");
    }
    assert_eq!(
        clipboard::paths(
            &paths,
            Modes {
                bracketed_paste: true,
                ..Modes::default()
            }
        ),
        Some([b"\x1b[200~".as_slice(), &escaped, b"\x1b[201~"].concat())
    );
    Ok(())
}
