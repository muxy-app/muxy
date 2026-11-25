use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[doc = " The fallback fonts that can be configured for a given font."]
#[doc = " Fallback fonts family names are stored here."]
#[derive(Default, Clone, Eq, PartialEq, Hash, Debug, Deserialize, Serialize, JsonSchema)]
pub struct FontFallbacks(pub Arc<Vec<String>>);

impl FontFallbacks {
    pub fn fallback_list(&self) -> &[String] {
        self.0.as_slice()
    }

    pub fn from_fonts(fonts: Vec<String>) -> Self {
        FontFallbacks(Arc::new(fonts))
    }
}
