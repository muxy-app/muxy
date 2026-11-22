use super::*;

fn composer_text(view: &Entity<AppModel>, cx: &VisualTestContext) -> String {
    view.read_with(cx, |model, cx| {
        model
            .composer
            .view
            .as_ref()
            .expect("composer")
            .read(cx)
            .draft
            .text
            .clone()
    })
}

fn attached_composer(
    cx: &mut TestAppContext,
) -> (
    Entity<AppModel>,
    &mut VisualTestContext,
    std::sync::mpsc::Receiver<(u64, Work)>,
) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let pane = state.home().tabs[0].panes[0].id;
    let session = SessionId::new(12).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    let (mut boot, requests) = stub_boot(state);
    boot.settings.composer.clear_after_sending = true;
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.receive(
            (
                ServerId::local(),
                1,
                Update::Attached {
                    pane,
                    session,
                    attachment: attachment(),
                    created: false,
                },
            ),
            cx,
        );
    });
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-i"
    } else {
        "ctrl-shift-i"
    });
    (view, cx, requests)
}

fn delivery(
    requests: &std::sync::mpsc::Receiver<(u64, Work)>,
) -> (Vec<u8>, async_channel::Sender<Result<(), String>>) {
    requests
        .try_iter()
        .find_map(|(_, work)| match work {
            Work::WriteInput {
                bytes, completion, ..
            } => Some((bytes, completion)),
            _ => None,
        })
        .expect("acknowledged terminal input")
}

#[gpui::test]
fn composer_delivery_preserves_edits_and_failures_then_clears_confirmed_draft(
    cx: &mut TestAppContext,
) {
    let (view, cx, requests) = attached_composer(cx);
    cx.simulate_input("first");
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-enter"
    });
    cx.run_until_parked();
    let (bytes, completion) = delivery(&requests);
    assert_eq!(bytes, b"\x15first\r");
    cx.simulate_input(" edited");
    completion.try_send(Ok(())).expect("ack");
    cx.run_until_parked();
    assert_eq!(composer_text(&view, cx), "first edited");
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-shift-enter"
    } else {
        "ctrl-shift-enter"
    });
    cx.run_until_parked();
    let (bytes, completion) = delivery(&requests);
    assert_eq!(bytes, b"\x15first edited");
    completion
        .try_send(Err("writer failed".into()))
        .expect("failure");
    cx.run_until_parked();
    assert_eq!(composer_text(&view, cx), "first edited");
    view.read_with(cx, |model, cx| {
        assert!(
            model
                .composer
                .view
                .as_ref()
                .expect("composer")
                .read(cx)
                .error
                .as_ref()
                .expect("error")
                .contains("Draft preserved")
        );
    });
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-enter"
    });
    cx.run_until_parked();
    delivery(&requests).1.try_send(Ok(())).expect("ack");
    cx.run_until_parked();
    assert_eq!(composer_text(&view, cx), "");
}

#[gpui::test]
fn composer_broadcast_deduplicates_sessions_and_retains_partial_failure(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let first = state.home().tabs[0].panes[0].id;
    let second = state.split_pane(first, Direction::Right).expect("split");
    let third = state.split_pane(second, Direction::Right).expect("split");
    for (pane, id) in [(first, 31), (second, 31), (third, 32)] {
        state
            .set_pane_session(pane, SessionId::new(id))
            .expect("session");
    }
    let (mut boot, requests) = stub_boot(state);
    boot.settings.composer.broadcast = true;
    boot.settings.composer.clear_after_sending = true;
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        for (index, pane) in [first, second, third].into_iter().enumerate() {
            let attachment = attachment_on(u32::try_from(index + 1).expect("channel"));
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Attached {
                        pane,
                        session: model.state.pane_session(pane).expect("session").1,
                        attachment,
                        created: false,
                    },
                ),
                cx,
            );
        }
    });
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-i"
    } else {
        "ctrl-shift-i"
    });
    cx.simulate_input("broadcast");
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-enter"
    });
    cx.run_until_parked();
    delivery(&requests).1.try_send(Ok(())).expect("first ack");
    cx.run_until_parked();
    delivery(&requests)
        .1
        .try_send(Err("second writer failed".into()))
        .expect("second failure");
    cx.run_until_parked();
    assert_eq!(composer_text(&view, cx), "broadcast");
    view.read_with(cx, |model, cx| {
        let panel = model.composer.view.as_ref().expect("composer").read(cx);
        assert!(
            panel
                .error
                .as_ref()
                .expect("partial result")
                .starts_with("1/2 terminals sent.")
        );
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::WriteInput { .. }))
    );
}

#[gpui::test]
fn switching_panes_clears_the_composer_error(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let first = state.home().tabs[0].panes[0].id;
    let second = state.split_pane(first, Direction::Right).expect("split");
    state.focus_pane(first).expect("focus");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        model.toggle_composer(window, cx);
        let composer = model.composer.view.clone().expect("composer");
        composer.update(cx, |composer, _| {
            composer.error = Some("Open a live terminal before sending.".into());
        });

        model.focus_pane(second, cx);
        model.sync_composer(cx);
        assert_eq!(composer.read(cx).error, None);
    });
}

#[gpui::test]
fn stored_clipboard_strategy_sends_an_image_path_without_a_clipboard_backend(
    cx: &mut TestAppContext,
) {
    use muxy_app_core::composer::submission::ImageSubmissionStrategy;
    use muxy_core::capability::{Capability, Feature};

    let (view, cx, requests) = attached_composer(cx);
    let mut png = Vec::new();
    image::RgbaImage::new(1, 1)
        .write_to(&mut io::Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("png");
    view.update(cx, |model, cx| {
        model
            .capabilities
            .set(Feature::ClipboardImages, Capability::Unsupported);
        let composer = model.composer.view.clone().expect("composer");
        composer.update(cx, |composer, cx| {
            composer.settings.image_strategy = ImageSubmissionStrategy::Clipboard;
            cx.emit(crate::views::composer::ComposerEvent::Image(png));
        });
    });
    cx.run_until_parked();
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-enter"
    });
    cx.run_until_parked();
    let (bytes, completion) = delivery(&requests);
    assert!(!bytes.contains(&0x16));
    assert!(String::from_utf8_lossy(&bytes).contains(".png"));
    completion.try_send(Ok(())).expect("ack");
    cx.run_until_parked();
}

#[gpui::test]
fn the_app_drawn_composer_menu_opens_at_its_button_and_hands_the_keyboard_back(
    cx: &mut TestAppContext,
) {
    use crate::views::overlays::Overlay;
    use gpui::Focusable;

    let (view, cx, _requests) = attached_composer(cx);
    let button = gpui::Bounds::new(gpui::point(px(120.0), px(300.0)), size(px(20.0), px(20.0)));
    let composer = view.update(cx, |model, cx| {
        let composer = model.composer.view.clone().expect("composer");
        composer.update(cx, |composer, _| composer.sending = true);
        let choices = composer.read(cx).menu_items();
        model.open_composer_menu(&choices, button, cx);
        let Some(Overlay::Menu(menu)) = &model.overlay else {
            panic!("the composer menu is not open");
        };
        assert_eq!(menu.drops_from, Some(button));
        composer
    });

    cx.simulate_keystrokes("down down");
    cx.simulate_keystrokes("enter");
    view.update_in(cx, |model, window, cx| {
        assert!(model.overlay.is_none());
        assert!(!model.settings.composer.clear_after_sending);
        assert!(composer.focus_handle(cx).is_focused(window));
    });

    view.update(cx, |model, cx| {
        let choices = composer.read(cx).menu_items();
        model.open_composer_menu(&choices, button, cx);
    });
    cx.simulate_keystrokes("escape");
    view.update_in(cx, |model, window, cx| {
        assert!(model.overlay.is_none());
        assert!(composer.focus_handle(cx).is_focused(window));
    });
}

#[gpui::test]
fn a_floating_composer_closed_by_its_menu_row_leaves_the_keyboard_to_the_terminal(
    cx: &mut TestAppContext,
) {
    use crate::views::composer::menu::Command;

    let (view, cx, _requests) = attached_composer(cx);
    let button = gpui::Bounds::new(gpui::point(px(120.0), px(300.0)), size(px(20.0), px(20.0)));
    view.update(cx, |model, cx| {
        let composer = model.composer.view.clone().expect("composer");
        composer.update(cx, |composer, cx| {
            composer.menu_action(Command::Presentation, cx);
        });
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        let composer = model.composer.view.clone().expect("composer");
        let choices = composer.read(cx).menu_items();
        model.open_composer_menu(&choices, button, cx);
        model.dismiss_overlay(cx);
        model.dismiss_composer(false, cx);
        assert!(model.composer.closing.is_some());
    });
    cx.run_until_parked();
    cx.executor()
        .advance_clock(crate::views::composer::TRANSITION * 2);
    cx.run_until_parked();
    view.update_in(cx, |model, window, cx| {
        let pane = model.active_pane().expect("pane");
        let terminal = model.terminal(&pane).expect("terminal");
        assert!(terminal.view.read(cx).focus.is_focused(window));
    });

    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-i"
    } else {
        "ctrl-shift-i"
    });
    view.update(cx, |model, cx| {
        let composer = model.composer.view.clone().expect("composer");
        let choices = composer.read(cx).menu_items();
        model.open_composer_menu(&choices, button, cx);
        model.close_composer(cx);
        assert!(model.overlay.is_none());
    });
}

#[gpui::test]
fn a_pinned_composer_keeps_its_saved_size_and_scales_with_the_interface(cx: &mut TestAppContext) {
    let (view, cx, _requests) = attached_composer(cx);
    cx.simulate_resize(size(px(2400.0), px(1600.0)));
    let composer = view.update_in(cx, |model, window, cx| {
        if model.composer.view.is_none() {
            model.toggle_composer(window, cx);
        }
        model.composer.view.clone().expect("composer")
    });
    cx.run_until_parked();
    let stored = |composer: &crate::views::composer::Composer| {
        if composer.settings.position == muxy_app_core::settings::ComposerPosition::Right {
            composer.settings.width
        } else {
            composer.settings.height
        }
    };
    composer.update(cx, |composer, _| composer.settings.pinned = true);
    let saved = composer.read_with(cx, |composer, _| stored(composer));
    let theme = view.read_with(cx, |model, _| model.theme.clone());
    composer.update(cx, |composer, cx| {
        composer.appearance(theme, Metrics::new(1.5), cx);
    });
    cx.update(|window, cx| {
        let composer = composer.read(cx);
        assert_eq!(
            composer.sizing(window).layout().dimension().to_bits(),
            (saved * 1.5).to_bits()
        );
        assert_eq!(stored(composer).to_bits(), saved.to_bits());
    });
}

#[gpui::test]
fn a_pinned_composer_leaves_room_for_the_workspace_in_a_small_window(cx: &mut TestAppContext) {
    let (view, cx, _requests) = attached_composer(cx);
    let width = 640.0;
    let sidebar = 252.0;
    cx.simulate_resize(size(px(width), px(400.0)));
    let composer = view.update_in(cx, |model, window, cx| {
        if model.composer.view.is_none() {
            model.toggle_composer(window, cx);
        }
        model.composer.view.clone().expect("composer")
    });
    cx.run_until_parked();
    let theme = view.read_with(cx, |model, _| model.theme.clone());
    cx.update(|window, cx| {
        composer.update(cx, |composer, cx| {
            composer.settings.position = muxy_app_core::settings::ComposerPosition::Right;
            composer.settings.pinned = true;
            composer.workspace_inset = sidebar;
            composer.appearance(theme, Metrics::new(1.40), cx);
            let consumed = composer.sizing(window).layout().consumed_width();
            assert!(consumed > 0.0, "the composer is not pinned");
            assert!(
                consumed < width - sidebar,
                "the composer takes {consumed} of the {} left",
                width - sidebar,
            );
        });
    });
}

#[gpui::test]
fn enhancing_an_empty_or_whitespace_composer_stops_before_calling_a_cli(cx: &mut TestAppContext) {
    let (view, cx, _requests) = attached_composer(cx);
    view.update(cx, |model, cx| {
        model.enhance_prompt(cx);
        assert!(model.composer.enhancing.is_none());
        assert!(
            model
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("empty"))
        );
        model.notice = None;
    });
    cx.simulate_input("   \n\t  ");
    view.update(cx, |model, cx| {
        model.enhance_prompt(cx);
        assert!(model.composer.enhancing.is_none());
        assert!(
            model
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("empty"))
        );
    });
}

#[gpui::test]
fn enhancing_reports_provider_and_model_errors_as_error_toasts_and_leaves_the_draft(
    cx: &mut TestAppContext,
) {
    let (view, cx, _requests) = attached_composer(cx);
    cx.simulate_input("fix the login bug");
    view.update(cx, |model, cx| {
        assert!(model.ai.installed.is_empty());
        model.enhance_prompt(cx);
        assert!(model.composer.enhancing.is_none());
        assert!(model.notice.is_none());
        assert!(
            model
                .visible_error()
                .is_some_and(|error| error.contains("installed"))
        );

        model.ai.installed = vec![crate::ai::provider("claude").expect("claude")];
        model
            .settings
            .ai
            .providers
            .insert(crate::prompt::PROMPT_PROVIDER.into(), "codex".into());
        model.enhance_prompt(cx);
        assert!(model.composer.enhancing.is_none());
        assert!(
            model
                .visible_error()
                .is_some_and(|error| error.contains("Codex"))
        );

        model
            .settings
            .ai
            .providers
            .insert(crate::prompt::PROMPT_PROVIDER.into(), "nope".into());
        model.enhance_prompt(cx);
        assert!(model.composer.enhancing.is_none());
        assert!(
            model
                .visible_error()
                .is_some_and(|error| error.contains("nope"))
        );

        model.ai.installed = vec![crate::ai::provider("xal").expect("xal")];
        model
            .settings
            .ai
            .providers
            .insert(crate::prompt::PROMPT_PROVIDER.into(), "xal".into());
        model
            .settings
            .ai
            .prompts
            .insert(crate::prompt::PROMPT_MODEL.into(), "big".into());
        model.enhance_prompt(cx);
        assert!(model.composer.enhancing.is_none());
        assert!(
            model
                .visible_error()
                .is_some_and(|error| error.contains("no confirmed model flag"))
        );

        model.dismiss_problem(cx);
        assert!(model.visible_error().is_none());
        model.enhance_prompt(cx);
        assert!(
            model
                .visible_error()
                .is_some_and(|error| error.contains("no confirmed model flag"))
        );
    });
    assert_eq!(composer_text(&view, cx), "fix the login bug");
}

#[gpui::test]
fn enhancing_rejects_a_second_request_while_one_is_running(cx: &mut TestAppContext) {
    let (view, cx, _requests) = attached_composer(cx);
    cx.simulate_input("fix the login bug");
    view.update(cx, |model, cx| {
        let key = model.composer_key();
        let (id, _) = model.start_prompt_enhancement(&key, cx);
        model.enhance_prompt(cx);
        assert_eq!(
            model.composer.enhancing.as_ref().map(|active| active.id),
            Some(id)
        );
        assert!(
            model
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("already being rewritten"))
        );
    });
}

#[gpui::test]
fn finishing_prompt_enhancement_previews_then_appends_or_discards_and_keeps_edits(
    cx: &mut TestAppContext,
) {
    let (view, cx, _requests) = attached_composer(cx);
    let attachment = if cfg!(windows) {
        r"C:\tmp\example.rs"
    } else {
        "/tmp/example.rs"
    };
    cx.simulate_input("fix bug");
    let key = view.update(cx, |model, cx| {
        let composer = model.composer.view.clone().expect("composer");
        composer.update(cx, |_, cx| {
            cx.emit(crate::views::composer::ComposerEvent::Files(vec![
                PathBuf::from(attachment),
            ]));
        });
        model.composer_key()
    });
    cx.run_until_parked();

    let id = view.update(cx, |model, cx| model.start_prompt_enhancement(&key, cx).0);
    cx.simulate_input(" now");
    view.update(cx, |model, cx| {
        model.finish_prompt_enhancement(id, &key, "fix bug", Ok("rewritten".into()), cx);
        assert!(model.composer.enhancing.is_none());
        assert!(model.composer.enhancement_preview.is_none());
        assert!(
            model
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("changed"))
        );
    });
    assert_eq!(composer_text(&view, cx), "fix bug now");

    view.update(cx, |model, cx| {
        let id = model.start_prompt_enhancement(&key, cx).0;
        model.finish_prompt_enhancement(id, &key, "fix bug now", Ok("  \n\t ".into()), cx);
        assert!(model.composer.enhancement_preview.is_none());
        assert!(
            model
                .visible_error()
                .is_some_and(|error| error.contains("empty"))
        );
    });
    assert_eq!(composer_text(&view, cx), "fix bug now");

    view.update(cx, |model, cx| {
        let id = model.start_prompt_enhancement(&key, cx).0;
        model.finish_prompt_enhancement(
            id,
            &key,
            "fix bug now",
            Ok("  Fix the login bug cleanly.\n".into()),
            cx,
        );
        let composer = model.composer.view.as_ref().expect("composer").read(cx);
        assert_eq!(composer.draft.text, "fix bug now");
        assert_eq!(
            composer.enhanced_preview.as_deref(),
            Some("Fix the login bug cleanly.")
        );
        model.dismiss_prompt_enhancement(cx);
        let composer = model.composer.view.as_ref().expect("composer").read(cx);
        assert_eq!(composer.draft.text, "fix bug now");
        assert!(composer.enhanced_preview.is_none());

        let id = model.start_prompt_enhancement(&key, cx).0;
        model.finish_prompt_enhancement(
            id,
            &key,
            "fix bug now",
            Ok("  Fix the login bug cleanly.\n".into()),
            cx,
        );
        model.apply_prompt_enhancement(true, cx);
        let composer = model.composer.view.as_ref().expect("composer").read(cx);
        assert_eq!(
            composer.draft.text,
            "fix bug now\n\nFix the login bug cleanly."
        );
        assert_eq!(
            composer.input.read(cx).text(),
            "fix bug now\n\nFix the login bug cleanly."
        );
        assert!(composer.enhanced_preview.is_none());
        assert_eq!(composer.draft.file_attachments, vec![attachment.to_owned()]);
    });
}

#[gpui::test]
fn finishing_prompt_enhancement_while_closed_restores_preview_on_reopen_and_replaces(
    cx: &mut TestAppContext,
) {
    let (view, cx, _requests) = attached_composer(cx);
    let attachment = if cfg!(windows) {
        r"C:\tmp\example.rs"
    } else {
        "/tmp/example.rs"
    };
    cx.simulate_input("fix bug");
    let key = view.update(cx, |model, cx| {
        let composer = model.composer.view.clone().expect("composer");
        composer.update(cx, |_, cx| {
            cx.emit(crate::views::composer::ComposerEvent::Files(vec![
                PathBuf::from(attachment),
            ]));
        });
        model.composer_key()
    });
    cx.run_until_parked();

    view.update(cx, |model, cx| {
        model.settings.composer.clear_on_close = false;
        model.close_composer(cx);
        assert!(model.composer.view.is_none());
        let id = model.start_prompt_enhancement(&key, cx).0;
        model.finish_prompt_enhancement(
            id,
            &key,
            "fix bug",
            Ok("  Final rewritten prompt\n".into()),
            cx,
        );
        assert_eq!(model.composer_text(cx), "fix bug");
        assert!(
            model
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("ready"))
        );
    });
    view.update_in(cx, |model, window, cx| {
        model.toggle_composer(window, cx);
        let composer = model.composer.view.as_ref().expect("composer").read(cx);
        assert_eq!(
            composer.enhanced_preview.as_deref(),
            Some("Final rewritten prompt")
        );
        model.apply_prompt_enhancement(false, cx);
        let composer = model.composer.view.as_ref().expect("composer").read(cx);
        assert_eq!(composer.draft.text, "Final rewritten prompt");
        assert!(composer.enhanced_preview.is_none());
        assert_eq!(composer.draft.file_attachments, vec![attachment.to_owned()]);
    });
}

#[gpui::test]
fn replacing_with_an_enhanced_prompt_preserves_pasted_image_attachments(cx: &mut TestAppContext) {
    let (view, cx, _requests) = attached_composer(cx);
    let mut png = Vec::new();
    image::RgbaImage::new(1, 1)
        .write_to(&mut io::Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("png");
    cx.simulate_input("fix layout ");
    view.update(cx, |model, cx| {
        let composer = model.composer.view.clone().expect("composer");
        composer.update(cx, |_, cx| {
            cx.emit(crate::views::composer::ComposerEvent::Image(png));
        });
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        let key = model.composer_key();
        let original = model.composer_text(cx);
        assert!(original.contains("[Image 1]"));
        let id = model.start_prompt_enhancement(&key, cx).0;
        model.finish_prompt_enhancement(
            id,
            &key,
            &original,
            Ok("Fix the layout issue cleanly.".into()),
            cx,
        );
        model.apply_prompt_enhancement(false, cx);
        let composer = model.composer.view.as_ref().expect("composer").read(cx);
        assert_eq!(composer.draft.image_attachments.len(), 1);
        assert!(
            composer
                .draft
                .text
                .contains("Fix the layout issue cleanly.")
        );
        assert!(composer.draft.text.contains("[Image 1]"));
    });
}

#[gpui::test]
fn submitting_the_composer_cancels_only_its_own_in_flight_prompt_enhancement(
    cx: &mut TestAppContext,
) {
    let (view, cx, requests) = attached_composer(cx);
    cx.simulate_input("ship it");
    let other_key =
        muxy_app_core::composer::DraftId::for_project(ServerId::local(), ProjectId::new());
    let (other_id, other_cancellation) = view.update(cx, |model, cx| {
        model.start_prompt_enhancement(&other_key, cx)
    });
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-enter"
    });
    cx.run_until_parked();
    let (_, completion) = delivery(&requests);
    view.update(cx, |model, _| {
        assert!(!other_cancellation.is_cancelled());
        assert_eq!(
            model.composer.enhancing.as_ref().map(|active| active.id),
            Some(other_id)
        );
    });
    completion.try_send(Ok(())).expect("ack");
    cx.run_until_parked();

    cx.simulate_input("ship again");
    let (id, cancellation, key) = view.update(cx, |model, cx| {
        model.composer.enhancing = None;
        let key = model.composer_key();
        let (id, cancellation) = model.start_prompt_enhancement(&key, cx);
        (id, cancellation, key)
    });
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-enter"
    });
    cx.run_until_parked();
    let (_, completion) = delivery(&requests);
    view.update(cx, |model, cx| {
        assert!(cancellation.is_cancelled());
        assert!(model.composer.enhancing.is_none());
        model.finish_prompt_enhancement(id, &key, "ship again", Ok("late rewrite".into()), cx);
    });
    assert_eq!(composer_text(&view, cx), "ship again");
    completion.try_send(Ok(())).expect("ack");
    cx.run_until_parked();
    assert_eq!(composer_text(&view, cx), "");
}

#[gpui::test]
fn editing_the_draft_while_a_preview_is_open_dismisses_the_stale_preview(cx: &mut TestAppContext) {
    let (view, cx, _requests) = attached_composer(cx);
    cx.simulate_input("fix login");
    view.update(cx, |model, cx| {
        let key = model.composer_key();
        let id = model.start_prompt_enhancement(&key, cx).0;
        model.finish_prompt_enhancement(
            id,
            &key,
            "fix login",
            Ok("Fix the login bug cleanly.".into()),
            cx,
        );
        let composer = model.composer.view.as_ref().expect("composer").read(cx);
        assert_eq!(
            composer.enhanced_preview.as_deref(),
            Some("Fix the login bug cleanly.")
        );
    });
    cx.simulate_input(" and signup");
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        let composer = model.composer.view.as_ref().expect("composer").read(cx);
        assert!(composer.enhanced_preview.is_none());
        assert!(model.composer.enhancement_preview.is_none());
        assert_eq!(composer.draft.text, "fix login and signup");
    });
}
