//! Handling various protocols pioneered by Kitty,
//! including the [Kitty graphics protocol](graphics).

pub mod graphics;

#[cfg(feature = "kitty-graphics")]
pub use graphics::Graphics;
