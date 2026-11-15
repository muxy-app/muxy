#[derive(Clone, Debug)]
pub struct Item {
    pub label: String,
    pub enabled: bool,
    pub checked: bool,
    pub separator: bool,
}

impl Item {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            enabled: true,
            checked: false,
            separator: false,
        }
    }
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod unsupported;
#[cfg(target_os = "macos")]
pub use macos::show;
#[cfg(not(target_os = "macos"))]
pub use unsupported::show;
