use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

pub const MINIMUM_COLUMNS: u16 = 10;
pub const MINIMUM_ROWS: u16 = 4;
pub const MAXIMUM_COLUMNS: u16 = 4096;
pub const MAXIMUM_ROWS: u16 = 4096;
pub const FALLBACK_COLUMNS: u16 = 80;
pub const FALLBACK_ROWS: u16 = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSize {
    pub columns: u16,
    pub rows: u16,
}

impl WindowSize {
    pub const fn new(columns: u16, rows: u16) -> Self {
        Self { columns, rows }
    }

    pub fn validate(self) -> Result<Self, WindowSizeError> {
        if !(MINIMUM_COLUMNS..=MAXIMUM_COLUMNS).contains(&self.columns)
            || !(MINIMUM_ROWS..=MAXIMUM_ROWS).contains(&self.rows)
        {
            return Err(WindowSizeError);
        }
        Ok(self)
    }

    pub fn create_or_fallback(self) -> Self {
        self.validate()
            .unwrap_or(Self::new(FALLBACK_COLUMNS, FALLBACK_ROWS))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowSizeError;

impl Display for WindowSizeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("terminal window size is out of range")
    }
}

impl std::error::Error for WindowSizeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_size_bounds_and_fallback_are_stable() {
        assert_eq!(
            WindowSize::new(10, 4).validate(),
            Ok(WindowSize::new(10, 4))
        );
        assert_eq!(
            WindowSize::new(4096, 4096).validate(),
            Ok(WindowSize::new(4096, 4096))
        );
        assert!(WindowSize::new(9, 24).validate().is_err());
        assert!(WindowSize::new(80, 3).validate().is_err());
        assert!(WindowSize::new(4097, 24).validate().is_err());
        assert_eq!(
            WindowSize::new(0, 0).create_or_fallback(),
            WindowSize::new(80, 24)
        );
    }
}
