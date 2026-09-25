//! Taps and scrolling for programs that read the mouse, such as editors and pagers.

use muxy_protocol::{MouseAction, MouseEvent};

use crate::keys::Modifiers;

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum ScrollDirection {
    Up,
    Down,
}

/// A press and then a release at one cell.
pub(crate) fn click(
    button: MouseButton,
    row: u16,
    column: u16,
    modifiers: Modifiers,
) -> [MouseEvent; 2] {
    let press = MouseEvent {
        action: MouseAction::Press,
        button: Some(match button {
            MouseButton::Left => muxy_protocol::MouseButton::Left,
            MouseButton::Middle => muxy_protocol::MouseButton::Middle,
            MouseButton::Right => muxy_protocol::MouseButton::Right,
        }),
        column,
        row,
        scroll: None,
        modifiers: muxy_protocol::Modifiers {
            shift: modifiers.shift,
            alt: modifiers.alt,
            ctrl: modifiers.control,
        },
    };
    [
        press,
        MouseEvent {
            action: MouseAction::Release,
            ..press
        },
    ]
}

/// One mouse-wheel step at a cell.
pub(crate) fn scroll(direction: ScrollDirection, row: u16, column: u16) -> MouseEvent {
    MouseEvent {
        action: MouseAction::Scroll,
        button: None,
        column,
        row,
        scroll: Some(match direction {
            ScrollDirection::Up => muxy_protocol::ScrollDirection::Up,
            ScrollDirection::Down => muxy_protocol::ScrollDirection::Down,
        }),
        modifiers: muxy_protocol::Modifiers::default(),
    }
}

#[cfg(test)]
mod tests {
    use muxy_protocol::Message;

    use super::*;

    #[test]
    fn clicks_press_and_release_the_same_button_with_its_modifiers() {
        let control = Modifiers {
            control: true,
            ..Modifiers::default()
        };
        let [press, release] = click(MouseButton::Right, 2, 4, control);
        assert_eq!(press.action, MouseAction::Press);
        assert_eq!(press.button, Some(muxy_protocol::MouseButton::Right));
        assert_eq!((press.row, press.column), (2, 4));
        assert!(press.modifiers.ctrl && !press.modifiers.shift && !press.modifiers.alt);
        assert_eq!(
            release,
            MouseEvent {
                action: MouseAction::Release,
                ..press
            }
        );
    }

    #[test]
    fn every_click_and_scroll_is_a_valid_mouse_message() {
        let buttons = [MouseButton::Left, MouseButton::Middle, MouseButton::Right];
        let scrolls =
            [ScrollDirection::Up, ScrollDirection::Down].map(|direction| scroll(direction, 0, 0));
        let events = buttons
            .into_iter()
            .flat_map(|button| click(button, 0, 0, Modifiers::default()))
            .chain(scrolls);
        for event in events {
            assert_eq!(Message::Mouse(event).validate(), Ok(()), "{event:?}");
        }
    }
}
