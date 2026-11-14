use gpui::AsyncApp;
use muxy_ui::dialog::{Parent, Question, ask};
use muxy_ui::tr;

/// Asks before a Git operation. Reports whether the teardown commands should
/// run too, or `None` when declined.
pub(super) async fn prompt(
    parent: &Parent,
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
    let answer = ask(parent, question, cx).await.ok()??;
    Some(answer.ticked())
}
