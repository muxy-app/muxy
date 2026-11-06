//! Reads iTerm2 inline image files (`OSC 1337 ; File=`).

use std::io::Cursor;

use base64::Engine as _;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};

use super::{Picture, Placement};

const BASE64: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);
/// No placement is wider or taller than this many cells.
const MAX_CELLS: u32 = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Length {
    Auto,
    Cells(u32),
    Pixels(u32),
    Percent(u32),
}

/// An inline image and how iTerm2 asks to size it.
#[derive(Debug)]
pub(crate) struct File {
    pub(crate) picture: Picture,
    width: Length,
    height: Length,
    preserve_aspect_ratio: bool,
    pub(crate) move_cursor: bool,
}

/// Decodes `key=value;…:base64`. Files that aren't inline images, such as
/// downloads, give `None`.
pub(crate) fn parse(file: &[u8]) -> Option<File> {
    let split = file.iter().position(|&b| b == b':')?;
    let (arguments, payload) = (&file[..split], &file[split + 1..]);
    let mut inline = false;
    let mut width = Length::Auto;
    let mut height = Length::Auto;
    let mut preserve_aspect_ratio = true;
    let mut move_cursor = true;
    for argument in arguments.split(|&b| b == b';') {
        let Some(equals) = argument.iter().position(|&b| b == b'=') else {
            continue;
        };
        let (key, value) = (&argument[..equals], &argument[equals + 1..]);
        match key {
            b"inline" => inline = value == b"1",
            b"width" => width = length(value),
            b"height" => height = length(value),
            b"preserveAspectRatio" => preserve_aspect_ratio = value != b"0",
            b"doNotMoveCursor" => move_cursor = value != b"1",
            _ => {}
        }
    }
    if !inline {
        return None;
    }
    let payload: Vec<u8> = payload
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let bytes = BASE64.decode(payload).ok()?;
    Some(File {
        picture: picture(&bytes)?,
        width,
        height,
        preserve_aspect_ratio,
        move_cursor,
    })
}

impl File {
    /// The cells to fill, for a terminal of `columns` by `rows` cells of
    /// `cell` pixels. A missing side keeps the image's aspect ratio. When both
    /// sides are set and the ratio is kept, only the side that limits the
    /// image stays, so it fits inside the box as in iTerm2.
    pub(crate) fn placement(&self, columns: u16, rows: u16, cell: crate::CellSize) -> Placement {
        let mut width = cells(self.width, cell.width, columns);
        let mut height = cells(self.height, cell.height, rows);
        if let (Some(w), Some(h), true) = (width, height, self.preserve_aspect_ratio) {
            let box_width = u64::from(w) * u64::from(cell.width);
            let box_height = u64::from(h) * u64::from(cell.height);
            if u64::from(self.picture.width) * box_height
                >= u64::from(self.picture.height) * box_width
            {
                height = None;
            } else {
                width = None;
            }
        }
        Placement {
            columns: width,
            rows: height,
        }
    }
}

fn length(value: &[u8]) -> Length {
    let text = std::str::from_utf8(value).unwrap_or_default();
    let number = |digits: &str| digits.parse::<u32>().ok();
    let parsed = if let Some(digits) = text.strip_suffix("px") {
        number(digits).map(Length::Pixels)
    } else if let Some(digits) = text.strip_suffix('%') {
        number(digits).map(Length::Percent)
    } else {
        number(text).map(Length::Cells)
    };
    parsed.unwrap_or(Length::Auto)
}

fn cells(length: Length, cell: u16, terminal: u16) -> Option<u32> {
    let cells = match length {
        Length::Auto => return None,
        Length::Cells(cells) => cells,
        Length::Pixels(pixels) => pixels.div_ceil(u32::from(cell.max(1))),
        Length::Percent(percent) => u32::from(terminal) * percent.min(100) / 100,
    };
    (cells > 0).then_some(cells.min(MAX_CELLS))
}

fn picture(bytes: &[u8]) -> Option<Picture> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let decoded = super::decode_image(reader)?;
    Some(Picture {
        width: decoded.width(),
        height: decoded.height(),
        rgba: decoded.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CELL: crate::CellSize = crate::CellSize {
        width: 10,
        height: 20,
    };

    fn file(width: u32, height: u32, arguments: &str) -> File {
        let parse_length = |key: &str| {
            arguments
                .split(';')
                .find_map(|pair| pair.strip_prefix(key))
                .map_or(Length::Auto, |value| length(value.as_bytes()))
        };
        File {
            picture: Picture {
                width,
                height,
                rgba: Vec::new(),
            },
            width: parse_length("width="),
            height: parse_length("height="),
            preserve_aspect_ratio: !arguments.contains("preserveAspectRatio=0"),
            move_cursor: true,
        }
    }

    #[test]
    fn lengths_in_cells_pixels_and_percent() {
        assert_eq!(length(b"5"), Length::Cells(5));
        assert_eq!(length(b"25px"), Length::Pixels(25));
        assert_eq!(length(b"50%"), Length::Percent(50));
        assert_eq!(length(b"auto"), Length::Auto);
        assert_eq!(cells(Length::Pixels(25), 10, 80), Some(3));
        assert_eq!(cells(Length::Percent(50), 10, 80), Some(40));
        assert_eq!(cells(Length::Cells(0), 10, 80), None);
    }

    #[test]
    fn keeps_aspect_ratio_inside_the_box() {
        // 100x100 pixels in a 4x4 cell box (40x80 px): the width limits it.
        let placement = file(100, 100, "width=4;height=4").placement(80, 24, CELL);
        assert_eq!((placement.columns, placement.rows), (Some(4), None));
        // 100x400 pixels in a 4x1 cell box (40x20 px): the height limits it.
        let placement = file(100, 400, "width=4;height=1").placement(80, 24, CELL);
        assert_eq!((placement.columns, placement.rows), (None, Some(1)));
        let placement =
            file(100, 100, "width=4;height=4;preserveAspectRatio=0").placement(80, 24, CELL);
        assert_eq!((placement.columns, placement.rows), (Some(4), Some(4)));
        let placement = file(100, 100, "").placement(80, 24, CELL);
        assert_eq!((placement.columns, placement.rows), (None, None));
    }

    #[test]
    fn parses_inline_png_and_ignores_downloads() {
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNg+M/wHwAEAQH/cetH5QAAAABJRU5ErkJggg==";
        let parsed = parse(format!("inline=1;doNotMoveCursor=1:{png}").as_bytes());
        let parsed = parsed
            .as_ref()
            .map(|file| (&file.picture.rgba[..], file.move_cursor));
        assert_eq!(parsed, Some((&[0, 255, 0, 255][..], false)));
        assert!(parse(format!("name=YQ==:{png}").as_bytes()).is_none());
        assert!(parse(b"inline=1:not an image").is_none());
    }
}
