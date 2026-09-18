use super::*;

#[test]
#[allow(clippy::too_many_lines, reason = "Cross-provider fixture table")]
fn bundled_manifests_compile_and_classify_provider_screens() {
    for (provider, source) in MANIFESTS {
        assert!(
            !compile(source)
                .unwrap_or_else(|error| panic!("{}: {error}", provider.name()))
                .is_empty()
        );
    }
    let fixtures = [
        (AgentProvider::Claude, "", "⠋ Working", AgentState::Working),
        (AgentProvider::Claude, "", "✳ Ready", AgentState::Idle),
        (
            AgentProvider::Codex,
            "",
            "Action Required",
            AgentState::Blocked,
        ),
        (AgentProvider::Codex, "", "⠋ Fix tests", AgentState::Working),
        (AgentProvider::Codex, "› ", "Fix tests", AgentState::Idle),
        (
            AgentProvider::OpenCode,
            "△ Permission required",
            "",
            AgentState::Blocked,
        ),
        (
            AgentProvider::OpenCode,
            "esc to interrupt",
            "",
            AgentState::Working,
        ),
        (
            AgentProvider::Cursor,
            "Waiting for approval\nRun this command?\nRun (once) (y)",
            "",
            AgentState::Blocked,
        ),
        (
            AgentProvider::Cursor,
            "ctrl+c to stop",
            "",
            AgentState::Working,
        ),
        (
            AgentProvider::Copilot,
            "enter to select · esc to cancel",
            "",
            AgentState::Blocked,
        ),
        (
            AgentProvider::Copilot,
            "◎ Waiting for background agents",
            "",
            AgentState::Working,
        ),
        (
            AgentProvider::Droid,
            "↑↓ navigate · enter select · esc cancel",
            "",
            AgentState::Blocked,
        ),
        (AgentProvider::Droid, "esc to stop", "", AgentState::Working),
        (
            AgentProvider::Pi,
            "── ⠋ Working ──────────",
            "",
            AgentState::Working,
        ),
        (
            AgentProvider::Grok,
            "┃  2 (○) Yes, proceed",
            "",
            AgentState::Blocked,
        ),
        (AgentProvider::Grok, "", "⠋ grok", AgentState::Working),
        (AgentProvider::Grok, "", "grok", AgentState::Idle),
        (
            AgentProvider::Kiro,
            "requires approval\nyes, single permission",
            "",
            AgentState::Blocked,
        ),
        (
            AgentProvider::Kiro,
            "kiro is working",
            "",
            AgentState::Working,
        ),
        (
            AgentProvider::Kiro,
            "ask a question or describe a task\n/copy to clipboard",
            "",
            AgentState::Idle,
        ),
        (
            AgentProvider::Xal,
            "! Approval needed · choose above",
            "",
            AgentState::Blocked,
        ),
        (
            AgentProvider::Xal,
            "? Input needed · answer above",
            "",
            AgentState::Blocked,
        ),
        (
            AgentProvider::Xal,
            "⠋ Working · Esc interrupt",
            "",
            AgentState::Working,
        ),
        (AgentProvider::Xal, "✓ Finished in 4s", "", AgentState::Idle),
        (
            AgentProvider::Xal,
            "! Interrupted after 4s",
            "",
            AgentState::Unknown,
        ),
        (
            AgentProvider::Antigravity,
            "requesting permission for:\ndo you want to proceed?",
            "",
            AgentState::Blocked,
        ),
        (
            AgentProvider::Antigravity,
            "⠋ Thinking",
            "",
            AgentState::Working,
        ),
    ];
    for (provider, screen, title, expected) in fixtures {
        assert_eq!(
            detect(
                provider,
                DetectionInput {
                    screen,
                    osc_title: title,
                    osc_progress: ""
                }
            )
            .map(|r| r.0),
            Some(expected),
            "{provider:?}: {screen} / {title}"
        );
    }
    for (provider, _) in MANIFESTS {
        assert_eq!(
            detect(
                *provider,
                DetectionInput {
                    screen: "New unrecognized UI",
                    osc_title: "",
                    osc_progress: ""
                }
            ),
            Some((AgentState::Idle, false))
        );
    }
}

#[test]
fn process_identity_uses_executable_positions_not_prompt_text() {
    assert_eq!(
        identify(
            "node",
            &[
                "node".into(),
                "/usr/lib/node_modules/@openai/codex/bin/codex.js".into()
            ]
        ),
        Some(AgentProvider::Codex)
    );
    assert_eq!(
        identify("python3", &["python3".into(), "/opt/bin/claude".into()]),
        Some(AgentProvider::Claude)
    );
    assert_eq!(
        identify(
            "node",
            &["node".into(), "my-app.js".into(), "claude".into()]
        ),
        None
    );
    assert_eq!(
        identify("bash", &["bash".into(), "-c".into(), "echo codex".into()]),
        None
    );
    assert_eq!(identify("tmux", &["tmux".into(), "codex".into()]), None);
}

#[test]
fn redraws_unmatched_screens_and_exits_do_not_invent_completion() {
    let mut detector = Detector::default();
    let now = Instant::now();
    assert!(!detector.update(
        Some(AgentProvider::Xal),
        "⠋ Working · Esc interrupt".into(),
        "",
        "",
        now
    ));
    assert_eq!(detector.state, AgentState::Working);
    assert!(!detector.update(
        Some(AgentProvider::Xal),
        String::new(),
        "",
        "",
        now + Duration::from_millis(100)
    ));
    assert_eq!(detector.state, AgentState::Working);
    assert!(!detector.update(
        Some(AgentProvider::Xal),
        "✓ Finished in 4s".into(),
        "",
        "",
        now + Duration::from_millis(200)
    ));
    assert!(detector.update(
        Some(AgentProvider::Xal),
        "✓ Finished in 4s".into(),
        "",
        "",
        now + Duration::from_millis(500)
    ));
    assert!(!detector.update(
        Some(AgentProvider::Xal),
        "⠋ Working · Esc interrupt".into(),
        "",
        "",
        now + Duration::from_secs(1)
    ));
    assert!(!detector.update(None, String::new(), "", "", now + Duration::from_secs(2)));
    assert_eq!(detector.state, AgentState::Unknown);
}

#[test]
fn old_prompt_text_does_not_override_live_xal_status() {
    let screen = format!(
        "! Approval needed · choose above\n{}\n✓ Finished in 4s",
        "old transcript\n".repeat(80)
    );
    assert_eq!(
        detect(
            AgentProvider::Xal,
            DetectionInput {
                screen: &screen,
                osc_title: "",
                osc_progress: ""
            }
        ),
        Some((AgentState::Idle, true))
    );
}

#[test]
#[ignore = "manual detector and terminal throughput measurement"]
#[allow(clippy::print_stderr, reason = "Reports manual benchmark measurements")]
fn detector_and_terminal_cost() -> Result<(), Box<dyn std::error::Error>> {
    prepare();
    let mut detectors: Vec<_> = (0..100).map(|_| Detector::default()).collect();
    let now = Instant::now();
    let screen = "A completed response\n› ".to_owned();
    for detector in &mut detectors {
        detector.update(Some(AgentProvider::Codex), screen.clone(), "Ready", "", now);
    }
    let start = Instant::now();
    for tick in 0..600 {
        for detector in &mut detectors {
            std::hint::black_box(detector.update(
                Some(AgentProvider::Codex),
                screen.clone(),
                "Ready",
                "",
                now + Duration::from_millis(tick * 100),
            ));
        }
    }
    eprintln!("100 idle agents × 600 polls: {:?}", start.elapsed());

    for detection in [false, true] {
        let mut terminal = muxy_terminal::Terminal::new(
            muxy_protocol::Size {
                cols: 200,
                rows: 50,
            },
            4 * 1024 * 1024,
        )?;
        let chunk = "build output with moderately long lines and progress\r\n".repeat(1200);
        let mut detector = Detector::default();
        let start = Instant::now();
        let mut next = start;
        let mut scans = 0;
        for _ in 0..1500 {
            terminal.feed(chunk.as_bytes());
            if detection && Instant::now() >= next {
                next = Instant::now() + Duration::from_millis(100);
                let screen = terminal.detection_text()?;
                detector.update(
                    Some(AgentProvider::Claude),
                    screen,
                    "⠋ Building",
                    "",
                    Instant::now(),
                );
                scans += 1;
            }
        }
        eprintln!(
            "{} bytes, detection={detection}, scans={scans}: {:?}",
            chunk.len() * 1500,
            start.elapsed()
        );
    }
    Ok(())
}

#[test]
fn fallback_providers_complete_only_after_stable_idle() {
    for (provider, working) in [
        (AgentProvider::OpenCode, "esc to interrupt"),
        (AgentProvider::Cursor, "ctrl+c to stop"),
        (AgentProvider::Copilot, "esc to cancel"),
        (AgentProvider::Droid, "esc to stop"),
        (AgentProvider::Pi, "Working..."),
        (AgentProvider::Antigravity, "⠋ Thinking"),
    ] {
        let mut detector = Detector::default();
        let now = Instant::now();
        assert!(!detector.update(Some(provider), "Ready".into(), "", "", now));
        assert!(!detector.update(Some(provider), working.into(), "", "", now));
        assert_eq!(detector.state, AgentState::Working, "{provider:?}");
        assert!(!detector.update(Some(provider), "Response complete".into(), "", "", now));
        assert_eq!(detector.state, AgentState::Working);
        assert!(
            detector.update(
                Some(provider),
                "Response complete".into(),
                "",
                "",
                now + Duration::from_millis(700)
            ),
            "{provider:?}"
        );
        assert!(!detector.update(
            Some(provider),
            "Response complete".into(),
            "",
            "",
            now + Duration::from_secs(1)
        ));
    }
}

#[test]
fn stable_title_does_not_turn_a_redraw_into_completion() {
    let mut detector = Detector::default();
    let now = Instant::now();
    let working = "• Working (1s • esc to interrupt)";
    detector.update(
        Some(AgentProvider::Codex),
        working.into(),
        "Fix tests",
        "",
        now,
    );
    assert_eq!(detector.state, AgentState::Working);
    assert!(!detector.update(
        Some(AgentProvider::Codex),
        String::new(),
        "Fix tests",
        "",
        now + Duration::from_millis(100)
    ));
    assert_eq!(detector.state, AgentState::Working);
    assert!(!detector.update(
        Some(AgentProvider::Codex),
        working.into(),
        "Fix tests",
        "",
        now + Duration::from_millis(200)
    ));
    assert_eq!(detector.state, AgentState::Working);
}

#[test]
fn long_blank_redraw_preserves_cycle_and_blocked_state() {
    for (initial, expected) in [
        ("⠋ Working · Esc interrupt", AgentState::Working),
        ("! Approval needed · choose above", AgentState::Blocked),
    ] {
        let mut detector = Detector::default();
        let now = Instant::now();
        detector.update(Some(AgentProvider::Xal), initial.into(), "", "", now);
        for elapsed in [100, 900] {
            assert!(!detector.update(
                Some(AgentProvider::Xal),
                String::new(),
                "",
                "",
                now + Duration::from_millis(elapsed)
            ));
            assert_eq!(detector.state, expected);
        }
        assert!(!detector.update(
            Some(AgentProvider::Xal),
            "✓ Finished in 4s".into(),
            "",
            "",
            now + Duration::from_secs(1)
        ));
        assert_eq!(
            detector.update(
                Some(AgentProvider::Xal),
                "✓ Finished in 4s".into(),
                "",
                "",
                now + Duration::from_millis(1300)
            ),
            expected == AgentState::Working
        );
    }
}

#[test]
fn codex_animated_idle_prompt_finishes_once_without_waiting_for_a_static_screen() {
    let mut detector = Detector::default();
    let now = Instant::now();
    detector.update(
        Some(AgentProvider::Codex),
        "• Working (1s • esc to interrupt)".into(),
        "Fix tests",
        "",
        now,
    );
    assert_eq!(detector.state, AgentState::Working);
    let mut completions = 0;
    for tick in 1..=20 {
        let screen = format!(
            "All tests passed.\n› Ask Codex to do anything {}⠈\n  gpt-6-astra low · ~/project",
            " ".repeat(tick)
        );
        completions += usize::from(detector.update(
            Some(AgentProvider::Codex),
            screen,
            "Fix tests",
            "",
            now + Duration::from_millis(tick as u64 * 100),
        ));
    }
    assert_eq!(detector.state, AgentState::Idle);
    assert_eq!(completions, 1);
}
