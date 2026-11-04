use std::ops::RangeInclusive;

pub const DEFAULT_WIDTH: u16 = 720;
pub const WIDTH: RangeInclusive<u16> = 480..=1200;
pub const DEFAULT_HEIGHT: u16 = 430;
pub const HEIGHT: RangeInclusive<u16> = 280..=800;
pub const DEFAULT_TRANSPARENCY: u8 = 18;
pub const TRANSPARENCY: RangeInclusive<u8> = 0..=55;
pub const DEFAULT_BLUR: u8 = 70;
pub const BLUR: RangeInclusive<u8> = 0..=100;
