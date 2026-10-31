use muxy_protocol::Modes;

pub(crate) fn contents(item: &gpui::ClipboardItem, modes: Modes) -> Option<Vec<u8>> {
    if let Some(text) = item.text() {
        return Some(muxy_client::input::paste(&text, modes));
    }
    item.entries()
        .iter()
        .any(|entry| matches!(entry, gpui::ClipboardEntry::Image(_)))
        .then(|| vec![0x16])
}
