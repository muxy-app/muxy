use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

pub fn command_arguments(source: &str) -> io::Result<Vec<String>> {
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidInput, message);
    if source.chars().any(char::is_control) {
        return Err(invalid("Commands must not contain control characters"));
    }
    let mut arguments = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote = None;
    let mut characters = source.chars();
    while let Some(character) = characters.next() {
        match (quote, character) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (None, '\'' | '"') => {
                quote = Some(character);
                started = true;
            }
            (None | Some('"'), '\\') => {
                let escaped = characters
                    .next()
                    .ok_or_else(|| invalid("Commands must not end with an unescaped backslash"))?;
                if quote == Some('"') && !matches!(escaped, '"' | '\\' | '$' | '`') {
                    word.push('\\');
                }
                word.push(escaped);
                started = true;
            }
            (None, character) if character.is_whitespace() => {
                if started {
                    arguments.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            _ => {
                word.push(character);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return Err(invalid("Close the quote in the command"));
    }
    if started {
        arguments.push(word);
    }
    if arguments.first().is_none_or(String::is_empty) {
        return Err(invalid("Enter a command with a non-empty program name"));
    }
    Ok(arguments)
}

pub fn current_path() -> io::Result<std::path::PathBuf> {
    // macOS current_exe can retain the launch symlink after its target changes.
    #[cfg(target_os = "macos")]
    let path = std::path::PathBuf::from(
        libproc::libproc::proc_pid::pidpath(
            i32::try_from(std::process::id()).map_err(io::Error::other)?,
        )
        .map_err(io::Error::other)?,
    );
    #[cfg(not(target_os = "macos"))]
    let path = std::env::current_exe()?;
    path.canonicalize()
}

pub fn build_metadata(executable: &Path) -> io::Result<Vec<u8>> {
    let output = crate::process::run(
        Command::new(executable).arg("--build-info"),
        crate::process::Limits {
            stderr: crate::process::Cap::TruncateAt(0),
            ..crate::process::Limits::new(Duration::from_secs(3), 4096)
        },
    )?;
    if output.timed_out {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Server build metadata timed out",
        ));
    }
    if output.stdout.truncated {
        return Err(io::Error::other("Executable build metadata is too large"));
    }
    if !output.status.success() {
        return Err(io::Error::other("Server build metadata is unavailable"));
    }
    Ok(output.stdout.bytes)
}
