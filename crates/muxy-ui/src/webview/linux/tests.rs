#![allow(
    clippy::unwrap_used,
    reason = "Tests fail immediately on fixture errors"
)]

use gpui::Keystroke;

#[test]
fn text_is_quoted_for_a_script_without_ending_its_string() {
    use super::page::quoted;
    assert_eq!(quoted("a\"b\\c"), r#""a\"b\\c""#);
    assert_eq!(quoted("line\nbreak\r\t"), r#""line\nbreak\r\t""#);
    assert_eq!(quoted("\u{2028}\u{1}é"), "\"\\u2028\\u0001é\"");
}

#[test]
fn a_paste_is_ctrl_v_or_shift_insert_and_nothing_else() {
    let pastes = |keystroke: &str| super::pastes(&Keystroke::parse(keystroke).unwrap());
    assert!(pastes("ctrl-v"));
    assert!(pastes("shift-insert"));
    for other in ["v", "ctrl-shift-v", "ctrl-alt-v", "insert", "ctrl-c"] {
        assert!(!pastes(other), "{other}");
    }
}
