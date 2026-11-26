use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const OFFICIAL_MARKETPLACE: &str = "muxy-app/extensions";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Marketplace(String);

impl Default for Marketplace {
    fn default() -> Self {
        Self(OFFICIAL_MARKETPLACE.to_owned())
    }
}

impl Marketplace {
    pub fn official() -> Self {
        Self::default()
    }

    pub fn parse(input: &str) -> Result<Self, String> {
        let mut text = input.trim();
        for prefix in [
            "https://github.com/",
            "https://www.github.com/",
            "github.com/",
        ] {
            if text.len() >= prefix.len() && text[..prefix.len()].eq_ignore_ascii_case(prefix) {
                text = &text[prefix.len()..];
                break;
            }
        }
        let text = text.strip_suffix('/').unwrap_or(text);
        let text = text.strip_suffix(".git").unwrap_or(text);
        let mut parts = text.split('/');
        let (Some(owner), Some(repo), None) = (parts.next(), parts.next(), parts.next()) else {
            return Err(format!(
                "'{}' is not a GitHub repository like owner/repo",
                input.trim()
            ));
        };
        if !valid_owner(owner) || !valid_repo(repo) {
            return Err(format!(
                "'{}' is not a GitHub repository like owner/repo",
                input.trim()
            ));
        }
        Ok(Self(format!("{owner}/{repo}").to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_official(&self) -> bool {
        self.0 == OFFICIAL_MARKETPLACE
    }

    pub fn index_url(&self) -> String {
        format!(
            "https://raw.githubusercontent.com/{}/index/index.json",
            self.0
        )
    }

    pub fn release_prefix(&self) -> String {
        format!("/{}/releases/download/", self.0)
    }
}

impl std::fmt::Display for Marketplace {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for Marketplace {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Marketplace {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

fn valid_owner(owner: &str) -> bool {
    (1..=39).contains(&owner.len())
        && owner
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && !owner.starts_with('-')
        && !owner.ends_with('-')
}

fn valid_repo(repo: &str) -> bool {
    (1..=100).contains(&repo.len())
        && repo
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && repo != "."
        && repo != ".."
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub(super) struct Sources {
    pub(super) marketplaces: Vec<Marketplace>,
    pub(super) installed: BTreeMap<String, Marketplace>,
}

impl<'de> Deserialize<'de> for Sources {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct Raw {
            marketplaces: Vec<String>,
            installed: BTreeMap<String, String>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let mut marketplaces: Vec<Marketplace> = Vec::new();
        for marketplace in raw
            .marketplaces
            .iter()
            .filter_map(|text| Marketplace::parse(text).ok())
        {
            if !marketplace.is_official() && !marketplaces.contains(&marketplace) {
                marketplaces.push(marketplace);
            }
        }
        let installed = raw
            .installed
            .into_iter()
            .filter_map(|(name, text)| Some((name, Marketplace::parse(&text).ok()?)))
            .filter(|(_, marketplace)| !marketplace.is_official())
            .collect();
        Ok(Self {
            marketplaces,
            installed,
        })
    }
}
