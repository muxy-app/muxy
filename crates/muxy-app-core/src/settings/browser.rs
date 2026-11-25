use serde::{Deserialize, Serialize};

use super::{Error, Result};
use crate::browser::{BLANK, SearchEngine, resolve_address, resolve_input};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BrowserSettings {
    pub enabled: bool,
    pub open_terminal_links: bool,
    pub default_profile: String,
    pub search_engine: SearchEngine,
    pub home_page: String,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            open_terminal_links: false,
            default_profile: String::new(),
            search_engine: SearchEngine::Google,
            home_page: String::new(),
        }
    }
}

impl BrowserSettings {
    pub fn validate(&self) -> Result<()> {
        if !self.home_page.trim().is_empty() && self.home_url().is_none() {
            return Err(Error::new(
                "browser.home_page",
                "must be a web address or empty",
            ));
        }
        Ok(())
    }

    pub fn home_url(&self) -> Option<String> {
        let home = self.home_page.trim();
        if home.is_empty() || home == BLANK {
            return None;
        }
        resolve_address(home)
    }

    pub fn resolve(&self, input: &str) -> Option<String> {
        resolve_input(input, self.search_engine)
    }
}
