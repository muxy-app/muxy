const SHELL_SAFE_EXTRA: &str = "-_./:@%+,";

pub fn shell_escape(value: &str) -> String {
    let safe = value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || SHELL_SAFE_EXTRA.contains(character));
    if safe {
        return value.to_owned();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn user_shell() -> String {
    std::env::var("SHELL")
        .ok()
        .filter(|shell| !shell.is_empty())
        .unwrap_or_else(|| "/bin/zsh".to_owned())
}

pub fn startup_shell_command(shell: &str, keeps_shell_open: bool) -> String {
    let escaped = shell_escape(shell);
    let script = if is_fish_shell(shell) {
        fish_script(keeps_shell_open)
    } else {
        posix_script(keeps_shell_open)
    };
    let compatibility = if shell.rsplit('/').next() == Some("bash") {
        " --posix"
    } else {
        ""
    };
    format!("{escaped}{compatibility} -l -i -c '{script}' {escaped}")
}

fn is_fish_shell(shell: &str) -> bool {
    shell.rsplit('/').next() == Some("fish")
}

fn posix_script(keeps_shell_open: bool) -> String {
    let tail = if keeps_shell_open {
        "else exec \"$0\" -l"
    } else {
        "else exit $muxy_status"
    };
    format!(
        "eval \"$MUXY_STARTUP_COMMAND\"; muxy_status=$?; if [ $muxy_status -ne 0 ]; \
         then exec \"$0\" -l; {tail}; fi"
    )
}

fn fish_script(keeps_shell_open: bool) -> String {
    let tail = if keeps_shell_open {
        "else exec \"$argv[1]\" -l"
    } else {
        "else exit $muxy_status"
    };
    format!(
        "eval \"$MUXY_STARTUP_COMMAND\"; set muxy_status $status; if test $muxy_status -ne 0; \
         exec \"$argv[1]\" -l; {tail}; end"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_only_paths_with_unsafe_characters() {
        assert_eq!(shell_escape("/bin/zsh"), "/bin/zsh");
        assert_eq!(shell_escape("/opt/my-shell_1.0"), "/opt/my-shell_1.0");
        assert_eq!(shell_escape("/bin/my shell"), "'/bin/my shell'");
        assert_eq!(shell_escape("/bin/it's"), "'/bin/it'\\''s'");
    }

    #[test]
    fn builds_the_posix_startup_command_for_both_exit_policies() {
        assert_eq!(
            startup_shell_command("/bin/zsh", true),
            "/bin/zsh -l -i -c 'eval \"$MUXY_STARTUP_COMMAND\"; muxy_status=$?; \
             if [ $muxy_status -ne 0 ]; then exec \"$0\" -l; else exec \"$0\" -l; fi' /bin/zsh"
        );
        assert_eq!(
            startup_shell_command("/bin/zsh", false),
            "/bin/zsh -l -i -c 'eval \"$MUXY_STARTUP_COMMAND\"; muxy_status=$?; \
             if [ $muxy_status -ne 0 ]; then exec \"$0\" -l; else exit $muxy_status; fi' /bin/zsh"
        );
    }

    #[test]
    fn builds_the_fish_startup_command() {
        assert_eq!(
            startup_shell_command("/opt/homebrew/bin/fish", false),
            "/opt/homebrew/bin/fish -l -i -c 'eval \"$MUXY_STARTUP_COMMAND\"; \
             set muxy_status $status; if test $muxy_status -ne 0; exec \"$argv[1]\" -l; \
             else exit $muxy_status; end' /opt/homebrew/bin/fish"
        );
    }

    #[test]
    fn bash_startup_uses_the_integration_compatible_posix_mode() {
        assert!(startup_shell_command("/bin/bash", true).starts_with("/bin/bash --posix -l -i"));
    }
}
