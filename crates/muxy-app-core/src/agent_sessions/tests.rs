use std::fs;
use std::io::Write;

use serde_json::json;

use super::*;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn fixture(home: &Path, relative: &str, records: &[serde_json::Value]) -> std::io::Result<PathBuf> {
    let path = home.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(&path)?;
    for record in records {
        writeln!(file, "{record}")?;
    }
    Ok(path)
}

#[test]
fn reads_native_formats_and_scopes_to_the_exact_project() -> Result {
    let home = tempfile::tempdir()?;
    let project = home.path().join("project");
    let worktree = project.join("worktree");
    fs::create_dir_all(&worktree)?;
    let examples = [
        (
            ".claude/projects/project/claude.jsonl",
            json!({"type":"user","sessionId":"claude-1","cwd":project,"message":{"content":[{"type":"text","text":"Fix regression"}]}}),
        ),
        (
            ".codex/sessions/2026/01/01/codex.jsonl",
            json!({"type":"session_meta","payload":{"id":"codex-1","cwd":project}}),
        ),
        (
            ".copilot/session-state/copilot-1/events.jsonl",
            json!({"type":"session.start","data":{"sessionId":"copilot-1","context":{"cwd":project}}}),
        ),
        (
            ".factory/sessions/project/droid.jsonl",
            json!({"type":"session_start","id":"droid-1","cwd":project,"title":"Fix regression"}),
        ),
        (
            ".pi/agent/sessions/project/pi.jsonl",
            json!({"type":"session","id":"pi-1","cwd":project}),
        ),
        (
            ".omp/agent/sessions/project/omp.jsonl",
            json!({"type":"session","id":"omp-1","cwd":project}),
        ),
        (
            ".xal/sessions/project/xal.jsonl",
            json!({"type":"meta","meta":{"id":"xal-1","cwd":project}}),
        ),
    ];
    for (path, record) in examples {
        fixture(home.path(), path, &[record])?;
    }
    fixture(
        home.path(),
        ".claude/projects/worktree/child.jsonl",
        &[json!({"type":"user","sessionId":"worktree-1","cwd":worktree})],
    )?;
    fixture(
        home.path(),
        ".claude/projects/project/sidechain.jsonl",
        &[json!({"type":"user","sessionId":"sidechain-1","cwd":project,"isSidechain":true})],
    )?;
    fixture(
        home.path(),
        ".codex/sessions/2026/01/01/child.jsonl",
        &[
            json!({"type":"session_meta","payload":{"id":"subagent-1","cwd":project,"source":{"subagent":{}}}}),
        ],
    )?;
    let roots = Roots::from_home(home.path());
    let mut catalog = Catalog::default();
    let listing = catalog.scan(&roots, &project);
    assert_eq!(listing.sessions.len(), 7);
    assert!(listing.warnings.is_empty());
    for provider in PROVIDERS
        .into_iter()
        .filter(|provider| supported(*provider))
    {
        assert!(
            listing
                .sessions
                .iter()
                .any(|session| session.provider == provider)
        );
    }
    let listing = catalog.scan(&roots, &worktree);
    assert_eq!(listing.sessions.len(), 1);
    assert_eq!(listing.sessions[0].id, "worktree-1");
    Ok(())
}

#[test]
fn refresh_handles_appends_partial_records_renames_and_deletions() -> Result {
    let home = tempfile::tempdir()?;
    let path = fixture(
        home.path(),
        ".claude/projects/project/one.jsonl",
        &[
            json!({"type":"user","sessionId":"session-1","cwd":home.path(),"message":{"content":"Initial prompt"}}),
        ],
    )?;
    let roots = Roots::from_home(home.path());
    let mut catalog = Catalog::default();
    assert_eq!(
        catalog.scan(&roots, home.path()).sessions[0].title,
        "Initial prompt"
    );
    let mut file = fs::OpenOptions::new().append(true).open(&path)?;
    write!(
        file,
        "{{\"type\":\"custom-title\",\"customTitle\":\"Renamed\"}}"
    )?;
    assert_eq!(
        catalog.scan(&roots, home.path()).sessions[0].title,
        "Initial prompt"
    );
    writeln!(file)?;
    let listing = catalog.scan(&roots, home.path());
    assert_eq!(listing.sessions[0].title, "Renamed");
    assert!(listing.sessions[0].matches("rename CLAUDE"));
    assert!(!listing.sessions[0].matches("missing"));
    fs::remove_file(path)?;
    assert!(catalog.scan(&roots, home.path()).sessions.is_empty());
    Ok(())
}

#[test]
fn titles_survive_transcript_growth_on_refresh_and_fresh_scans() -> Result {
    let home = tempfile::tempdir()?;
    let path = fixture(
        home.path(),
        ".claude/projects/project/one.jsonl",
        &[
            json!({"type":"user","sessionId":"session-1","cwd":home.path(),"message":{"content":"Initial prompt"}}),
            json!({"type":"custom-title","customTitle":"Renamed"}),
        ],
    )?;
    let roots = Roots::from_home(home.path());
    let mut catalog = Catalog::default();
    assert_eq!(
        catalog.scan(&roots, home.path()).sessions[0].title,
        "Renamed"
    );
    let mut file = fs::OpenOptions::new().append(true).open(path)?;
    let message = json!({"type":"assistant","message":{"content":"x".repeat(1024)}});
    for count in [128, 1024] {
        for _ in 0..count {
            writeln!(file, "{message}")?;
        }
        for listing in [
            catalog.scan(&roots, home.path()),
            Catalog::default().scan(&roots, home.path()),
        ] {
            assert_eq!(listing.sessions.len(), 1);
            assert_eq!(listing.sessions[0].title, "Renamed");
        }
    }
    Ok(())
}

#[test]
fn long_malformed_transcripts_are_bounded_and_appended_titles_are_read() -> Result {
    let home = tempfile::tempdir()?;
    let path = fixture(
        home.path(),
        ".xal/sessions/project/one.jsonl",
        &[
            json!({"type":"meta","meta":{"id":"session-1","cwd":home.path()}}),
            json!({"type":"item","item":{"type":"user_message","text":"Initial"}}),
        ],
    )?;
    let mut file = fs::OpenOptions::new().append(true).open(path)?;
    file.write_all(&vec![b'x'; 2 * 1024 * 1024])?;
    writeln!(file)?;
    writeln!(
        file,
        "{}",
        json!({"type":"event","event":{"type":"session_title_changed","title":"Final title"}})
    )?;
    let listing = Catalog::default().scan(&Roots::from_home(home.path()), home.path());
    assert_eq!(listing.sessions.len(), 1);
    assert_eq!(listing.sessions[0].title, "Final title");
    Ok(())
}

#[test]
fn resume_commands_never_interpolate_transcript_text_or_paths() {
    let mut session = Session {
        provider: AgentProvider::Xal,
        id: "a123-456".into(),
        title: "$(touch /tmp/unsafe)".into(),
        directory: PathBuf::from("/project"),
        path: PathBuf::from("/home/'$(unsafe)/session.jsonl"),
        modified: SystemTime::UNIX_EPOCH,
    };
    for (provider, command) in [
        (AgentProvider::Claude, "claude --resume"),
        (AgentProvider::Codex, "codex resume"),
        (AgentProvider::Copilot, "copilot --resume"),
        (AgentProvider::Droid, "droid --resume"),
        (AgentProvider::Pi, "pi --session"),
        (AgentProvider::OhMyPi, "omp --resume"),
        (AgentProvider::Xal, "xal resume"),
    ] {
        session.provider = provider;
        assert_eq!(
            session.resume_command(),
            Some(format!("{command} a123-456"))
        );
    }
    for id in [
        "", "--help", "a;exit", "a\nexit", "$(exit)", "a%PATH%", "a'", "../a",
    ] {
        session.id = id.into();
        assert_eq!(session.resume_command(), None);
    }
}

#[cfg(unix)]
#[test]
fn symlinked_transcripts_are_not_followed_but_project_aliases_match() -> Result {
    let home = tempfile::tempdir()?;
    let project = home.path().join("project");
    fs::create_dir(&project)?;
    let alias = home.path().join("alias");
    std::os::unix::fs::symlink(&project, &alias)?;
    let path = fixture(
        home.path(),
        ".pi/agent/sessions/project/one.jsonl",
        &[json!({"type":"session","id":"one","cwd":alias})],
    )?;
    let link = path.with_file_name("link.jsonl");
    std::os::unix::fs::symlink(path, &link)?;
    let listing = Catalog::default().scan(&Roots::from_home(home.path()), &project);
    assert_eq!(listing.sessions.len(), 1);
    Ok(())
}

#[test]
fn a_large_provider_history_does_not_starve_other_providers_on_first_open() -> Result {
    let home = tempfile::tempdir()?;
    for index in 0..70 {
        let path = fixture(
            home.path(),
            &format!(".claude/projects/project/{index}.jsonl"),
            &[json!({"type":"user","sessionId":format!("claude-{index}"),"cwd":home.path()})],
        )?;
        fs::OpenOptions::new()
            .write(true)
            .open(path)?
            .set_len(1024 * 1024)?;
    }
    fixture(
        home.path(),
        ".codex/sessions/2026/01/01/codex.jsonl",
        &[json!({"type":"session_meta","payload":{"id":"codex-1","cwd":home.path()}})],
    )?;
    fixture(
        home.path(),
        ".xal/sessions/project/xal.jsonl",
        &[json!({"type":"meta","meta":{"id":"xal-1","cwd":home.path()}})],
    )?;
    let roots = Roots::from_home(home.path());
    let mut catalog = Catalog::default();
    let mut listing = catalog.scan(&roots, home.path());
    for provider in [
        AgentProvider::Claude,
        AgentProvider::Codex,
        AgentProvider::Xal,
    ] {
        assert!(
            listing
                .sessions
                .iter()
                .any(|session| session.provider == provider),
            "missing {provider:?} on first scan"
        );
    }
    assert!(listing.pending);
    assert!(!listing.limited);
    for _ in 0..10 {
        if !listing.pending {
            break;
        }
        let count = listing.sessions.len();
        listing = catalog.scan(&roots, home.path());
        assert!(listing.sessions.len() > count, "each batch makes progress");
    }
    assert!(!listing.pending);
    assert_eq!(listing.sessions.len(), 72);
    Ok(())
}

#[test]
fn exhausted_read_budget_does_not_read_past_the_limit() -> Result {
    let home = tempfile::tempdir()?;
    let path = fixture(
        home.path(),
        "session.jsonl",
        &[
            json!({"type":"session","id":"one","cwd":home.path()}),
            json!({"type":"message","message":{"role":"user","content":"Fix regression"}}),
        ],
    )?;
    let mut budget = 10;
    assert!(
        parse::read(
            &path,
            AgentProvider::Pi,
            SystemTime::UNIX_EPOCH,
            &mut budget
        )?
        .is_none()
    );
    assert_eq!(budget, 0);
    let mut budget = 4096;
    assert!(
        parse::read(
            &path,
            AgentProvider::Pi,
            SystemTime::UNIX_EPOCH,
            &mut budget
        )?
        .is_some()
    );
    assert!(budget > 0);
    Ok(())
}
