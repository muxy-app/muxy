use super::*;

pub(super) fn content(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> Vec<AnyElement> {
    let style = modal.style();
    let mut sections = Vec::new();
    sections.extend(visible(
        modal,
        Category::Voice,
        "Voice Recording",
        Some("Use the status-bar microphone or assign the Legacy Voice Recording shortcut to dictate. Muxy inserts the on-device transcript wherever your cursor was before you opened the recorder. If that target is gone, the transcript lands on your clipboard. Composer dictation never auto-sends."),
        true,
        vec![toggle_row(
            style,
            "Press Return after inserting",
            "muxy.recording.autoSend",
            false,
            cx,
        )],
    ));
    let stored = settings::string_value("muxy.recording.language", "");
    sections.extend(visible(
        modal,
        Category::Voice,
        "Language",
        Some("No on-device speech models are installed. Add a dictation language in System Settings → Keyboard → Dictation, then return here."),
        false,
        vec![controls::row(
            style,
            "Language",
            div()
                .text_size(metrics_font(modal))
                .text_color(style.theme.fg_muted)
                .child(SharedString::from(if stored.is_empty() {
                    "None available".to_owned()
                } else {
                    stored
                }))
                .into_any_element(),
        )],
    ));
    sections
}

fn metrics_font(modal: &SettingsModal) -> gpui::Pixels {
    modal.style().metrics.font_body()
}
