use super::*;
use gpui::{
    AnyView, AppContext, Context, Entity, ParentElement, Render, Styled, canvas, div, point, px,
    size,
};
use std::{cell::RefCell, rc::Rc};

struct Child {
    renders: usize,
    bounds: Rc<RefCell<Option<Bounds<Pixels>>>>,
}

impl Render for Child {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let bounds = self.bounds.clone();
        canvas(
            move |actual, _, _| *bounds.borrow_mut() = Some(actual),
            |_, (), _, _| {},
        )
        .size_full()
    }
}

struct Host {
    child: Entity<Child>,
    profile: &'static Profile,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Measured {
            child: div()
                .size_full()
                .child(AnyView::from(self.child.clone()).cached(div().size_full().style().clone()))
                .into_any_element(),
            profile: self.profile,
        }
    }
}

#[gpui::test]
fn workspace_measurement_includes_cached_replay_and_preserves_layout(
    cx: &mut gpui::TestAppContext,
) {
    let profile: &'static Profile = Box::leak(Box::new(Profile::new(false)));
    let bounds = Rc::new(RefCell::new(None));
    let (host, cx) = cx.add_window_view(|_, cx| Host {
        child: cx.new(|_| Child {
            renders: 0,
            bounds: bounds.clone(),
        }),
        profile,
    });
    cx.simulate_resize(size(px(300.0), px(200.0)));
    cx.run_until_parked();
    let child = host.read_with(cx, |host, _| host.child.clone());
    let renders = child.read_with(cx, |child, _| child.renders);
    assert_eq!(
        *bounds.borrow(),
        Some(Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(300.0), px(200.0))
        ))
    );
    profile.take();
    for _ in 0..3 {
        host.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    assert_eq!(child.read_with(cx, |child, _| child.renders), renders);
    let metrics = profile.take();
    for metric in [
        "workspace.request_layout",
        "workspace.prepaint",
        "workspace.paint",
    ] {
        assert_eq!(metrics[metric]["calls"], 3);
    }
    cx.simulate_resize(size(px(400.0), px(250.0)));
    cx.run_until_parked();
    assert_eq!(
        *bounds.borrow(),
        Some(Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(400.0), px(250.0))
        ))
    );
    profile.take();
    profile
        .active
        .store(false, std::sync::atomic::Ordering::Relaxed);
    host.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(profile.take()["workspace.paint"]["calls"], 0);
}
