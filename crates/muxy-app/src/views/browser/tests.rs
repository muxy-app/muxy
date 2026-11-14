use super::element_clipboard_text;
use muxy_app_core::browser::automation::ELEMENT_TEXT_LIMIT;
use serde_json::json;

#[test]
fn page_results_cannot_bypass_the_text_safety_limit() {
    let text = "😀".repeat(ELEMENT_TEXT_LIMIT + 1);
    let copied = element_clipboard_text(&json!({
        "url": "https://example.com",
        "html": "<p>…</p>",
        "text": text,
    }))
    .expect("valid selection");
    let (_, selected) = copied.split_once("Text:\n").expect("text section");
    assert_eq!(
        selected
            .split_once("\n\n[Text truncated")
            .expect("limit notice")
            .0
            .chars()
            .count(),
        ELEMENT_TEXT_LIMIT
    );
}
