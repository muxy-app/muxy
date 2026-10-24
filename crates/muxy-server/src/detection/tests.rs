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
        (
            AgentProvider::OhMyPi,
            " say hello\n  ⎋ Working…",
            "",
            AgentState::Working,
        ),
        (
            AgentProvider::OhMyPi,
            "╭─── ✎ Write: 🗒 test.txt · 1 line ─────╮\n│   1 hello                         ╯\n[↑ ↓ to navigate, ⏎ to select, ⎋ to deny]",
            "",
            AgentState::Blocked,
        ),
        (
            AgentProvider::OhMyPi,
            " π · ◉ glm-5.2 · 🗑 /tmp ↳ muxy-pr · ⑂ fix/1x-icon-import",
            "",
            AgentState::Idle,
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
fn claude_background_shells_keep_the_agent_working() {
    let prompt = "\n────────────────────\n❯ \n────────────────────\n  ⏵⏵ auto mode on";
    let cases = [
        (
            "✻ Baked for 1m 25s · done 2:38 PM · 1 shell still running",
            prompt,
            AgentState::Working,
        ),
        (
            "✻ Cooked for 4m 2s · 3 shells still running",
            prompt,
            AgentState::Working,
        ),
        (
            "✻ Brewed for 12s · 2 shells, 1 monitor still running",
            prompt,
            AgentState::Working,
        ),
        (
            "✻ Churned for 3m 40s · done 11:02 AM · 2 shells\n  still running",
            prompt,
            AgentState::Working,
        ),
        (
            "✻ Baked for 9s · 1 monitor still running",
            prompt,
            AgentState::Idle,
        ),
        (
            "❯ why does it say · 1 shell still running",
            prompt,
            AgentState::Idle,
        ),
        (
            "✻ Baked for 1m 25s · 1 shell still running",
            "\n Bash command\n Do you want to proceed?\n ❯ 1. Yes\n   2. No\n\n Esc to cancel",
            AgentState::Blocked,
        ),
    ];
    for (summary, below, expected) in cases {
        let screen = format!("{summary}\n{below}");
        assert_eq!(
            detect(
                AgentProvider::Claude,
                DetectionInput {
                    screen: &screen,
                    osc_title: "✳ Claude Code",
                    osc_progress: ""
                }
            )
            .map(|r| r.0),
            Some(expected),
            "{summary}"
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
    assert_eq!(
        identify("omp", &["omp".into()]),
        Some(AgentProvider::OhMyPi)
    );
    assert_eq!(
        identify("node", &["node".into(), "/usr/local/bin/oh-my-pi".into()]),
        Some(AgentProvider::OhMyPi)
    );
    assert_eq!(identify("tmux", &["tmux".into(), "codex".into()]), None);
    assert_eq!(identify("tmux", &["tmux".into(), "omp".into()]), None);
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
fn fallback_providers_complete_only_after_stable_idle() {
    for (provider, working) in [
        (AgentProvider::OpenCode, "esc to interrupt"),
        (AgentProvider::Cursor, "ctrl+c to stop"),
        (AgentProvider::Copilot, "esc to cancel"),
        (AgentProvider::Droid, "esc to stop"),
        (AgentProvider::Pi, "Working..."),
        (AgentProvider::OhMyPi, "  ⎋ Working…"),
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
fn omp_osc_title_states_override_stale_screen_and_progress() {
    let mut detector = Detector::default();
    let now = Instant::now();
    for (elapsed, title, expected, completed) in [
        (0, "π ⠋ Inspect files", AgentState::Working, false),
        (100, "π ⠙ Inspect files", AgentState::Working, false),
        (200, "π ! Inspect files", AgentState::Blocked, false),
        (300, "π ⠹ Inspect files", AgentState::Working, false),
        (400, "π > Inspect files", AgentState::Working, false),
        (700, "π > Inspect files", AgentState::Idle, true),
        (1000, "π > Inspect files", AgentState::Idle, false),
    ] {
        assert_eq!(
            detector.update(
                Some(AgentProvider::OhMyPi),
                "  ⎋ Inspecting files\n π · model · project".into(),
                title,
                "4;1;-1",
                now + Duration::from_millis(elapsed),
            ),
            completed,
            "{title:?}",
        );
        assert_eq!(detector.state, expected, "{title:?}");
    }
}

#[test]
fn omp_title_spinner_variants_do_not_confuse_labels_with_state() {
    for spinner in ["⠋", "⠁", "◔", "-", "\\", "|", "/", ":"] {
        for label in ["", " Inspect files"] {
            let title = format!("π {spinner}{label}");
            assert_eq!(
                detect(
                    AgentProvider::OhMyPi,
                    DetectionInput {
                        screen: "",
                        osc_title: &title,
                        osc_progress: ""
                    },
                ),
                Some((AgentState::Working, false)),
                "{title:?}",
            );
        }
    }
    for title in [
        "π: ⠋ Inspect files",
        "π !important",
        "π >file",
        "π ⠋file",
        "Session π ⠋ Inspect files",
    ] {
        assert_eq!(
            detect(
                AgentProvider::OhMyPi,
                DetectionInput {
                    screen: "",
                    osc_title: title,
                    osc_progress: ""
                },
            ),
            Some((AgentState::Idle, false)),
            "{title:?}",
        );
    }
}

#[test]
fn omp_progress_reports_support_disabled_title_state() {
    let mut detector = Detector::default();
    let now = Instant::now();
    for (elapsed, progress, expected, completed) in [
        (0, "4;1;-1", AgentState::Working, false),
        (1000, "4;1;-1", AgentState::Working, false),
        (1100, "4;0;0", AgentState::Working, false),
        (1800, "4;0;0", AgentState::Idle, true),
        (2000, "4;0;0", AgentState::Idle, false),
    ] {
        assert_eq!(
            detector.update(
                Some(AgentProvider::OhMyPi),
                "Session output".into(),
                "π: Inspect files",
                progress,
                now + Duration::from_millis(elapsed),
            ),
            completed,
        );
        assert_eq!(detector.state, expected);
    }
}

#[test]
fn omp_working_osc_signals_do_not_hide_visible_approval() {
    for screen in [
        "[↑ ↓ to navigate, ⏎ to select, ⎋ to deny]",
        "╭─ Allow tool: bash ─╮\n│ ❯ Approve │\n│ Deny │\n│ ↑ ↓ navigate  ⏎ select  ⎋ cancel │",
    ] {
        assert_eq!(
            detect(
                AgentProvider::OhMyPi,
                DetectionInput {
                    screen,
                    osc_title: "π ⠋ Inspect files",
                    osc_progress: "4;1;-1"
                },
            ),
            Some((AgentState::Blocked, false)),
            "{screen:?}",
        );
    }
}

#[test]
fn omp_headless_progress_completes_with_progress_still_in_history() {
    let mut detector = Detector::default();
    let now = Instant::now();
    for (elapsed, screen, expected, completed) in [
        (0, "Working...\n\n", AgentState::Working, false),
        (1000, "", AgentState::Working, false),
        (1100, "Working...\nAnswer", AgentState::Working, false),
        (1800, "Working...\nAnswer", AgentState::Idle, true),
        (2000, "Working...\nAnswer\n$ ", AgentState::Idle, false),
    ] {
        assert_eq!(
            detector.update(
                Some(AgentProvider::OhMyPi),
                screen.into(),
                "",
                "",
                now + Duration::from_millis(elapsed),
            ),
            completed,
            "{screen:?}"
        );
        assert_eq!(detector.state, expected, "{screen:?}");
    }
}

#[test]
fn omp_trimmed_intent_preserves_work_until_approval_and_completion() {
    let mut detector = Detector::default();
    let now = Instant::now();
    for (elapsed, screen, expected, completed) in [
        (0, "  ⎋ Working…", AgentState::Working, false),
        (100, "  ⎋ Inspecting files ", AgentState::Working, false),
        (200, "  ⎋ Inspecting files", AgentState::Working, false),
        (1000, "  ⎋ Inspecting files", AgentState::Working, false),
        (
            1100,
            "  ⎋ Inspecting files\n[↑ ↓ to navigate, ⏎ to select, ⎋ to deny]",
            AgentState::Blocked,
            false,
        ),
        (1200, "  ⎋ Applying changes", AgentState::Working, false),
        (1300, " π · model · project", AgentState::Working, false),
        (1600, " π · model · project", AgentState::Idle, true),
        (2000, " π · model · project", AgentState::Idle, false),
    ] {
        assert_eq!(
            detector.update(
                Some(AgentProvider::OhMyPi),
                screen.into(),
                "",
                "",
                now + Duration::from_millis(elapsed),
            ),
            completed,
            "{screen:?}"
        );
        assert_eq!(detector.state, expected, "{screen:?}");
    }
}

#[test]
fn omp_current_approval_requests_attention_until_work_resumes() {
    use muxy_protocol::{ActivityKind, AgentActivity, ProjectId};

    let approval = "╭─ Allow tool: bash ───────────────────────────╮\n\
        │ Command: echo hello                         │\n\
        │ ❯ Approve                                   │\n\
        │   Deny                                      │\n\
        │ ↑ ↓ navigate  ⏎ select  ⎋ cancel             │\n\
        ╰─────────────────────────────────────────────╯\n\
          ⎋ Working…\n ⠋ 2s · model · project";
    let deny_selected = approval
        .replace("❯ Approve", "  Approve")
        .replace("  Deny", "❯ Deny")
        .replace("⠋ 2s", "⠙ 3s");
    let mut detector = Detector::default();
    let activity = crate::activity::Activity::default();
    let now = Instant::now();
    for (elapsed, screen, expected, event) in [
        (0, "  ⎋ Working…", AgentState::Working, None),
        (
            100,
            approval,
            AgentState::Blocked,
            Some(ActivityKind::Attention),
        ),
        (
            1000,
            deny_selected.as_str(),
            AgentState::Blocked,
            Some(ActivityKind::Attention),
        ),
        (
            1100,
            "  ⎋ Applying changes",
            AgentState::Working,
            Some(ActivityKind::Attention),
        ),
        (
            1200,
            " π · model · project",
            AgentState::Working,
            Some(ActivityKind::Attention),
        ),
        (
            1500,
            " π · model · project",
            AgentState::Idle,
            Some(ActivityKind::Completed),
        ),
        (
            2000,
            " π · model · project",
            AgentState::Idle,
            Some(ActivityKind::Completed),
        ),
    ] {
        let completed = detector.update(
            Some(AgentProvider::OhMyPi),
            screen.into(),
            "",
            "",
            now + Duration::from_millis(elapsed),
        );
        assert_eq!(detector.state, expected, "{screen:?}");
        assert_eq!(completed, elapsed == 1500, "{screen:?}");
        activity.update(
            AgentActivity {
                session: std::num::NonZeroU64::MIN.into(),
                project: ProjectId::from_u128(1),
                provider: AgentProvider::OhMyPi,
                state: detector.state,
            },
            completed,
        );
        let events: Vec<_> = activity
            .snapshot()
            .events
            .iter()
            .map(|event| event.kind)
            .collect();
        assert_eq!(events, event.into_iter().collect::<Vec<_>>(), "{screen:?}");
    }
}

#[test]
fn omp_generic_selectors_and_approval_text_do_not_request_attention() {
    for screen in [
        "Choose a session\n❯ project-a\n  project-b\n↑ ↓ navigate  ⏎ select  ⎋ cancel",
        "Allow tool: bash\nApprove or Deny the command to continue.\n↑ ↓ navigate  ⏎ select  ⎋ cancel",
        "Allow tool: bash\n❯ Approve\n  Deny\nCommand approved.\n  ⎋ Applying changes",
        "Confirm action\n❯ Approve\n  Deny\n↑ ↓ navigate  ⏎ select  ⎋ cancel",
    ] {
        assert_ne!(
            detect(
                AgentProvider::OhMyPi,
                DetectionInput {
                    screen,
                    osc_title: "",
                    osc_progress: "",
                },
            )
            .map(|result| result.0),
            Some(AgentState::Blocked),
            "{screen:?}"
        );
    }
}

#[test]
fn omp_legacy_spinner_survives_animation_and_completes_once() {
    let mut detector = Detector::default();
    let now = Instant::now();
    for (elapsed, screen, expected, completed) in [
        (0, "⠋ Working… [esc]\nπ › model", AgentState::Working, false),
        (
            1000,
            "⠙ Working… [esc]\nπ › model",
            AgentState::Working,
            false,
        ),
        (1100, "Answer\nπ › model", AgentState::Working, false),
        (1800, "Answer\nπ › model", AgentState::Idle, true),
        (2000, "Answer\nπ › model", AgentState::Idle, false),
    ] {
        assert_eq!(
            detector.update(
                Some(AgentProvider::OhMyPi),
                screen.into(),
                "",
                "",
                now + Duration::from_millis(elapsed),
            ),
            completed,
            "{screen:?}"
        );
        assert_eq!(detector.state, expected, "{screen:?}");
    }
}

#[test]
fn omp_working_mentions_do_not_start_a_cycle() {
    let old_spinner = format!("⠋ Working… [esc]\n{}π › model", "answer\n".repeat(12));
    for screen in [
        "The tool printed Working...",
        "> Working...",
        "Working...\nAnswer\n$ ",
        "  ⎋ ",
        &old_spinner,
    ] {
        assert_eq!(
            detect(
                AgentProvider::OhMyPi,
                DetectionInput {
                    screen,
                    osc_title: "",
                    osc_progress: "",
                },
            ),
            Some((AgentState::Idle, false)),
            "{screen:?}"
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
