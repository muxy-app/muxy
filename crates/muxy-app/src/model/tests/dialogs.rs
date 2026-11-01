use std::cell::RefCell;
use std::rc::Rc;

use super::*;

#[gpui::test]
fn dialogs_asked_together_are_shown_one_after_the_other(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let confirmed = Rc::new(RefCell::new(Vec::new()));
    view.update(cx, |model, cx| {
        for message in ["first", "second"] {
            let confirmed = confirmed.clone();
            model.confirm("Question", message.into(), "Remove", cx, move |_, _| {
                confirmed.borrow_mut().push(message);
            });
        }
    });
    for message in ["first", "second"] {
        cx.run_until_parked();
        assert_eq!(
            cx.pending_prompt(),
            Some(("Question".into(), message.into()))
        );
        cx.simulate_prompt_answer("Remove");
    }
    cx.run_until_parked();
    assert!(!cx.has_pending_prompt());
    assert_eq!(*confirmed.borrow(), ["first", "second"]);
    assert!(view.read_with(cx, |model, _| !model.dialogs.is_open()));
}

#[gpui::test]
fn dialogs_on_a_closed_window_go_unanswered(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let confirmed = Rc::new(RefCell::new(Vec::new()));
    view.update(cx, |model, cx| {
        for message in ["first", "second"] {
            let confirmed = confirmed.clone();
            model.confirm("Question", message.into(), "Remove", cx, move |_, _| {
                confirmed.borrow_mut().push(message);
            });
        }
        model.window_closed(model.window, cx);
    });
    cx.run_until_parked();
    assert!(!cx.has_pending_prompt());
    assert!(confirmed.borrow().is_empty());
    assert!(view.read_with(cx, |model, _| !model.dialogs.is_open()));
}
