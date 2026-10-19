use std::path::Path;

use muxy_protocol::{AgentProvider, ServerPath};

use crate::platform::windows::JobProcess;
use crate::spawn::Shell;

/// The newest program the shell started that is still running, or the shell
/// itself. Windows has no foreground process group, and its children all
/// share the session's job.
pub(super) struct Foreground {
    pub(super) group: u32,
    pub(super) is_shell: bool,
    name: String,
    processes: Vec<JobProcess>,
}

impl Foreground {
    pub(super) fn of(shell: &Shell) -> Option<Self> {
        let shell_pid = shell.pty.child_pid();
        let processes = shell.job.processes();
        let started = processes
            .iter()
            .find(|process| process.pid == shell_pid)?
            .created;
        let foreground = processes
            .iter()
            .rfind(|process| process.parent == shell_pid && process.created >= started)
            .or_else(|| processes.iter().find(|process| process.pid == shell_pid))?;
        Some(Self {
            group: foreground.pid,
            is_shell: foreground.pid == shell_pid,
            name: foreground
                .image()
                .map_or_else(String::new, |image| program_name(&image)),
            processes,
        })
    }

    pub(super) fn name(&self) -> String {
        self.name.clone()
    }

    pub(super) fn is_idle_shell(&self) -> bool {
        self.is_shell
    }

    /// An agent among the foreground program and the processes it started,
    /// such as `claude` running on `node.exe`.
    pub(super) fn agent(&self) -> Option<AgentProvider> {
        let mut family = vec![self.group];
        for process in &self.processes {
            if family.contains(&process.parent) && !family.contains(&process.pid) {
                family.push(process.pid);
            }
        }
        self.processes
            .iter()
            .filter(|process| family.contains(&process.pid))
            .take(128)
            .find_map(|process| {
                let arguments = arguments(&process.command_line().unwrap_or_default());
                crate::detection::identify(&program_name(&process.image()?), &arguments)
            })
    }

    /// Windows can't read another process's working directory, so the
    /// folder comes only from OSC 7.
    #[allow(
        clippy::unused_self,
        reason = "Matches the Unix foreground's interface"
    )]
    pub(super) fn directory(&self) -> Option<ServerPath> {
        None
    }
}

/// `C:\…\node.exe` is `node`, as a Unix process name would be.
fn program_name(image: &Path) -> String {
    let name = image
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let name = name.unwrap_or_default();
    match name.len().checked_sub(4) {
        Some(stem) if name[stem..].eq_ignore_ascii_case(".exe") => name[..stem].to_owned(),
        _ => name,
    }
}

/// The first arguments of a command line, split as the Microsoft C runtime
/// does, with `\` turned into `/` and the program named as by
/// [`program_name`], so agent detection reads them as it does on Unix.
fn arguments(command_line: &str) -> Vec<String> {
    split_command_line(command_line)
        .into_iter()
        .take(64)
        .enumerate()
        .map(|(index, argument)| {
            if index == 0 {
                program_name(Path::new(&argument))
            } else {
                argument.replace('\\', "/")
            }
        })
        .collect()
}

fn split_command_line(command_line: &str) -> Vec<String> {
    let mut arguments = Vec::new();
    let mut characters = command_line.chars().peekable();
    // The program name ends at the first space outside quotes and has no escapes.
    let mut program = String::new();
    let mut quoted = false;
    while let Some(&character) = characters.peek() {
        if character == '"' {
            quoted = !quoted;
        } else if !quoted && character.is_whitespace() {
            break;
        } else {
            program.push(character);
        }
        characters.next();
    }
    arguments.push(program);
    loop {
        while characters
            .next_if(|character| character.is_whitespace())
            .is_some()
        {}
        if characters.peek().is_none() {
            return arguments;
        }
        let mut argument = String::new();
        let mut quoted = false;
        while let Some(&character) = characters.peek() {
            match character {
                '\\' => {
                    let mut slashes = 0;
                    while characters.next_if_eq(&'\\').is_some() {
                        slashes += 1;
                    }
                    if characters.peek() == Some(&'"') {
                        argument.extend(std::iter::repeat_n('\\', slashes / 2));
                        if slashes % 2 == 1 {
                            argument.push('"');
                            characters.next();
                        }
                    } else {
                        argument.extend(std::iter::repeat_n('\\', slashes));
                    }
                    continue;
                }
                '"' => {
                    characters.next();
                    if quoted && characters.peek() == Some(&'"') {
                        argument.push('"');
                        characters.next();
                    } else {
                        quoted = !quoted;
                    }
                    continue;
                }
                character if character.is_whitespace() && !quoted => break,
                character => argument.push(character),
            }
            characters.next();
        }
        arguments.push(argument);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_lines_split_as_the_c_runtime_does() {
        assert_eq!(
            split_command_line(
                r#""C:\Program Files\nodejs\node.exe" C:\npm\cli.js "a b" c\"d e\\"#
            ),
            [
                r"C:\Program Files\nodejs\node.exe",
                r"C:\npm\cli.js",
                "a b",
                "c\"d",
                r"e\\",
            ]
        );
        assert_eq!(
            split_command_line(r#"x "a""b" "\\" end"#),
            ["x", "a\"b", "\\", "end"]
        );
        assert_eq!(split_command_line(""), [""]);
    }

    #[test]
    fn agents_launched_through_node_are_identified() {
        let claude = arguments(
            r#""C:\Program Files\nodejs\node.exe" C:\Users\me\AppData\Roaming\npm\node_modules\@anthropic-ai\claude-code\cli.js --resume"#,
        );
        assert_eq!(claude[0], "node");
        assert_eq!(
            crate::detection::identify("node", &claude),
            Some(AgentProvider::Claude)
        );
        let codex = arguments(r"C:\nodejs\node.exe C:\npm\node_modules\@openai\codex\bin\codex.js");
        assert_eq!(
            crate::detection::identify("node", &codex),
            Some(AgentProvider::Codex)
        );
        assert_eq!(
            program_name(Path::new(r"C:\Users\me\.local\bin\claude.EXE")),
            "claude"
        );
        assert_eq!(
            crate::detection::identify(&program_name(Path::new(r"C:\bin\claude.exe")), &[]),
            Some(AgentProvider::Claude)
        );
    }
}
