use crate::{
    Context, CursorStyle, Entity, InteractiveElement as _, IntoElement, Modifiers,
    ParentElement as _, Render, Styled as _, VisualTestContext, Window, canvas, div, point, px,
};

struct Page {
    window_cursor: Option<CursorStyle>,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let window_cursor = self.window_cursor;
        div()
            .size_full()
            .child(
                div()
                    .id("field")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size(px(40.))
                    .cursor(CursorStyle::IBeam),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        if let Some(cursor) = window_cursor {
                            window.set_window_cursor_style(cursor);
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}

fn open(cx: &mut crate::TestAppContext) -> (Entity<Page>, &mut VisualTestContext) {
    let (page, cx) = cx.add_window_view(|_, _| Page {
        window_cursor: None,
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (page, cx)
}

fn hover_child_view(hovered: bool, cx: &mut VisualTestContext) {
    cx.update(|window, _| {
        window
            .platform_window
            .as_test()
            .expect("a test window")
            .simulate_child_view_hover(hovered);
    });
}

fn move_to(x: f32, cx: &mut VisualTestContext) {
    cx.simulate_mouse_move(point(px(x), px(x)), None, Modifiers::default());
    cx.run_until_parked();
}

#[crate::test]
fn a_child_view_keeps_its_cursor_until_the_mouse_leaves_it(cx: &mut crate::TestAppContext) {
    let (_, cx) = open(cx);
    move_to(200., cx);
    assert_eq!(cx.active_cursor(), CursorStyle::Arrow);

    hover_child_view(true, cx);
    move_to(10., cx);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(
        cx.active_cursor(),
        CursorStyle::Arrow,
        "gpui replaced the cursor of the view above it"
    );

    hover_child_view(false, cx);
    move_to(11., cx);
    assert_eq!(cx.active_cursor(), CursorStyle::IBeam);
}

#[crate::test]
fn a_window_cursor_applies_over_a_child_view(cx: &mut crate::TestAppContext) {
    let (page, cx) = open(cx);
    hover_child_view(true, cx);
    move_to(10., cx);
    page.update(cx, |page, cx| {
        page.window_cursor = Some(CursorStyle::ResizeLeftRight);
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(cx.active_cursor(), CursorStyle::ResizeLeftRight);
}
