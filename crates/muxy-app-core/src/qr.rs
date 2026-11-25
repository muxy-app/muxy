pub const QUIET_ZONE: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QrCode {
    size: usize,
    modules: Vec<bool>,
}

impl QrCode {
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

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn dark(&self, x: usize, y: usize) -> bool {
        x < self.size && y < self.size && self.modules[y * self.size + x]
    }
}
