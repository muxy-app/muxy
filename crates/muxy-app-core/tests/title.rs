use muxy_app_core::{AppState, Direction, Pane, PaneContent, PaneId, Tab, title};
use muxy_protocol::{ForegroundProcess, ServerPath};
use serde_json::json;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn terminal_titles_remove_decorations_and_complete_emoji_sequences() {
    let directory = ServerPath(b"/tmp/project".to_vec());
    for (input, expected) in [
        ("◐ Working", "Working"),
        ("◑◒◓⠋⠙✻✶✽ Build", "Build"),
        ("before🚀after", "beforeafter"),
        ("👩🏽‍💻 Develop 🏳️‍🌈", "Develop"),
        ("🇬🇧 Deploy", "Deploy"),
        ("1️⃣#️⃣*️⃣ Tests", "Tests"),
        (
            "🏴\u{e0067}\u{e0062}\u{e0065}\u{e006e}\u{e0067}\u{e007f} Shell",
            "Shell",
        ),
        ("☀︎☀️™️〰️〽️↔️‼️⁉️ Status", "Status"),
        ("\u{e0b0}\u{f120}\u{f0001} Prompt", "Prompt"),
    ] {
        assert_eq!(title::derive(input, None, &directory), expected, "{input}");
    }
}

#[test]
fn terminal_titles_preserve_international_text_and_shell_punctuation() {
    let directory = ServerPath(b"/tmp/project".to_vec());
    for text in [
        "日本語 — café — cafe\u{301} — Ελληνικά — العربية — हिन्दी",
        "क्\u{200d}ष — می\u{200c}روم",
        "~/src/my-project: vim [main] (2) #3 $HOME + tests * 50% @host!",
        "価格 €20 £5 ±2 ×3 ↔ ‼ ⁉",
    ] {
        assert_eq!(title::derive(text, None, &directory), text);
        assert_eq!(
            title::derive(&format!("🚀 {text} ◐"), None, &directory),
            text
        );
    }
    let process = ForegroundProcess {
        name: "oh-my-pi".into(),
        is_shell: false,
    };
    assert_eq!(
        title::derive("π ◐ Inspect files", Some(&process), &directory),
        "Inspect files"
    );
    assert_eq!(title::derive("π = 3.14", None, &directory), "π = 3.14");
}

#[test]
fn decoration_only_titles_follow_sanitized_process_and_directory_fallbacks() {
    let directory = ServerPath("/tmp/🚀 project".as_bytes().to_vec());
    let mut process = ForegroundProcess {
        name: "◐ editor".into(),
        is_shell: false,
    };
    for title in ["◐ 🚀", "👩🏽‍💻", "1️⃣", "🏳️‍🌈", "\u{fe0f}\u{200d}"] {
        assert_eq!(title::derive(title, Some(&process), &directory), "editor");
        assert_eq!(title::derive(title, None, &directory), "project");
    }
    process.is_shell = true;
    assert_eq!(title::derive("◐", Some(&process), &directory), "project");
    process.is_shell = false;
    process.name = "🚀".into();
    assert_eq!(title::derive("◐", Some(&process), &directory), "project");
    assert_eq!(
        title::derive(
            "◐",
            Some(&process),
            &ServerPath("/tmp/🚀".as_bytes().to_vec())
        ),
        "Terminal"
    );
}

#[test]
fn pane_updates_and_custom_titles_are_clean_before_display() -> Result {
    let mut state = AppState::bootstrap()?;
    let tab = state.open_terminal_tab(state.home().id)?;
    let first = state.home().tabs[0].panes[0].id;
    let second = state.split_pane(first, Direction::Right)?;
    state.set_pane_title(first, "◐ Shell")?;
    state.set_pane_title(second, "🚀 日本語")?;
    assert_eq!(state.home().tabs[0].title(Some(first)), "Shell");
    assert_eq!(state.home().tabs[0].title(Some(second)), "日本語");
    state.set_tab_title(tab, Some("👩🏽‍💻 My terminal ◐".into()))?;
    assert_eq!(
        state.home().tabs[0].custom_title.as_deref(),
        Some("My terminal")
    );
    state.set_tab_title(tab, Some("◐ 🚀".into()))?;
    assert!(state.home().tabs[0].custom_title.is_none());
    assert_eq!(state.home().tabs[0].title(Some(second)), "日本語");
    state.set_pane_title(second, "👩🏽‍💻")?;
    assert_eq!(state.home().tabs[0].title(Some(second)), "Terminal");
    Ok(())
}

#[test]
fn restored_terminal_titles_are_filtered_without_changing_other_tab_types() -> Result {
    let mut state = AppState::bootstrap()?;
    state.open_terminal_tab(state.home().id)?;
    let mut saved = serde_json::to_value(&state)?;
    let tab = &mut saved["projects"][0]["tabs"][0];
    tab["custom_title"] = json!("🚀 Custom");
    tab["panes"][0]["title"] = json!("◐ Shell");
    let restored: AppState = serde_json::from_value(saved.clone())?;
    assert_eq!(
        restored.home().tabs[0].custom_title.as_deref(),
        Some("Custom")
    );
    assert_eq!(restored.home().tabs[0].panes[0].title, "Shell");
    let tab = &mut saved["projects"][0]["tabs"][0];
    tab["custom_title"] = json!("🚀");
    tab["panes"][0]["title"] = json!("◐");
    let restored: AppState = serde_json::from_value(saved)?;
    assert!(restored.home().tabs[0].custom_title.is_none());
    assert_eq!(restored.home().tabs[0].panes[0].title, "Terminal");

    let pane = Pane {
        id: PaneId::new(),
        title: "🚀 Settings".into(),
        content: PaneContent::Settings,
        proxy: false,
    };
    let tab: Tab = serde_json::from_value(json!({
        "id": state.home().tabs[0].id,
        "custom_title": " 🚀 Custom ",
        "panes": [pane],
    }))?;
    assert_eq!(tab.custom_title.as_deref(), Some("🚀 Custom"));
    assert_eq!(tab.panes[0].title, "🚀 Settings");
    Ok(())
}
