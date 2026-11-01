use gpui::{AnyWindowHandle, AsyncApp};
use muxy_ui::dialog::{Question, ask};
use muxy_ui::tr;

/// Asks before a Git operation. Reports whether the teardown commands should
/// run too, or `None` when declined.
pub(super) async fn prompt(
    window: AnyWindowHandle,
    message: &str,
    has_hooks: bool,
    cx: &mut AsyncApp,
) -> Option<bool> {
    let teardown = tr!("Run the teardown commands shown above");
    let question = Question::Confirm {
        title: &tr!("Confirm Git Operation"),
        message,
        confirm: &tr!("Confirm"),
        checkbox: has_hooks.then_some(teardown.as_ref()),
    };
    let answer = ask(window, question, cx).await.ok()??;
    Some(answer.ticked())
}
