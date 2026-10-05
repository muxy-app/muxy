//! ssh asks the running app for a remote server's password. The app's own
//! executable is ssh's `SSH_ASKPASS`; it fetches the answer from the app
//! through a private socket. Passwords stay in memory and are never saved.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use muxy_app_core::ServerId;

/// Where the app answers. The app runs as askpass when it is set.
pub(crate) const SOCKET: &str = "MUXY_ASKPASS_SOCKET";
/// Which password ssh asks for. It is in the environment of the app's ssh
/// runs, so only processes of this user can read it.
pub(crate) const TOKEN: &str = "MUXY_ASKPASS_TOKEN";

/// What the socket answers for one token.
#[derive(Default)]
pub(crate) struct Answer {
    password: Option<String>,
    /// The destination's `user@host`. A password prompt that names any other
    /// login, such as a jump host's, gets nothing.
    login: Option<String>,
}

pub(crate) type Answers = Arc<Mutex<HashMap<String, Answer>>>;

/// Passwords typed this session, and the tokens ssh asks with.
pub(crate) struct Passwords {
    socket: PathBuf,
    listening: bool,
    /// Each remote server's token, made once per session.
    tokens: HashMap<ServerId, String>,
    answers: Answers,
}

impl Passwords {
    pub(crate) fn new(socket: PathBuf) -> Self {
        Self {
            socket,
            listening: false,
            tokens: HashMap::new(),
            answers: Arc::default(),
        }
    }

    /// Moves the socket before anything listens, for paths short enough
    /// for a socket in tests.
    #[cfg(test)]
    pub(crate) fn set_socket(&mut self, socket: PathBuf) {
        assert!(!self.listening, "the socket already listens");
        self.socket = socket;
    }

    fn answers(&self) -> MutexGuard<'_, HashMap<String, Answer>> {
        self.answers.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn token(&mut self, server: ServerId) -> String {
        self.tokens
            .entry(server)
            .or_insert_with(|| ServerId::new().to_string())
            .clone()
    }

    /// How ssh asks for `server`'s password, which answers only prompts that
    /// name `login`.
    pub(crate) fn askpass(
        &mut self,
        server: ServerId,
        login: String,
    ) -> io::Result<muxy_client::Askpass> {
        let token = self.token(server);
        self.answers().entry(token.clone()).or_default().login = Some(login);
        self.program(token)
    }

    fn program(&self, token: String) -> io::Result<muxy_client::Askpass> {
        Ok(muxy_client::Askpass {
            program: std::env::current_exe()?,
            environment: vec![
                (SOCKET.into(), self.socket.clone().into_os_string()),
                (TOKEN.into(), token.into()),
            ],
        })
    }

    pub(crate) fn has(&self, server: ServerId) -> bool {
        self.tokens.get(&server).is_some_and(|token| {
            self.answers()
                .get(token)
                .is_some_and(|answer| answer.password.is_some())
        })
    }

    pub(crate) fn set(&mut self, server: ServerId, password: String) -> io::Result<()> {
        self.listen()?;
        let token = self.token(server);
        self.answers().entry(token).or_default().password = Some(password);
        Ok(())
    }

    pub(crate) fn forget(&mut self, server: ServerId) {
        if let Some(token) = self.tokens.get(&server) {
            let token = token.clone();
            if let Some(answer) = self.answers().get_mut(&token) {
                answer.password = None;
            }
        }
    }

    /// A one-off password for `login`, such as for a Test Connection, with
    /// the askpass that answers it. Forget it with `forget_once`.
    pub(crate) fn once(
        &mut self,
        password: String,
        login: String,
    ) -> io::Result<(String, muxy_client::Askpass)> {
        self.listen()?;
        let token = ServerId::new().to_string();
        let answer = Answer {
            password: Some(password),
            login: Some(login),
        };
        self.answers().insert(token.clone(), answer);
        let askpass = self.program(token.clone())?;
        Ok((token, askpass))
    }

    pub(crate) fn forget_once(answers: &Answers, token: &str) {
        answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(token);
    }

    pub(crate) fn shared(&self) -> Answers {
        Arc::clone(&self.answers)
    }

    /// Answers on the socket from now on, which only this user can reach.
    fn listen(&mut self) -> io::Result<()> {
        if self.listening {
            return Ok(());
        }
        let _ = std::fs::remove_file(&self.socket);
        let listener = UnixListener::bind(&self.socket)?;
        std::fs::set_permissions(&self.socket, std::fs::Permissions::from_mode(0o600))?;
        let answers = Arc::clone(&self.answers);
        std::thread::Builder::new()
            .name("muxy-askpass".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let _ = respond(stream, &answers);
                }
            })?;
        self.listening = true;
        Ok(())
    }
}

impl Drop for Passwords {
    fn drop(&mut self) {
        if self.listening {
            let _ = std::fs::remove_file(&self.socket);
        }
    }
}

/// Reads `token`, a newline, and ssh's prompt, then writes the password, or
/// nothing to refuse.
fn respond(mut stream: UnixStream, answers: &Mutex<HashMap<String, Answer>>) -> io::Result<()> {
    let mut request = Vec::new();
    Read::by_ref(&mut stream)
        .take(16 * 1024)
        .read_to_end(&mut request)?;
    let request = String::from_utf8_lossy(&request);
    let (token, prompt) = request.split_once('\n').unwrap_or((&request, ""));
    let answer = answers
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(token)
        .and_then(|answer| answer_for(answer, prompt))
        .unwrap_or_default();
    stream.write_all(answer.as_bytes())
}

/// A key's passphrase never leaves this computer, so ssh's own passphrase
/// prompt gets the password. A password goes to a host, so only a prompt
/// for exactly the destination's login gets it. Anything else, such as a jump
/// host's prompt or trusting an unknown host, is refused.
fn answer_for(answer: &Answer, prompt: &str) -> Option<String> {
    let wanted = prompt.starts_with("Enter passphrase for key '")
        || answer
            .login
            .as_deref()
            .is_some_and(|login| asks_password_of(prompt, login));
    answer.password.clone().filter(|_| wanted)
}

/// ssh starts a password prompt with the login it is for: `user@host's
/// password: `, or `(user@host) ` before the server's own words. Both parts
/// are as `ssh -G` resolves them, so they match exactly.
fn asks_password_of(prompt: &str, login: &str) -> bool {
    if prompt.strip_prefix(login).map(str::trim_end) == Some("'s password:") {
        return true;
    }
    prompt
        .strip_prefix('(')
        .and_then(|rest| rest.strip_prefix(login))
        .and_then(|rest| rest.strip_prefix(") "))
        .is_some_and(|words| words.to_lowercase().contains("password"))
}

/// Answers one ssh prompt by asking the app, then exits.
pub(crate) fn run(socket: &std::ffi::OsStr, prompt: &str) -> ExitCode {
    let token = std::env::var(TOKEN).unwrap_or_default();
    let answer = UnixStream::connect(socket).and_then(|mut stream| {
        stream.write_all(format!("{token}\n{prompt}").as_bytes())?;
        stream.shutdown(std::net::Shutdown::Write)?;
        let mut answer = String::new();
        stream.read_to_string(&mut answer)?;
        Ok(answer)
    });
    match answer {
        Ok(answer) if !answer.is_empty() && writeln!(io::stdout(), "{answer}").is_ok() => {
            ExitCode::SUCCESS
        }
        _ => ExitCode::FAILURE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(socket: &std::path::Path, token: &str, prompt: &str) -> io::Result<String> {
        let mut stream = UnixStream::connect(socket)?;
        stream.write_all(format!("{token}\n{prompt}").as_bytes())?;
        stream.shutdown(std::net::Shutdown::Write)?;
        let mut answer = String::new();
        stream.read_to_string(&mut answer)?;
        Ok(answer)
    }

    #[test]
    fn the_app_answers_its_own_ssh_runs_with_passwords_typed_this_session() -> io::Result<()> {
        let directory = tempfile::Builder::new()
            .prefix("askpass")
            .tempdir_in("/tmp")?;
        let socket = directory.path().join("askpass.sock");
        let server = ServerId::new();
        let mut passwords = Passwords::new(socket.clone());
        assert!(!passwords.has(server));
        let askpass = passwords.askpass(server, "dev@box".into())?;
        assert!(
            !socket.exists(),
            "nothing listens until a password is typed"
        );
        passwords.set(server, "hunter2".into())?;
        assert!(passwords.has(server));
        assert_eq!(
            std::fs::metadata(&socket)?.permissions().mode() & 0o777,
            0o600
        );
        let token = askpass
            .environment
            .iter()
            .find(|(key, _)| key == TOKEN)
            .map(|(_, value)| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        for prompt in [
            "dev@box's password: ",
            "(dev@box) Password: ",
            "Enter passphrase for key '/Users/dev/.ssh/id_ed25519': ",
        ] {
            assert_eq!(ask(&socket, &token, prompt)?, "hunter2");
        }
        let trust = "Are you sure you want to continue connecting (yes/no/[fingerprint])? ";
        assert_eq!(ask(&socket, &token, trust)?, "");
        assert_eq!(
            ask(&socket, &token, "jump@gateway's password: ")?,
            "",
            "a jump host's prompt never gets the destination's password"
        );
        assert_eq!(ask(&socket, "someone else", "Password: ")?, "");
        let (once, _) = passwords.once("typed".into(), "dev@other".into())?;
        assert_eq!(ask(&socket, &once, "dev@other's password: ")?, "typed");
        Passwords::forget_once(&passwords.shared(), &once);
        assert_eq!(ask(&socket, &once, "dev@other's password: ")?, "");
        passwords.forget(server);
        assert!(!passwords.has(server));
        assert_eq!(ask(&socket, &token, "dev@box's password: ")?, "");
        drop(passwords);
        assert!(!socket.exists());
        Ok(())
    }

    #[test]
    fn only_prompts_for_the_destinations_own_login_get_its_password() {
        let answer = Answer {
            password: Some("hunter2".into()),
            login: Some("dev@box".into()),
        };
        for prompt in [
            "dev@box's password: ",
            "(dev@box) Password: ",
            "Enter passphrase for key '/Users/dev/.ssh/id_ed25519': ",
        ] {
            assert_eq!(answer_for(&answer, prompt).as_deref(), Some("hunter2"));
        }
        for prompt in [
            "dev@box-jump's password: ",
            "otherdev@box's password: ",
            "Dev@box's password: ",
            "(dev@box-jump) Password: ",
            "(otherdev@box) Password: ",
            "(jump@gateway) dev@box's password: ",
            "(jump@gateway) Enter passphrase for key 'id': ",
            "(dev@box) Verification code: ",
            "Enter dev@box's new password: ",
        ] {
            assert_eq!(answer_for(&answer, prompt), None, "{prompt}");
        }
    }
}
