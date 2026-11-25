use std::path::PathBuf;

use muxy_protocol::{
    ForegroundProcess, Modes, ServerPath, validate_absolute_path, validate_relative_path,
};

pub fn paste(text: &str, modes: Modes) -> Vec<u8> {
    let text = text.replace("\r\n", "\n").replace('\n', "\r");
    let text = text.replace(['\x1b', '\u{009b}'], "");
    bracket(text.into_bytes(), modes)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathQuoting {
    Bash,
    Posix,
    Fish,
    PowerShell,
    Cmd,
    Program,
    Unknown,
}

impl PathQuoting {
    pub fn for_process(process: Option<&ForegroundProcess>) -> Option<Self> {
        let Some(process) = process else {
            return Some(Self::Unknown);
        };
        let name = process.name.trim_start_matches('-').to_ascii_lowercase();
        let name = name.strip_suffix(".exe").unwrap_or(&name);
        Some(match name {
            "bash" | "zsh" | "ksh" | "ksh93" | "mksh" => Self::Bash,
            "sh" | "dash" | "ash" => Self::Posix,
            "fish" => Self::Fish,
            "pwsh" | "powershell" => Self::PowerShell,
            "cmd" => Self::Cmd,
            "" => Self::Unknown,
            "csh" | "tcsh" | "nu" | "xonsh" | "elvish" | "ion" | "rc" | "es" | "yash" | "osh"
            | "ysh" | "oil" | "murex" | "nushell" | "ssh" | "mosh" | "mosh-client" | "et"
            | "telnet" | "tmux" | "screen" | "zellij" | "docker" | "podman" | "kubectl" | "wsl"
            | "sudo" | "su" | "doas" => return None,
            _ if process.is_shell => return None,
            _ => Self::Program,
        })
    }
}

pub fn paths(paths: &[PathBuf], modes: Modes) -> Option<Vec<u8>> {
    quoted_paths(paths, PathQuoting::Unknown, modes)
}

pub fn quoted_paths(paths: &[PathBuf], quoting: PathQuoting, modes: Modes) -> Option<Vec<u8>> {
    let mut text = Vec::new();
    for path in paths {
        let path = ServerPath::from(path);
        validate_absolute_path(&path).ok()?;
        if !text.is_empty() {
            text.push(b' ');
        }
        text.extend(quote_path(&path.0, quoting)?);
    }
    Some(bracket(text, modes))
}

pub fn typed_path(
    path: &ServerPath,
    quoting: Option<PathQuoting>,
    modes: Modes,
) -> Option<Vec<u8>> {
    if path.0.is_empty() {
        return None;
    }
    validate_absolute_path(path)
        .or_else(|_| validate_relative_path(path))
        .ok()?;
    let windows_shell = matches!(quoting, Some(PathQuoting::PowerShell | PathQuoting::Cmd));
    let mut word = path.0.clone();
    if word.starts_with(b"-") {
        word.splice(0..0, if windows_shell { *b".\\" } else { *b"./" });
    }
    if !bare(&word, quoting) {
        let quoting = quoting?;
        if quoting == PathQuoting::Unknown
            && !word.starts_with(b"/")
            && word.iter().any(|byte| b"%!\"^&|<>".contains(byte))
        {
            return None;
        }
        let windows_quotes = windows_shell
            || (matches!(quoting, PathQuoting::Program | PathQuoting::Unknown)
                && windows_path(&word));
        if windows_quotes && word.ends_with(b"\\") && !word.ends_with(b":\\") && word.len() > 1 {
            word.pop();
        }
        word = quote_path(&word, quoting)?;
    }
    Some(bracket(word, modes))
}

fn bare(bytes: &[u8], quoting: Option<PathQuoting>) -> bool {
    let windows = matches!(quoting, Some(PathQuoting::PowerShell | PathQuoting::Cmd));
    bytes.iter().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b'_' | b'.' | b'/' | b'-')
            || (windows && matches!(byte, b'\\' | b':'))
    })
}

fn quote_path(bytes: &[u8], quoting: PathQuoting) -> Option<Vec<u8>> {
    match quoting {
        PathQuoting::Program if windows_path(bytes) => powershell(std::str::from_utf8(bytes).ok()?),
        PathQuoting::Bash | PathQuoting::Program if bytes.iter().any(u8::is_ascii_control) => {
            Some(ansi_c(bytes))
        }
        PathQuoting::Bash | PathQuoting::Posix | PathQuoting::Program => posix(bytes, false),
        PathQuoting::Fish => posix(bytes, true),
        PathQuoting::PowerShell => powershell(std::str::from_utf8(bytes).ok()?),
        PathQuoting::Cmd => cmd(std::str::from_utf8(bytes).ok()?),
        PathQuoting::Unknown if windows_path(bytes) => {
            let text = std::str::from_utf8(bytes).ok()?;
            if text.contains(['%', '!', '"', '^', '&', '|', '<', '>']) {
                return None;
            }
            powershell(text)
        }
        PathQuoting::Unknown => {
            let text = std::str::from_utf8(bytes).ok()?;
            if text.contains(CURLY_SINGLE_QUOTES) || text.contains(['\'', '\\']) {
                return None;
            }
            posix(bytes, false)
        }
    }
}

fn windows_path(bytes: &[u8]) -> bool {
    match bytes {
        [drive, b':', b'\\' | b'/', ..] => drive.is_ascii_alphabetic(),
        [b'\\', b'\\', ..] => true,
        _ => false,
    }
}

const CURLY_SINGLE_QUOTES: [char; 4] = ['\u{2018}', '\u{2019}', '\u{201a}', '\u{201b}'];

fn ansi_c(bytes: &[u8]) -> Vec<u8> {
    let mut text = b"$'".to_vec();
    for &byte in bytes {
        match byte {
            byte if byte.is_ascii_control() => {
                let hex = b"0123456789abcdef";
                text.extend_from_slice(&[
                    b'\\',
                    b'x',
                    hex[usize::from(byte >> 4)],
                    hex[usize::from(byte & 15)],
                ]);
            }
            b'\'' | b'\\' => text.extend_from_slice(&[b'\\', byte]),
            byte => text.push(byte),
        }
    }
    text.push(b'\'');
    text
}

fn posix(bytes: &[u8], backslashes_outside: bool) -> Option<Vec<u8>> {
    if bytes.iter().any(u8::is_ascii_control) {
        return None;
    }
    let mut text = b"'".to_vec();
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        let curly = byte == 0xe2
            && bytes.get(index + 1) == Some(&0x80)
            && bytes
                .get(index + 2)
                .is_some_and(|last| (0x98..=0x9b).contains(last));
        if curly {
            text.extend_from_slice(b"'\"");
            text.extend_from_slice(&bytes[index..index + 3]);
            text.extend_from_slice(b"\"'");
            index += 3;
            continue;
        }
        match byte {
            b'\'' => text.extend_from_slice(b"'\\''"),
            b'\\' if backslashes_outside => text.extend_from_slice(b"'\\\\'"),
            byte => text.push(byte),
        }
        index += 1;
    }
    text.push(b'\'');
    Some(text)
}

fn powershell(text: &str) -> Option<Vec<u8>> {
    if text.chars().any(char::is_control) {
        return None;
    }
    let mut word = String::from("'");
    for character in text.chars() {
        if character == '\'' || CURLY_SINGLE_QUOTES.contains(&character) {
            word.push(character);
        }
        word.push(character);
    }
    word.push('\'');
    Some(word.into_bytes())
}

fn cmd(text: &str) -> Option<Vec<u8>> {
    if text.chars().any(char::is_control) || text.contains(['%', '!', '"']) {
        return None;
    }
    Some(format!("\"{text}\"").into_bytes())
}

fn bracket(text: Vec<u8>, modes: Modes) -> Vec<u8> {
    if modes.bracketed_paste && !text.is_empty() {
        [b"\x1b[200~".as_slice(), &text, b"\x1b[201~"].concat()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modes(bracketed_paste: bool) -> Modes {
        Modes {
            bracketed_paste,
            ..Modes::default()
        }
    }

    fn process(name: &str, is_shell: bool) -> ForegroundProcess {
        ForegroundProcess {
            name: name.to_owned(),
            is_shell,
        }
    }

    fn quoted(path: &str, quoting: PathQuoting) -> Option<String> {
        quoted_paths(&[PathBuf::from(path)], quoting, modes(false))
            .map(|bytes| String::from_utf8(bytes).unwrap())
    }

    #[test]
    fn paste_normalizes_newlines_and_removes_embedded_escape_sequences() {
        let text = "first\r\nsecond\n\x1b[201~\rthird\u{009b}201~界\t";
        for bracketed_paste in [false, true] {
            let bytes = paste(text, modes(bracketed_paste));
            let expected = "first\rsecond\r[201~\rthird201~界\t";
            let expected = if bracketed_paste {
                format!("\x1b[200~{expected}\x1b[201~")
            } else {
                expected.to_owned()
            };
            assert_eq!(bytes, expected.as_bytes());
        }
        assert!(paste("", modes(true)).is_empty());
    }

    fn typed(path: &str, quoting: Option<PathQuoting>) -> Option<String> {
        typed_path(&ServerPath::from(path), quoting, modes(false))
            .map(|bytes| String::from_utf8(bytes).unwrap())
    }

    #[test]
    fn typed_paths_stay_bare_when_every_program_reads_them_the_same() {
        for quoting in [
            None,
            Some(PathQuoting::Unknown),
            Some(PathQuoting::Bash),
            Some(PathQuoting::Fish),
            Some(PathQuoting::Cmd),
        ] {
            for path in [
                "crates/muxy-app/Cargo.toml",
                "docs/",
                "/usr/local/bin",
                "a_b.c-d",
            ] {
                assert_eq!(typed(path, quoting).as_deref(), Some(path), "{quoting:?}");
            }
        }
        assert_eq!(
            typed_path(&ServerPath::from("docs/"), None, modes(true)),
            Some(b"\x1b[200~docs/\x1b[201~".to_vec())
        );
    }

    #[test]
    fn typed_paths_that_need_quotes_follow_the_program() {
        assert_eq!(
            typed("my file.txt", Some(PathQuoting::Bash)).as_deref(),
            Some("'my file.txt'")
        );
        assert_eq!(
            typed("it's/a b", Some(PathQuoting::Bash)).as_deref(),
            Some("'it'\\''s/a b'")
        );
        assert_eq!(
            typed("$(reboot)", Some(PathQuoting::Fish)).as_deref(),
            Some("'$(reboot)'")
        );
        assert_eq!(
            typed("~user", Some(PathQuoting::Posix)).as_deref(),
            Some("'~user'"),
            "a name is never read as a home folder"
        );
        assert_eq!(
            typed("my file.txt", Some(PathQuoting::Cmd)).as_deref(),
            Some("\"my file.txt\"")
        );
        assert_eq!(
            typed("my file.txt", None),
            None,
            "an unknown grammar gets nothing"
        );
        assert_eq!(typed("it's", Some(PathQuoting::Unknown)), None);
        assert_eq!(typed("100%", Some(PathQuoting::Cmd)), None);
    }

    #[test]
    fn typed_paths_are_never_read_as_options_or_other_text() {
        assert_eq!(typed("-rf", None).as_deref(), Some("./-rf"));
        assert_eq!(
            typed("-r f", Some(PathQuoting::Bash)).as_deref(),
            Some("'./-r f'")
        );
        assert_eq!(typed("", Some(PathQuoting::Bash)), None);
        assert_eq!(typed("a\0b", Some(PathQuoting::Bash)), None);
        assert_eq!(typed("a\nb", Some(PathQuoting::Posix)), None);
        assert_eq!(
            typed("a\nb", Some(PathQuoting::Bash)).as_deref(),
            Some("$'a\\x0ab'")
        );
        assert_eq!(typed(&"a".repeat(5000), None), None);
    }

    #[test]
    fn typed_windows_paths_keep_backslashes_only_in_windows_shells() {
        assert_eq!(
            typed("src\\main.rs", Some(PathQuoting::PowerShell)).as_deref(),
            Some("src\\main.rs")
        );
        assert_eq!(
            typed("C:\\Users\\dev", Some(PathQuoting::Cmd)).as_deref(),
            Some("C:\\Users\\dev")
        );
        assert_eq!(
            typed("src\\main.rs", Some(PathQuoting::Bash)).as_deref(),
            Some("'src\\main.rs'")
        );
        assert_eq!(typed("src\\main.rs", None), None);
    }

    #[test]
    fn typed_windows_folders_never_end_in_a_backslash_before_a_quote() {
        assert_eq!(
            typed("my dir\\", Some(PathQuoting::Cmd)).as_deref(),
            Some("\"my dir\"")
        );
        assert_eq!(
            typed("C:\\Users\\my dir\\", Some(PathQuoting::PowerShell)).as_deref(),
            Some("'C:\\Users\\my dir'")
        );
        assert_eq!(
            typed("C:\\my dir\\", Some(PathQuoting::Program)).as_deref(),
            Some("'C:\\my dir'")
        );
        assert_eq!(
            typed("src\\", Some(PathQuoting::Cmd)).as_deref(),
            Some("src\\"),
            "a bare folder keeps it"
        );
        assert_eq!(
            typed("a b\\", Some(PathQuoting::Bash)).as_deref(),
            Some("'a b\\'"),
            "elsewhere a backslash is part of the name"
        );
        assert_eq!(
            typed("-x", Some(PathQuoting::Cmd)).as_deref(),
            Some(".\\-x"),
            "cmd reads a slash as a switch"
        );
        for quoting in [PathQuoting::Bash, PathQuoting::Posix] {
            assert_eq!(
                typed("C:\\notes\\", Some(quoting)).as_deref(),
                Some("'C:\\notes\\'"),
                "a POSIX file may be named like a Windows folder"
            );
        }
    }

    #[test]
    fn an_unknown_program_gets_no_relative_path_cmd_would_run() {
        let unknown = Some(PathQuoting::Unknown);
        for name in ["a&whoami&rem x", "a|b", "50% off", "a>b", "wow!", "a^b c"] {
            assert_eq!(typed(name, unknown), None, "{name}");
        }
        assert_eq!(typed("a b", unknown).as_deref(), Some("'a b'"));
        assert_eq!(
            typed("/tmp/a&b", unknown).as_deref(),
            Some("'/tmp/a&b'"),
            "a path from / is not on a computer that runs cmd"
        );
        assert_eq!(
            typed("a&b", Some(PathQuoting::Bash)).as_deref(),
            Some("'a&b'"),
            "a known shell quotes them"
        );
    }

    #[test]
    fn quoting_follows_the_program_in_the_foreground() {
        let of = |name, is_shell| PathQuoting::for_process(Some(&process(name, is_shell)));
        assert_eq!(PathQuoting::for_process(None), Some(PathQuoting::Unknown));
        assert_eq!(of("-zsh", true), Some(PathQuoting::Bash));
        assert_eq!(of("bash", false), Some(PathQuoting::Bash));
        assert_eq!(of("fish", true), Some(PathQuoting::Fish));
        assert_eq!(of("dash", true), Some(PathQuoting::Posix));
        assert_eq!(of("pwsh", true), Some(PathQuoting::PowerShell));
        assert_eq!(of("PowerShell.exe", true), Some(PathQuoting::PowerShell));
        assert_eq!(of("cmd", true), Some(PathQuoting::Cmd));
        assert_eq!(of("", true), Some(PathQuoting::Unknown));
        assert_eq!(of("tcsh", true), None, "an unknown grammar gets nothing");
        assert_eq!(of("nu", false), None, "even when another shell started it");
        assert_eq!(of("someshell", true), None);
        assert_eq!(of("claude", false), Some(PathQuoting::Program));
        assert_eq!(of("ssh", false), None, "a shell elsewhere could be any");
        assert_eq!(of("tmux", false), None);
        assert_eq!(of("claude.exe", false), Some(PathQuoting::Program));
    }

    #[test]
    fn windows_server_paths_keep_their_backslashes_from_any_client() {
        assert_eq!(
            quoted(r"C:\Users\me\it's.png", PathQuoting::Program).as_deref(),
            Some(r"'C:\Users\me\it''s.png'")
        );
        assert_eq!(
            quoted(r"C:\Users\me\a b.png", PathQuoting::Unknown).as_deref(),
            Some(r"'C:\Users\me\a b.png'")
        );
        assert_eq!(
            quoted(r"\\host\share\a.png", PathQuoting::Unknown).as_deref(),
            Some(r"'\\host\share\a.png'")
        );
        assert_eq!(
            paths(&[PathBuf::from(r"C:\Users\me\img.png")], modes(false)).as_deref(),
            Some(r"'C:\Users\me\img.png'".as_bytes())
        );
        assert_eq!(quoted(r"C:\a&b", PathQuoting::Unknown), None);
        assert_eq!(
            quoted(r"C:\a&b", PathQuoting::Program).as_deref(),
            Some(r"'C:\a&b'")
        );
        assert_eq!(
            quoted("/tmp/it's", PathQuoting::Program).as_deref(),
            Some(r"'/tmp/it'\''s'")
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn paths_quote_for_each_unix_shell() {
        let dropped = [PathBuf::from("/tmp/it's"), PathBuf::from("/tmp/a\x1bb")];
        assert_eq!(
            quoted_paths(&dropped, PathQuoting::Bash, modes(false)).as_deref(),
            Some(b"'/tmp/it'\\''s' $'/tmp/a\\x1bb'".as_slice())
        );
        assert_eq!(
            quoted_paths(&dropped, PathQuoting::Posix, modes(false)),
            None,
            "only bash, zsh and ksh read $'…'"
        );
        assert_eq!(
            quoted(r"/tmp/x\' ; touch pwned #", PathQuoting::Fish).as_deref(),
            Some(r"'/tmp/x'\\''\'' ; touch pwned #'")
        );
        assert_eq!(
            quoted(r"/tmp/x\' ; touch pwned #", PathQuoting::Posix).as_deref(),
            Some(r"'/tmp/x\'\'' ; touch pwned #'")
        );
        assert_eq!(
            quoted("/tmp/it\u{2019}s", PathQuoting::Posix).as_deref(),
            Some("'/tmp/it'\"\u{2019}\"'s'")
        );
        assert_eq!(
            quoted("/tmp/it\u{2019}s; x", PathQuoting::PowerShell).as_deref(),
            Some("'/tmp/it\u{2019}\u{2019}s; x'")
        );
        assert_eq!(quoted("/tmp/it\u{2019}s", PathQuoting::Unknown), None);
        assert_eq!(quoted("/tmp/a\x1bb", PathQuoting::Unknown), None);
        assert_eq!(
            paths(&[PathBuf::from("/tmp/a b")], modes(false)).as_deref(),
            Some(b"'/tmp/a b'".as_slice())
        );
        assert_eq!(paths(&[PathBuf::from("/tmp/it's; x")], modes(false)), None);
        assert_eq!(paths(&[PathBuf::from(r"/tmp/x\")], modes(false)), None);
        assert_eq!(paths(&[PathBuf::from("relative")], modes(false)), None);
    }

    #[cfg(windows)]
    #[test]
    fn paths_quote_for_powershell_and_cmd() {
        let dropped = [
            PathBuf::from(r"C:\Users\me\it's"),
            PathBuf::from("C:\\Users\\me\\\u{2019}x y"),
        ];
        assert_eq!(
            paths(&dropped, modes(false)).as_deref(),
            Some("'C:\\Users\\me\\it''s' 'C:\\Users\\me\\\u{2019}\u{2019}x y'".as_bytes())
        );
        assert_eq!(
            paths(&[PathBuf::from(r"C:\a\x1bb")], modes(false)),
            Some(b"'C:\\a\\x1bb'".to_vec())
        );
        assert_eq!(paths(&[PathBuf::from("C:\\a\x1bb")], modes(false)), None);
        assert_eq!(paths(&[PathBuf::from("relative")], modes(false)), None);
        assert_eq!(quoted(r"C:\a&b", PathQuoting::Unknown), None);
        assert_eq!(
            quoted(r"C:\a&b", PathQuoting::PowerShell).as_deref(),
            Some(r"'C:\a&b'")
        );
        assert_eq!(
            quoted(r"C:\a&b it's", PathQuoting::Cmd).as_deref(),
            Some(r#""C:\a&b it's""#)
        );
        assert_eq!(quoted(r"C:\100%", PathQuoting::Cmd), None);
    }
}
