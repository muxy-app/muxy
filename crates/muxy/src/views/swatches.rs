use gpui::Rgba;
use muxy_core::store::ICON_PALETTE;
use muxy_ui::theme::{contrasting_foreground, parse_hex};

pub fn icon_color(identifier: Option<&str>) -> Option<Rgba> {
    let identifier = identifier?;
    let swatch = ICON_PALETTE
        .iter()
        .find(|swatch| swatch.id == identifier || swatch.hex.eq_ignore_ascii_case(identifier))?;
    parse_hex(swatch.hex)
}

pub fn icon_foreground(identifier: Option<&str>) -> Option<Rgba> {
    icon_color(identifier).map(contrasting_foreground)
}
