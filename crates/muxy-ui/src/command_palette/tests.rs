use super::*;
use crate::theme::ColorScheme;
use gpui::{Modifiers, ParentElement, Styled, TestAppContext, VisualTestContext, div, px, size};

#[test]
fn registration_replaces_ids_and_searches_words_without_adding_row_details() {
    let mut registry = Registry::default();
    registry.register(Command::new("new", "New Tab", 1).keywords("terminal shell"));
    registry.register(Command::new("settings", "Settings", 2));
    assert!(
        registry
            .register(Command::new("settings", "Open Settings", 3))
            .is_some()
    );
    assert_eq!(registry.commands.len(), 2);
    assert!(matches!(registry.commands[1].target, Target::Action(3)));
    let items = registry.items(" SHELL  taB ");
    let [PickerItem::Row(row)] = items.as_slice() else {
        panic!("one matching command");
    };
    assert_eq!(row.id, "new");
    assert!(row.detail.is_none());
    assert!(row.leading.is_none());
    assert!(row.actions.is_empty());
    assert!(registry.items("no match").is_empty());
}

struct Host {
    palette: Entity<CommandPalette<usize>>,
    events: Vec<CommandPaletteEvent<usize>>,
    _subscription: Subscription,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.palette.clone())
    }
}

fn open(
    commands: Registry<usize>,
    cx: &mut TestAppContext,
) -> (Entity<Host>, &mut VisualTestContext) {
    cx.update(|cx| {
        cx.bind_keys(crate::text_input::key_bindings());
        cx.bind_keys(crate::picker::key_bindings());
    });
    let (host, cx) = cx.add_window_view(|_, cx| {
        let palette = cx.new(|cx| {
            CommandPalette::new(
                commands,
                Theme::from_scheme(&ColorScheme::default()),
                Metrics::new(1.0),
                cx,
            )
        });
        let subscription = cx.subscribe(&palette, |host: &mut Host, _, event, _| {
            host.events.push(event.clone());
        });
        Host {
            palette,
            events: Vec::new(),
            _subscription: subscription,
        }
    });
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    cx.run_until_parked();
    (host, cx)
}

#[gpui::test]
fn nested_lists_restore_queries_and_selection_and_build_when_opened(cx: &mut TestAppContext) {
    let count = Rc::new(std::cell::Cell::new(0));
    let calls = count.clone();
    let mut commands = Registry::default();
    commands.register(Command::new("other", "Other Command", 0));
    commands.register(Command::list("projects", "Switch Project", move |_| {
        calls.set(calls.get() + 1);
        let mut projects = Registry::default();
        projects.register(Command::new("disabled", "Disabled", 99).disabled(true));
        projects.register(Command::new("alpha", "Alpha", 1));
        projects.register(Command::list("more", "More Projects", |_| {
            let mut projects = Registry::default();
            projects.register(Command::new("beta", "Beta", 2));
            projects
        }));
        projects
    }));
    let (host, cx) = open(commands, cx);
    assert_eq!(count.get(), 0);
    cx.simulate_input("switch");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(count.get(), 1);
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    cx.simulate_input("bet");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    let palette = host.read_with(cx, |host, _| host.palette.clone());
    palette.read_with(cx, |palette, cx| {
        assert_eq!(palette.parents.len(), 1);
        assert_eq!(palette.picker.read(cx).query(), "");
    });
    cx.simulate_keystrokes("enter enter");
    cx.run_until_parked();
    host.read_with(cx, |host, _| {
        assert!(matches!(
            host.events.as_slice(),
            [CommandPaletteEvent::Selected(2)]
        ));
    });
    cx.simulate_keystrokes("alt-backspace");
    cx.run_until_parked();
    palette.read_with(cx, |palette, _| assert_eq!(palette.parents.len(), 1));
    let back = cx.debug_bounds("picker-back").expect("back button");
    cx.simulate_click(back.center(), Modifiers::default());
    cx.run_until_parked();
    palette.read_with(cx, |palette, cx| {
        assert!(palette.parents.is_empty());
        assert_eq!(palette.picker.read(cx).query(), "switch");
    });
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(count.get(), 2);
    cx.simulate_keystrokes("enter escape escape");
    cx.run_until_parked();
    host.read_with(cx, |host, _| {
        assert!(matches!(
            host.events.as_slice(),
            [
                CommandPaletteEvent::Selected(2),
                CommandPaletteEvent::Selected(1),
                CommandPaletteEvent::Dismissed,
            ]
        ));
    });
}

#[gpui::test]
fn direct_theme_page_has_color_previews_and_escape_dismisses(cx: &mut TestAppContext) {
    let colors = vec![gpui::rgb(0x12_34_56).into(), gpui::rgb(0xab_cd_ef).into()];
    let preview = colors.clone();
    let mut commands = Registry::default();
    commands.register(Command::list("themes", "Change Theme…", move |_| {
        let mut themes = Registry::default();
        themes.register(
            Command::new("fixture", "Fixture", 7)
                .current(true)
                .keep_open()
                .swatches(preview.clone()),
        );
        themes
    }));
    let (host, cx) = open(commands, cx);
    let palette = host.read_with(cx, |host, _| host.palette.clone());
    palette.update(cx, |palette, cx| palette.open_root_page("themes", cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("picker-back").is_none());
    palette.read_with(cx, |palette, _| {
        let [PickerItem::Row(row)] = &palette.page.registry.items("")[..] else {
            panic!("theme row");
        };
        assert_eq!(row.swatches, colors);
        assert!(row.current);
    });
    cx.simulate_keystrokes("enter escape");
    cx.run_until_parked();
    host.read_with(cx, |host, _| {
        assert!(matches!(
            host.events.as_slice(),
            [
                CommandPaletteEvent::Applied(7),
                CommandPaletteEvent::Dismissed
            ]
        ));
    });
}

#[gpui::test]
fn filtering_empty_and_disabled_results_are_safe_and_compact(cx: &mut TestAppContext) {
    let mut commands = Registry::default();
    for index in 0..20 {
        commands.register(Command::new(
            index.to_string(),
            format!("Command {index}"),
            index,
        ));
    }
    commands.register(Command::new("disabled", "Unavailable", 99).disabled(true));
    let (host, cx) = open(commands, cx);
    let bounds = cx.debug_bounds("command-palette").expect("palette");
    assert_eq!(bounds.size, size(px(480.0), px(360.0)));
    cx.simulate_input("Command 19");
    cx.run_until_parked();
    let filtered = cx
        .debug_bounds("command-palette")
        .expect("filtered palette");
    assert!(filtered.size.height <= px(80.0));
    cx.simulate_keystrokes("enter cmd-a");
    cx.simulate_input("Unavailable");
    cx.simulate_keystrokes("enter cmd-a");
    cx.simulate_input("No such command");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    host.read_with(cx, |host, _| {
        assert!(matches!(
            host.events.as_slice(),
            [CommandPaletteEvent::Selected(19)]
        ));
    });
    cx.simulate_resize(size(px(390.0), px(240.0)));
    cx.run_until_parked();
    let bounds = cx.debug_bounds("command-palette").expect("small palette");
    assert!(bounds.right() <= px(390.0));
    assert!(bounds.bottom() <= px(240.0));
}
