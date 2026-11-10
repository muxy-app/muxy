use super::*;

fn graphics(id: u32, generation: u64, pixels: bool) -> muxy_protocol::Graphics {
    muxy_protocol::Graphics {
        images: vec![muxy_protocol::GraphicImage {
            id,
            generation,
            width: 1,
            height: 1,
            rgba: if pixels {
                vec![0; 4].into()
            } else {
                std::sync::Arc::default()
            },
            ..muxy_protocol::GraphicImage::default()
        }],
        ..muxy_protocol::Graphics::default()
    }
}

fn frame(pane: &TerminalPane, seq: u64, graphics: muxy_protocol::Graphics) -> ScreenFrame {
    let grid = pane.grid.as_ref().unwrap();
    ScreenFrame {
        size: grid.size,
        graphics: Some(graphics),
        seq,
        reset: false,
        rows: Vec::new(),
        cursor: grid.cursor,
        modes: Modes::default(),
    }
}

#[gpui::test]
fn a_frame_that_brings_pixels_is_acknowledged_once_they_can_be_drawn(cx: &mut TestAppContext) {
    let pane = cx.new(|cx| {
        let mut pane = TerminalPane::new(
            Palette::new(true),
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        );
        prepare_mouse(&mut pane);
        pane
    });
    let cx = cx.add_empty_window();
    let acks = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    cx.update(|_, cx| {
        let acks = acks.clone();
        cx.subscribe(&pane, move |_, event, _| {
            if let PaneEvent::Ack(_, seq) = event {
                acks.borrow_mut().push(*seq);
            }
        })
        .detach();
    });
    pane.update(cx, |pane, cx| {
        assert_eq!(pane.apply(&frame(pane, 1, graphics(1, 1, true)), cx), None);
    });
    cx.run_until_parked();
    assert_eq!(*acks.borrow(), [1]);

    // A later screen that lists the image without its pixels keeps them,
    // and has nothing to wait for.
    pane.update(cx, |pane, cx| {
        assert_eq!(
            pane.apply(&frame(pane, 2, graphics(1, 1, false)), cx),
            Some(2)
        );
        let grid = pane.displayed_grid().unwrap();
        assert!(!grid.graphics.images[0].rgba.is_empty());
    });
    cx.run_until_parked();
    assert_eq!(*acks.borrow(), [1]);
}
