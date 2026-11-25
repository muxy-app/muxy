use serde::{Deserialize, Serialize};

use crate::settings::OpenerSettings;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRule {
    pub pattern: String,
    pub opener: String,
}

impl FileRule {
    pub fn validate(&self) -> Result<(), String> {
        if self.pattern.trim().is_empty()
            || self.pattern.len() > 1024
            || self.pattern.chars().any(char::is_control)
        {
            return Err(
                muxy_core::tr!("Enter a file pattern, such as *.md or Dockerfile.").to_string(),
            );
        }
        if self.opener != "ask"
            && self.opener != "system.finder"
            && !self
                .opener
                .split_once(':')
                .is_some_and(|(owner, id)| !owner.is_empty() && !id.is_empty())
        {
            return Err(
                muxy_core::tr!("Choose an extension, the file manager, or Always ask.").to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileHandler {
    pub id: String,
    pub title: String,
    pub patterns: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileResolution {
    System,
    Handler(String),
    Choose(Vec<String>),
}

pub fn resolve_file(
    path: &str,
    handlers: &[FileHandler],
    settings: &OpenerSettings,
) -> FileResolution {
    let rules: Vec<_> = settings
        .file_rules
        .iter()
        .filter(|rule| rule.validate().is_ok() && matches_pattern(&rule.pattern, path))
        .collect();
    let available: Vec<_> = handlers
        .iter()
        .filter(|handler| {
            handler
                .patterns
                .iter()
                .any(|pattern| matches_pattern(pattern, path))
                || rules.iter().any(|rule| rule.opener == handler.id)
        })
        .map(|handler| handler.id.clone())
        .collect();
    let rule = rules.into_iter().find(|rule| {
        matches!(rule.opener.as_str(), "ask" | "system.finder") || available.contains(&rule.opener)
    });
    if !settings.always_ask
        && let Some(rule) = rule
    {
        match rule.opener.as_str() {
            "system.finder" => return FileResolution::System,
            "ask" => (),
            id => return FileResolution::Handler(id.to_owned()),
        }
    }
    match available.as_slice() {
        [] => FileResolution::System,
        [id] if !settings.always_ask && rule.is_none_or(|rule| rule.opener != "ask") => {
            FileResolution::Handler(id.clone())
        }
        _ => FileResolution::Choose(available),
    }
}

pub fn file_pattern(path: &str, handlers: &[FileHandler]) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.starts_with('.')
        || handlers
            .iter()
            .flat_map(|handler| &handler.patterns)
            .any(|pattern| !pattern.contains(['*', '?', '/']) && matches_pattern(pattern, name))
    {
        return name.to_lowercase();
    }
    if let Some(pattern) = handlers
        .iter()
        .flat_map(|handler| &handler.patterns)
        .filter(|pattern| {
            pattern
                .strip_prefix("*.")
                .is_some_and(|suffix| !suffix.is_empty() && !suffix.contains(['*', '?', '/']))
                && matches_pattern(pattern, name)
        })
        .max_by_key(|pattern| pattern.len())
    {
        return pattern.to_lowercase();
    }
    name.rsplit_once('.')
        .filter(|(stem, suffix)| !stem.is_empty() && !suffix.is_empty())
        .map_or_else(
            || name.to_lowercase(),
            |(_, suffix)| format!("*.{}", suffix.to_lowercase()),
        )
}

pub fn matches_pattern(pattern: &str, relative: &str) -> bool {
    if pattern.is_empty() || relative.is_empty() {
        return false;
    }
    let text = if pattern.contains('/') {
        relative
    } else {
        relative.rsplit('/').next().unwrap_or(relative)
    };
    glob(
        &pattern.to_lowercase().chars().collect::<Vec<_>>(),
        &text.to_lowercase().chars().collect::<Vec<_>>(),
    )
}

fn glob(pattern: &[char], text: &[char]) -> bool {
    let (mut p, mut t) = (0, 0);
    let mut resume: Option<(usize, usize)> = None;
    while t < text.len() {
        if p < pattern.len() && pattern[p] == '*' {
            resume = Some((p, t));
            p += 1;
        } else if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if let Some((star, matched)) = resume {
            p = star + 1;
            t = matched + 1;
            resume = Some((star, matched + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|character| *character == '*')
}
