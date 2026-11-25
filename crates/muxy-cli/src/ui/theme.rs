use muxy_app_core::activity::ActivityIndicator;
use ratatui::style::{Color, Modifier, Style};

pub(crate) const ACCENT: Color = Color::Blue;
const CONTRAST: Color = Color::Black;
const LINE: Color = Color::DarkGray;

pub(crate) fn text() -> Style {
    Style::new()
}

pub(crate) fn strong() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

pub(crate) fn muted() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}

pub(crate) fn accent() -> Style {
    Style::new().fg(ACCENT)
}

pub(crate) fn key() -> Style {
    accent().add_modifier(Modifier::BOLD)
}

pub(crate) fn line() -> Style {
    Style::new().fg(LINE)
}

pub(crate) fn pill() -> Style {
    Style::new()
        .fg(CONTRAST)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn danger() -> Style {
    Style::new().fg(Color::Red)
}

pub(crate) fn danger_pill() -> Style {
    pill().bg(Color::Red)
}

pub(crate) fn resize_pill() -> Style {
    pill().bg(Color::Magenta)
}

pub(crate) fn button() -> Style {
    Style::new()
        .add_modifier(Modifier::REVERSED)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn warning() -> Style {
    Style::new().fg(Color::Yellow)
}

pub(crate) fn connected(online: bool) -> Style {
    Style::new().fg(if online { Color::Green } else { Color::Yellow })
}

pub(crate) fn activity(indicator: ActivityIndicator) -> Option<(&'static str, Style)> {
    let (symbol, color) = match indicator {
        ActivityIndicator::None => return None,
        ActivityIndicator::Idle => ("○", Color::Green),
        ActivityIndicator::Completed => ("●", Color::Cyan),
        ActivityIndicator::Working => ("●", Color::Yellow),
        ActivityIndicator::Blocked => ("●", Color::Red),
    };
    Some((symbol, Style::new().fg(color)))
}

pub(crate) fn activity_label(indicator: ActivityIndicator) -> &'static str {
    match indicator {
        ActivityIndicator::None | ActivityIndicator::Idle => "idle",
        ActivityIndicator::Completed => "done",
        ActivityIndicator::Working => "working",
        ActivityIndicator::Blocked => "needs you",
    }
}

pub(crate) fn project(color: &str) -> Style {
    let channel = |range: std::ops::Range<usize>| {
        color
            .get(range)
            .and_then(|hex| u8::from_str_radix(hex, 16).ok())
    };
    match (
        color.starts_with('#'),
        channel(1..3),
        channel(3..5),
        channel(5..7),
    ) {
        (true, Some(red), Some(green), Some(blue)) if color.len() == 7 => {
            Style::new().fg(Color::Rgb(red, green, blue))
        }
        _ => accent(),
    }
}
