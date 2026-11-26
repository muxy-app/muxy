use super::*;
use muxy_protocol::OperationId;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Profile {
    root: PathBuf,
    shell: PathBuf,
}

impl Profile {
    fn new(shell: &str) -> io::Result<Option<Self>> {
        let Some(shell) = ["/bin", "/usr/bin"]
            .iter()
            .map(|folder| Path::new(folder).join(shell))
            .find(|path| path.is_file())
        else {
            return Ok(None);
        };
        let root = muxy_core::files::canonicalize(&std::env::temp_dir())?
            .join(format!("muxy-resume-{}", OperationId::new()));
        for folder in ["home", "work"] {
            fs::create_dir_all(root.join(folder))?;
        }
        fs::write(root.join("home/.zshrc"), "")?;
        fs::write(
            root.join("home/.bash_profile"),
            "source \"$MUXY_SHELL_INTEGRATION_DIR/muxy.bash\"\n",
        )?;
        Ok(Some(Self { root, shell }))
    }

    fn settings(&self) -> ServerSettings {
        ServerSettings {
            default_shell: Some(self.shell.clone()),
            resume_allowlist: vec!["sh".into()],
            ..ServerSettings::default()
        }
    }

    fn open(&self) -> io::Result<Registry> {
        let (events, _) = std::sync::mpsc::channel();
        let registry = Registry::persistent(self.settings(), events, &self.root.join("sessions"))?;
        Ok(
            registry.with_shell_integration(crate::ShellIntegration::install(
                &self.root.join("integration"),
            )?),
        )
    }

    fn record(&self, session: SessionId) -> PathBuf {
        self.root
            .join("sessions/resume")
            .join(format!("{:016x}.json", session.get()))
    }

    fn start(&self, registry: &Registry, resume_from: Option<SessionId>) -> io::Result<SessionId> {
        let info = registry
            .create_launched(
                registry.home_project(),
                OperationId::new(),
                &self.root.join("work"),
                Size {
                    cols: 120,
                    rows: 40,
                },
                Launch {
                    environment: [(
                        "HOME".to_owned(),
                        self.root.join("home").display().to_string(),
                    )]
                    .into(),
                    resume_from,
                    ..Launch::default()
                },
                None,
            )
            .map_err(io::Error::other)?;
        Ok(info.id)
    }
}

impl Drop for Profile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn wait(what: &str, mut done: impl FnMut() -> bool) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done() {
        if Instant::now() > deadline {
            return Err(io::Error::other(format!("timed out waiting for {what}")));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

fn encoded(argv: &[&str]) -> String {
    argv.iter()
        .map(|argument| muxy_core::base64::encode(argument.as_bytes()))
        .collect::<Vec<_>>()
        .join(",")
}

fn report(registry: &Registry, session: SessionId, argv: &[&str], then: &str) -> TestResult {
    let script = format!(
        "body=\"pid=$$:time=$(date +%s):argv={}\"; mac=$(printf '%s:%s' \"$body\" \"$OSC7577_TOKEN\" | {{ sha256sum 2>/dev/null || shasum -a 256; }} | cut -c1-64); printf '\\033]7577;%s:mac=%s\\033\\\\' \"$body\" \"$mac\"; {then}",
        encoded(argv)
    );
    let line = format!("/bin/sh -c '{}'\r", script.replace('\'', "'\\''"));
    registry
        .handle(session)
        .ok_or("session ended early")?
        .send(session::SessionCommand::Input(line.into_bytes()))?;
    Ok(())
}

fn resumes_with_exact_arguments_in_its_folder(shell: &str) -> TestResult {
    let Some(profile) = Profile::new(shell)? else {
        return Ok(());
    };
    let work = profile.root.join("work");
    let argv = [
        "/bin/sh",
        "-c",
        "pwd > out.tmp; printf \"[%s]\" \"$@\" >> out.tmp; mv out.tmp out",
        "sh",
        "a  b",
        "$(touch pwned)",
        "q'u\"ote",
        "",
    ];
    let registry = profile.open()?;
    let first = profile.start(&registry, None)?;
    report(&registry, first, &argv, "exec sleep 60")?;
    wait("the report", || profile.record(first).exists())?;
    registry.shutdown();
    registry.wait_stopped();
    drop(registry);
    assert!(profile.record(first).exists());

    let registry = profile.open()?;
    let resumed = profile.start(&registry, Some(first))?;
    assert!(!profile.record(first).exists());
    wait("the resumed program", || work.join("out").exists())?;
    assert_eq!(
        fs::read_to_string(work.join("out"))?,
        format!("{}\n[a  b][$(touch pwned)][q'u\"ote][]", work.display())
    );
    assert!(!work.join("pwned").exists());
    registry.end(resumed)?;
    Ok(())
}

#[test]
fn a_reported_program_resumes_in_zsh_after_a_server_restart() -> TestResult {
    resumes_with_exact_arguments_in_its_folder("zsh")
}

#[test]
fn a_reported_program_resumes_in_bash_after_a_server_restart() -> TestResult {
    resumes_with_exact_arguments_in_its_folder("bash")
}

#[test]
fn records_end_with_their_program_or_terminal_but_not_the_server() -> TestResult {
    let Some(profile) = Profile::new("sh")? else {
        return Ok(());
    };
    let registry = profile.open()?;
    let exiting = profile.start(&registry, None)?;
    report(&registry, exiting, &["sh"], "sleep 1")?;
    wait("the report", || profile.record(exiting).exists())?;
    wait("the program to exit", || !profile.record(exiting).exists())?;

    let closed = profile.start(&registry, None)?;
    report(&registry, closed, &["sh"], "exec sleep 60")?;
    wait("the report", || profile.record(closed).exists())?;
    registry.end(closed)?;
    registry.wait_ended(closed)?;
    assert!(!profile.record(closed).exists());

    let stopped = profile.start(&registry, None)?;
    report(&registry, stopped, &["sh"], "exec sleep 60")?;
    wait("the report", || profile.record(stopped).exists())?;
    registry.shutdown();
    registry.wait_stopped();
    assert!(profile.record(stopped).exists());
    drop(registry);

    let registry = profile.open()?;
    registry.discard(stopped)?;
    assert!(!profile.record(stopped).exists());
    Ok(())
}

#[test]
fn programs_are_not_resumed_when_resume_is_off_or_they_are_not_allowed() -> TestResult {
    let Some(profile) = Profile::new("zsh")? else {
        return Ok(());
    };
    let work = profile.root.join("work");
    let argv = ["/bin/sh", "-c", "touch resumed"];
    for settings in [
        ServerSettings {
            resume_programs: false,
            ..profile.settings()
        },
        ServerSettings {
            resume_allowlist: vec!["claude".into()],
            ..profile.settings()
        },
    ] {
        let registry = profile.open()?;
        let first = profile.start(&registry, None)?;
        report(&registry, first, &argv, "exec sleep 60")?;
        wait("the report", || profile.record(first).exists())?;
        registry.shutdown();
        registry.wait_stopped();
        drop(registry);
        let registry = profile.open()?;
        registry.write_settings(settings)?;
        let resumed = profile.start(&registry, Some(first))?;
        std::thread::sleep(Duration::from_secs(2));
        assert!(!work.join("resumed").exists());
        assert!(!profile.record(first).exists());
        registry.end(resumed)?;
    }
    Ok(())
}

#[test]
fn a_shell_that_fails_to_start_keeps_the_record_for_the_next_try() -> TestResult {
    let Some(profile) = Profile::new("sh")? else {
        return Ok(());
    };
    let registry = profile.open()?;
    let old = SessionId::new(77).ok_or("zero id")?;
    let record = crate::resume::Record {
        argv: vec!["sh".into()],
        cwd: None,
    };
    registry.resumes.save(old, Some(&record));
    *registry
        .settings
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = ServerSettings {
        default_shell: Some(profile.root.join("missing-shell")),
        ..profile.settings()
    };
    assert!(profile.start(&registry, Some(old)).is_err());
    assert!(profile.record(old).exists());
    Ok(())
}
