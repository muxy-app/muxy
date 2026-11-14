use std::cell::Cell;
use std::rc::Rc;

use super::*;

fn painted(cols: u16, background: muxy_protocol::Color) -> Vec<Run> {
    vec![Run {
        text: " ".repeat(usize::from(cols)),
        width: cols,
        style: Style {
            bg: background,
            ..Style::default()
        },
    }]
}

fn header_background(
    pane: &gpui::Entity<TerminalPane>,
    cx: &mut gpui::VisualTestContext,
) -> gpui::Hsla {
    pane.read_with(cx, |pane, _| pane.header_colors().0)
}

#[gpui::test]
fn the_header_follows_the_color_a_program_paints_the_screen_in(cx: &mut TestAppContext) {
    let (pane, cx) = cx.add_window_view(|_, cx| {
        let mut pane = TerminalPane::new(
            Palette::new(true),
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        );
        prepare_mouse(&mut pane);
        pane
    });
    let changes = Rc::new(Cell::new(0));
    cx.update(|_, cx| {
        let changes = changes.clone();
        cx.subscribe(&pane, move |_, event, _| {
            if matches!(event, PaneEvent::HeaderBackground) {
                changes.set(changes.get() + 1);
            }
        })
        .detach();
    });
    cx.run_until_parked();
    let terminal: gpui::Hsla = gpui::rgb(Palette::new(true).background).into();
    assert_eq!(header_background(&pane, cx), terminal);
    assert_eq!(changes.get(), 0);

    pane.update(cx, |pane, cx| {
        let grid = pane.grid.as_mut().unwrap();
        let cols = grid.size.cols;
        for row in &mut grid.rows {
            *row = painted(cols, muxy_protocol::Color::Rgb(0x30, 0x40, 0x50));
        }
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(header_background(&pane, cx), gpui::rgb(0x30_40_50).into());
    assert_eq!(changes.get(), 1);

    pane.update(cx, |pane, cx| {
        for row in &mut pane.grid.as_mut().unwrap().rows {
            row.clear();
        }
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(header_background(&pane, cx), terminal);
    assert_eq!(changes.get(), 2);
}

#[gpui::test]
fn the_header_text_stays_readable_on_a_light_app_background(cx: &mut TestAppContext) {
    let palette = Palette::new(true);
    let pane = cx.new(|cx| {
        let mut pane = TerminalPane::new(
            palette,
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        );
        pane.header_background = Some(0xff_ff_ff);
        pane
    });
    pane.read_with(cx, |pane, _| {
        assert_eq!(pane.header_colors().1, gpui::rgb(palette.background).into());
    });
}

#[gpui::test]
fn the_header_names_the_shell_instead_of_its_folder(cx: &mut TestAppContext) {
    let pane = cx.new(|cx| {
        TerminalPane::new(
            Palette::new(true),
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        )
    });
    pane.update(cx, |pane, cx| {
        assert_eq!(pane.header_title(), None);
        pane.metadata(MetadataEvent::Directory("/home/dev/muxy".into()), cx);
        assert_eq!(pane.header_title(), None);
        pane.metadata(
            MetadataEvent::ForegroundProcess {
                name: "-zsh".into(),
                is_shell: true,
            },
            cx,
        );
        assert_eq!(pane.header_title().as_deref(), Some("zsh"));
        pane.metadata(MetadataEvent::Title("~/muxy".into()), cx);
        assert_eq!(pane.header_title().as_deref(), Some("~/muxy"));
    });
}
