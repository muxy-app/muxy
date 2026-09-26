use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::wire::cbor::open_enum;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct Size {
    #[n(0)]
    pub cols: u16,
    #[n(1)]
    pub rows: u16,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum Color {
    #[default]
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum Underline {
    #[default]
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

open_enum! {
    #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
    pub enum CursorShape {
        #[default]
        Block = 0,
        Bar = 1,
        Underline = 2,
        Hollow = 3,
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: Underline,
    pub underline_color: Color,
    pub invisible: bool,
    pub overline: bool,
    pub inverse: bool,
    pub strikethrough: bool,
    pub faint: bool,
}

impl Style {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub text: String,
    pub width: u16,
    pub style: Style,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub index: u16,
    pub runs: Vec<Run>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct Cursor {
    #[n(0)]
    pub shape: CursorShape,
    #[n(1)]
    pub row: u16,
    #[n(2)]
    pub col: u16,
    #[n(3)]
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct Modes {
    #[n(0)]
    pub application_cursor_keys: bool,
    #[n(1)]
    pub bracketed_paste: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ScreenFrame {
    #[n(0)]
    pub size: Size,
    #[n(1)]
    pub graphics: Option<crate::Graphics>,
    #[n(2)]
    pub seq: u64,
    #[n(3)]
    pub reset: bool,
    #[n(4)]
    #[cbor(with = "crate::wire::cbor::rows")]
    pub rows: Vec<Row>,
    #[n(5)]
    pub cursor: Cursor,
    #[n(6)]
    pub modes: Modes,
}
