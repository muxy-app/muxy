//! QR codes for pairing links, shared by the desktop and the terminal client.

/// Modules outside the code that scanners need to find it.
pub const QUIET_ZONE: usize = 4;

/// A square grid of modules; `true` is dark.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QrCode {
    size: usize,
    modules: Vec<bool>,
}

impl QrCode {
    /// Returns `None` when the text is too long for any QR version.
    pub fn encode(text: &str) -> Option<Self> {
        let code = qrcodegen::QrCode::encode_text(text, qrcodegen::QrCodeEcc::Medium).ok()?;
        let code = &code;
        let modules = (0..code.size())
            .flat_map(|y| (0..code.size()).map(move |x| code.get_module(x, y)))
            .collect();
        Some(Self {
            size: usize::try_from(code.size()).ok()?,
            modules,
        })
    }

    /// Modules per side, without the quiet zone.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Whether the module is dark; positions outside the code are light.
    pub fn dark(&self, x: usize, y: usize) -> bool {
        x < self.size && y < self.size && self.modules[y * self.size + x]
    }
}
