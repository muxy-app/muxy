use ghostty_sys::ffi;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseButtonState {
    Release,
    Press,
}

impl MouseButtonState {
    pub(crate) const fn as_raw(self) -> ffi::ghostty_input_mouse_state_e {
        match self {
            Self::Release => ffi::ghostty_input_mouse_state_e_GHOSTTY_MOUSE_RELEASE,
            Self::Press => ffi::ghostty_input_mouse_state_e_GHOSTTY_MOUSE_PRESS,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseButton {
    Unknown,
    Left,
    Right,
    Middle,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Eleven,
}

impl MouseButton {
    pub const fn from_appkit_button_number(button_number: usize) -> Self {
        match button_number {
            0 => Self::Left,
            1 => Self::Right,
            2 => Self::Middle,
            3 => Self::Four,
            4 => Self::Five,
            5 => Self::Six,
            6 => Self::Seven,
            7 => Self::Eight,
            8 => Self::Nine,
            9 => Self::Ten,
            10 => Self::Eleven,
            _ => Self::Unknown,
        }
    }

    pub(crate) const fn as_raw(self) -> ffi::ghostty_input_mouse_button_e {
        match self {
            Self::Unknown => ffi::ghostty_input_mouse_button_e_GHOSTTY_MOUSE_UNKNOWN,
            Self::Left => ffi::ghostty_input_mouse_button_e_GHOSTTY_MOUSE_LEFT,
            Self::Right => ffi::ghostty_input_mouse_button_e_GHOSTTY_MOUSE_RIGHT,
            Self::Middle => ffi::ghostty_input_mouse_button_e_GHOSTTY_MOUSE_MIDDLE,
            Self::Four => ffi::ghostty_input_mouse_button_e_GHOSTTY_MOUSE_FOUR,
            Self::Five => ffi::ghostty_input_mouse_button_e_GHOSTTY_MOUSE_FIVE,
            Self::Six => ffi::ghostty_input_mouse_button_e_GHOSTTY_MOUSE_SIX,
            Self::Seven => ffi::ghostty_input_mouse_button_e_GHOSTTY_MOUSE_SEVEN,
            Self::Eight => ffi::ghostty_input_mouse_button_e_GHOSTTY_MOUSE_EIGHT,
            Self::Nine => ffi::ghostty_input_mouse_button_e_GHOSTTY_MOUSE_NINE,
            Self::Ten => ffi::ghostty_input_mouse_button_e_GHOSTTY_MOUSE_TEN,
            Self::Eleven => ffi::ghostty_input_mouse_button_e_GHOSTTY_MOUSE_ELEVEN,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum MouseMomentum {
    #[default]
    None = 0,
    Began = 1,
    Stationary = 2,
    Changed = 3,
    Ended = 4,
    Cancelled = 5,
    MayBegin = 6,
}

impl MouseMomentum {
    pub const fn from_appkit_phase_bits(phase: u32) -> Self {
        match phase {
            1 => Self::Began,
            2 => Self::Stationary,
            4 => Self::Changed,
            8 => Self::Ended,
            16 => Self::Cancelled,
            32 => Self::MayBegin,
            _ => Self::None,
        }
    }

    const fn packed_bits(self) -> ffi::ghostty_input_scroll_mods_t {
        (self as ffi::ghostty_input_scroll_mods_t) << 1
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScrollMetadata {
    pub precise: bool,
    pub momentum: MouseMomentum,
}

impl ScrollMetadata {
    pub const fn new(precise: bool, momentum: MouseMomentum) -> Self {
        Self { precise, momentum }
    }

    pub const fn packed(self) -> ffi::ghostty_input_scroll_mods_t {
        let precision = if self.precise { 1 } else { 0 };
        precision | self.momentum.packed_bits()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MousePressureStage {
    #[default]
    None,
    Normal,
    Deep,
}

impl MousePressureStage {
    pub(crate) const fn as_raw(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Normal => 1,
            Self::Deep => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseVisibility {
    Visible,
    Hidden,
}

impl MouseVisibility {
    pub(crate) const fn from_raw(raw: ffi::ghostty_action_mouse_visibility_e) -> Option<Self> {
        match raw {
            ffi::ghostty_action_mouse_visibility_e_GHOSTTY_MOUSE_VISIBLE => Some(Self::Visible),
            ffi::ghostty_action_mouse_visibility_e_GHOSTTY_MOUSE_HIDDEN => Some(Self::Hidden),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseShape {
    Default,
    ContextMenu,
    Help,
    Pointer,
    Progress,
    Wait,
    Cell,
    Crosshair,
    Text,
    VerticalText,
    Alias,
    Copy,
    Move,
    NoDrop,
    NotAllowed,
    Grab,
    Grabbing,
    AllScroll,
    ColumnResize,
    RowResize,
    NorthResize,
    EastResize,
    SouthResize,
    WestResize,
    NorthEastResize,
    NorthWestResize,
    SouthEastResize,
    SouthWestResize,
    EastWestResize,
    NorthSouthResize,
    NorthEastSouthWestResize,
    NorthWestSouthEastResize,
    ZoomIn,
    ZoomOut,
}

impl MouseShape {
    pub(crate) const fn from_raw(raw: ffi::ghostty_action_mouse_shape_e) -> Option<Self> {
        match raw {
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_DEFAULT => Some(Self::Default),
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_CONTEXT_MENU => {
                Some(Self::ContextMenu)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_HELP => Some(Self::Help),
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_POINTER => Some(Self::Pointer),
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_PROGRESS => Some(Self::Progress),
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_WAIT => Some(Self::Wait),
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_CELL => Some(Self::Cell),
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_CROSSHAIR => {
                Some(Self::Crosshair)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_TEXT => Some(Self::Text),
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_VERTICAL_TEXT => {
                Some(Self::VerticalText)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_ALIAS => Some(Self::Alias),
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_COPY => Some(Self::Copy),
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_MOVE => Some(Self::Move),
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_NO_DROP => Some(Self::NoDrop),
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_NOT_ALLOWED => {
                Some(Self::NotAllowed)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_GRAB => Some(Self::Grab),
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_GRABBING => Some(Self::Grabbing),
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_ALL_SCROLL => {
                Some(Self::AllScroll)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_COL_RESIZE => {
                Some(Self::ColumnResize)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_ROW_RESIZE => {
                Some(Self::RowResize)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_N_RESIZE => {
                Some(Self::NorthResize)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_E_RESIZE => {
                Some(Self::EastResize)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_S_RESIZE => {
                Some(Self::SouthResize)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_W_RESIZE => {
                Some(Self::WestResize)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_NE_RESIZE => {
                Some(Self::NorthEastResize)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_NW_RESIZE => {
                Some(Self::NorthWestResize)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_SE_RESIZE => {
                Some(Self::SouthEastResize)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_SW_RESIZE => {
                Some(Self::SouthWestResize)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_EW_RESIZE => {
                Some(Self::EastWestResize)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_NS_RESIZE => {
                Some(Self::NorthSouthResize)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_NESW_RESIZE => {
                Some(Self::NorthEastSouthWestResize)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_NWSE_RESIZE => {
                Some(Self::NorthWestSouthEastResize)
            }
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_ZOOM_IN => Some(Self::ZoomIn),
            ffi::ghostty_action_mouse_shape_e_GHOSTTY_MOUSE_SHAPE_ZOOM_OUT => Some(Self::ZoomOut),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appkit_button_numbers_map_exhaustively() {
        let expected = [
            MouseButton::Left,
            MouseButton::Right,
            MouseButton::Middle,
            MouseButton::Four,
            MouseButton::Five,
            MouseButton::Six,
            MouseButton::Seven,
            MouseButton::Eight,
            MouseButton::Nine,
            MouseButton::Ten,
            MouseButton::Eleven,
        ];
        for (button_number, button) in expected.into_iter().enumerate() {
            assert_eq!(
                MouseButton::from_appkit_button_number(button_number),
                button
            );
            assert_eq!(button.as_raw() as usize, button_number + 1);
        }
        assert_eq!(
            MouseButton::from_appkit_button_number(11),
            MouseButton::Unknown
        );
        assert_eq!(
            MouseButton::from_appkit_button_number(usize::MAX),
            MouseButton::Unknown
        );
        assert_eq!(MouseButton::Unknown.as_raw(), 0);
    }

    #[test]
    fn mouse_button_states_match_the_pinned_abi() {
        assert_eq!(
            MouseButtonState::Release.as_raw(),
            ffi::ghostty_input_mouse_state_e_GHOSTTY_MOUSE_RELEASE
        );
        assert_eq!(
            MouseButtonState::Press.as_raw(),
            ffi::ghostty_input_mouse_state_e_GHOSTTY_MOUSE_PRESS
        );
    }

    #[test]
    fn every_scroll_metadata_combination_has_exact_packed_bits() {
        let phases = [
            MouseMomentum::None,
            MouseMomentum::Began,
            MouseMomentum::Stationary,
            MouseMomentum::Changed,
            MouseMomentum::Ended,
            MouseMomentum::Cancelled,
            MouseMomentum::MayBegin,
        ];
        for (index, momentum) in phases.into_iter().enumerate() {
            for precise in [false, true] {
                let packed = ScrollMetadata::new(precise, momentum).packed();
                assert_eq!(packed, ((index as i32) << 1) | i32::from(precise));
                assert_eq!(packed & !0b1111, 0);
            }
        }
    }

    #[test]
    fn appkit_momentum_phase_bits_map_exhaustively() {
        assert_eq!(
            MouseMomentum::from_appkit_phase_bits(0),
            MouseMomentum::None
        );
        assert_eq!(
            MouseMomentum::from_appkit_phase_bits(1),
            MouseMomentum::Began
        );
        assert_eq!(
            MouseMomentum::from_appkit_phase_bits(2),
            MouseMomentum::Stationary
        );
        assert_eq!(
            MouseMomentum::from_appkit_phase_bits(4),
            MouseMomentum::Changed
        );
        assert_eq!(
            MouseMomentum::from_appkit_phase_bits(8),
            MouseMomentum::Ended
        );
        assert_eq!(
            MouseMomentum::from_appkit_phase_bits(16),
            MouseMomentum::Cancelled
        );
        assert_eq!(
            MouseMomentum::from_appkit_phase_bits(32),
            MouseMomentum::MayBegin
        );
        assert_eq!(
            MouseMomentum::from_appkit_phase_bits(3),
            MouseMomentum::None
        );
    }

    #[test]
    fn cursor_shape_decoder_covers_the_complete_pinned_enum() {
        let expected = [
            MouseShape::Default,
            MouseShape::ContextMenu,
            MouseShape::Help,
            MouseShape::Pointer,
            MouseShape::Progress,
            MouseShape::Wait,
            MouseShape::Cell,
            MouseShape::Crosshair,
            MouseShape::Text,
            MouseShape::VerticalText,
            MouseShape::Alias,
            MouseShape::Copy,
            MouseShape::Move,
            MouseShape::NoDrop,
            MouseShape::NotAllowed,
            MouseShape::Grab,
            MouseShape::Grabbing,
            MouseShape::AllScroll,
            MouseShape::ColumnResize,
            MouseShape::RowResize,
            MouseShape::NorthResize,
            MouseShape::EastResize,
            MouseShape::SouthResize,
            MouseShape::WestResize,
            MouseShape::NorthEastResize,
            MouseShape::NorthWestResize,
            MouseShape::SouthEastResize,
            MouseShape::SouthWestResize,
            MouseShape::EastWestResize,
            MouseShape::NorthSouthResize,
            MouseShape::NorthEastSouthWestResize,
            MouseShape::NorthWestSouthEastResize,
            MouseShape::ZoomIn,
            MouseShape::ZoomOut,
        ];
        for (raw, shape) in expected.into_iter().enumerate() {
            assert_eq!(MouseShape::from_raw(raw as u32), Some(shape));
        }
        assert_eq!(MouseShape::from_raw(expected.len() as u32), None);
    }
}
