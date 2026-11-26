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
            if let Some(head) = text.get(..prefix.len())
                && head.eq_ignore_ascii_case(prefix)
            {
                text = &text[prefix.len()..];
                break;
            }
        }
        let text = text.strip_suffix('/').unwrap_or(text);
        let text = match text.len().checked_sub(4).and_then(|at| text.get(at..)) {
            Some(suffix) if suffix.eq_ignore_ascii_case(".git") => &text[..text.len() - 4],
            _ => text,
        };
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
        && !repo.to_ascii_lowercase().ends_with(".git")
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
            marketplaces: serde_json::Value,
            installed: serde_json::Value,
        }
        let raw = Raw::deserialize(deserializer)?;
        let mut marketplaces: Vec<Marketplace> = Vec::new();
        for marketplace in raw
            .marketplaces
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .filter_map(|text| Marketplace::parse(text).ok())
        {
            if !marketplace.is_official() && !marketplaces.contains(&marketplace) {
                marketplaces.push(marketplace);
            }
        }
        let installed = raw
            .installed
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(name, text)| {
                Some((name.clone(), Marketplace::parse(text.as_str()?).ok()?))
            })
            .filter(|(_, marketplace)| !marketplace.is_official())
            .collect();
        Ok(Self {
            marketplaces,
            installed,
        })
    }
}
