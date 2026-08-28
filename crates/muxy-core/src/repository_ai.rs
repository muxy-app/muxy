use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};

pub const COMMIT_PROVIDER_KEY: &str = "muxy.ai.repositoryActions.commit.provider";
pub const COMMIT_PROMPT_KEY: &str = "muxy.ai.repositoryActions.commit.prompt";
pub const CREATE_PULL_REQUEST_PROVIDER_KEY: &str =
    "muxy.ai.repositoryActions.createPullRequest.provider";
pub const CREATE_PULL_REQUEST_PROMPT_KEY: &str =
    "muxy.ai.repositoryActions.createPullRequest.prompt";
pub const COMMIT_PROMPT: &str = "Write a concise commit message that explains the intent of all staged changes. Follow the repository's existing commit-message style.";
pub const CREATE_PULL_REQUEST_PROMPT: &str = "Write an accurate pull request title and a concise summary of the changes. Choose a short descriptive branch name and the appropriate target branch.";
pub const CONFIGURED_PROMPT_BYTE_LIMIT: usize = 16 * 1_024;
pub const ADDITIONAL_PROMPT_CHARACTER_LIMIT: usize = 2_000;
pub const ADDITIONAL_PROMPT_BYTE_LIMIT: usize = 8 * 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub icon_key: &'static str,
    pub executable_names: &'static [&'static str],
    pub home_relative_bins: &'static [&'static str],
    pub headless_arguments: &'static [&'static str],
    pub model_argument: Option<&'static str>,
    pub environment: &'static [(&'static str, &'static str)],
}

pub const PROVIDERS: [ProviderDescriptor; 11] = [
    ProviderDescriptor {
        id: "claude",
        display_name: "Claude Code",
        icon_key: "claude",
        executable_names: &["claude"],
        home_relative_bins: &[".local/bin"],
        headless_arguments: &[
            "--print",
            "--output-format",
            "text",
            "--permission-mode",
            "dontAsk",
            "--no-session-persistence",
            "--tools=",
        ],
        model_argument: Some("--model"),
        environment: &[],
    },
    ProviderDescriptor {
        id: "opencode",
        display_name: "OpenCode",
        icon_key: "opencode",
        executable_names: &["opencode"],
        home_relative_bins: &[".opencode/bin", ".local/bin"],
        headless_arguments: &["run", "--pure"],
        model_argument: Some("--model"),
        environment: &[("OPENCODE_PERMISSION", r#"{"*":"deny"}"#)],
    },
    ProviderDescriptor {
        id: "codex",
        display_name: "Codex",
        icon_key: "codex",
        executable_names: &["codex"],
        home_relative_bins: &[".local/bin", ".npm-global/bin"],
        headless_arguments: &[
            "exec",
            "--ephemeral",
            "--sandbox",
            "read-only",
            "--color",
            "never",
        ],
        model_argument: Some("--model"),
        environment: &[],
    },
    ProviderDescriptor {
        id: "cursor",
        display_name: "Cursor CLI",
        icon_key: "cursor",
        executable_names: &["cursor-agent"],
        home_relative_bins: &[".local/bin"],
        headless_arguments: &["--print", "--output-format", "text"],
        model_argument: Some("--model"),
        environment: &[],
    },
    ProviderDescriptor {
        id: "copilot",
        display_name: "GitHub Copilot",
        icon_key: "copilot",
        executable_names: &["copilot"],
        home_relative_bins: &[".local/bin", ".npm-global/bin"],
        headless_arguments: &["--silent", "--no-ask-user", "--available-tools=", "-p"],
        model_argument: None,
        environment: &[],
    },
    ProviderDescriptor {
        id: "droid",
        display_name: "Droid",
        icon_key: "factory",
        executable_names: &["droid"],
        home_relative_bins: &[".factory/bin", ".local/bin"],
        headless_arguments: &["exec", "--output-format", "text"],
        model_argument: Some("--model"),
        environment: &[],
    },
    ProviderDescriptor {
        id: "pi",
        display_name: "Pi",
        icon_key: "pi",
        executable_names: &["pi"],
        home_relative_bins: &[".local/bin"],
        headless_arguments: &["--print", "--no-session", "--no-tools"],
        model_argument: Some("--model"),
        environment: &[],
    },
    ProviderDescriptor {
        id: "grok",
        display_name: "Grok",
        icon_key: "grok",
        executable_names: &["grok"],
        home_relative_bins: &[".local/bin"],
        headless_arguments: &[
            "--no-auto-update",
            "--sandbox",
            "workspace",
            "--permission-mode",
            "dontAsk",
            "--no-subagents",
            "--disable-web-search",
            "--output-format",
            "plain",
            "-p",
        ],
        model_argument: Some("--model"),
        environment: &[],
    },
    ProviderDescriptor {
        id: "kiro",
        display_name: "Kiro CLI",
        icon_key: "kiro",
        executable_names: &["kiro-cli"],
        home_relative_bins: &[".local/bin"],
        headless_arguments: &["chat", "--no-interactive", "--trust-tools="],
        model_argument: Some("--model"),
        environment: &[],
    },
    ProviderDescriptor {
        id: "xal",
        display_name: "Xal",
        icon_key: "xal",
        executable_names: &["xal"],
        home_relative_bins: &[".local/bin"],
        headless_arguments: &["run", "--format", "text"],
        model_argument: Some("--model"),
        environment: &[],
    },
    ProviderDescriptor {
        id: "antigravity",
        display_name: "Antigravity CLI",
        icon_key: "antigravity",
        executable_names: &["agy", "antigravity"],
        home_relative_bins: &[".local/bin", ".gemini/antigravity-cli/bin"],
        headless_arguments: &["--print", "--output-format", "text", "--mode=plan"],
        model_argument: Some("--model"),
        environment: &[],
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RepositoryAiAction {
    Commit,
    CreatePullRequest,
}

impl RepositoryAiAction {
    pub fn provider_key(self) -> &'static str {
        match self {
            Self::Commit => COMMIT_PROVIDER_KEY,
            Self::CreatePullRequest => CREATE_PULL_REQUEST_PROVIDER_KEY,
        }
    }

    pub fn prompt_key(self) -> &'static str {
        match self {
            Self::Commit => COMMIT_PROMPT_KEY,
            Self::CreatePullRequest => CREATE_PULL_REQUEST_PROMPT_KEY,
        }
    }

    pub fn default_prompt(self) -> &'static str {
        match self {
            Self::Commit => COMMIT_PROMPT,
            Self::CreatePullRequest => CREATE_PULL_REQUEST_PROMPT,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepositoryAiActionPreferences {
    pub provider: String,
    pub prompt: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepositoryAiPreferences {
    pub commit: RepositoryAiActionPreferences,
    pub create_pull_request: RepositoryAiActionPreferences,
}

impl Default for RepositoryAiPreferences {
    fn default() -> Self {
        Self {
            commit: RepositoryAiActionPreferences {
                provider: String::new(),
                prompt: COMMIT_PROMPT.to_owned(),
            },
            create_pull_request: RepositoryAiActionPreferences {
                provider: String::new(),
                prompt: CREATE_PULL_REQUEST_PROMPT.to_owned(),
            },
        }
    }
}

impl RepositoryAiPreferences {
    pub fn load() -> Self {
        Self {
            commit: RepositoryAiActionPreferences {
                provider: crate::prefs::settings::string_value(COMMIT_PROVIDER_KEY, ""),
                prompt: crate::prefs::settings::string_value(COMMIT_PROMPT_KEY, COMMIT_PROMPT),
            },
            create_pull_request: RepositoryAiActionPreferences {
                provider: crate::prefs::settings::string_value(
                    CREATE_PULL_REQUEST_PROVIDER_KEY,
                    "",
                ),
                prompt: crate::prefs::settings::string_value(
                    CREATE_PULL_REQUEST_PROMPT_KEY,
                    CREATE_PULL_REQUEST_PROMPT,
                ),
            },
        }
    }

    pub fn action(&self, action: RepositoryAiAction) -> &RepositoryAiActionPreferences {
        match action {
            RepositoryAiAction::Commit => &self.commit,
            RepositoryAiAction::CreatePullRequest => &self.create_pull_request,
        }
    }

    pub fn action_mut(&mut self, action: RepositoryAiAction) -> &mut RepositoryAiActionPreferences {
        match action {
            RepositoryAiAction::Commit => &mut self.commit,
            RepositoryAiAction::CreatePullRequest => &mut self.create_pull_request,
        }
    }

    pub fn resolved_prompt(
        &self,
        action: RepositoryAiAction,
        project_prompt: Option<&str>,
        additional_prompt: Option<&str>,
    ) -> Result<String, RepositoryAiPreferencesError> {
        let configured = self.action(action).prompt.as_str();
        let prompt = if action == RepositoryAiAction::CreatePullRequest {
            normalized_prompt(project_prompt).unwrap_or_else(|| {
                normalized_prompt(Some(configured)).unwrap_or(action.default_prompt())
            })
        } else {
            normalized_prompt(Some(configured)).unwrap_or(action.default_prompt())
        };
        if prompt.len() > CONFIGURED_PROMPT_BYTE_LIMIT {
            return Err(RepositoryAiPreferencesError::ConfiguredPromptTooLong);
        }
        let Some(additional) = bounded_additional_prompt(additional_prompt) else {
            return Ok(prompt.to_owned());
        };
        Ok(format!("{prompt}\n\n{additional}"))
    }

    pub fn resolve_provider(
        &self,
        action: RepositoryAiAction,
        installed: &HashSet<&str>,
    ) -> Result<&'static ProviderDescriptor, RepositoryAiProviderError> {
        let configured = self.action(action).provider.trim();
        if configured.is_empty() {
            return PROVIDERS
                .iter()
                .find(|provider| installed.contains(provider.id))
                .ok_or(RepositoryAiProviderError::NoProviderInstalled);
        }
        let provider = PROVIDERS
            .iter()
            .find(|provider| provider.id == configured)
            .ok_or_else(|| RepositoryAiProviderError::Unsupported(bounded_label(configured)))?;
        if !installed.contains(provider.id) {
            return Err(RepositoryAiProviderError::NotInstalled(
                provider.display_name.to_owned(),
            ));
        }
        Ok(provider)
    }
}

pub fn normalized_prompt(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.trim().is_empty())
}

pub fn normalized_project_prompt(value: Option<&str>) -> Option<String> {
    normalized_prompt(value).map(str::to_owned)
}

pub fn use_global_prompt() -> Option<String> {
    None
}

fn bounded_additional_prompt(value: Option<&str>) -> Option<String> {
    let value = value?;
    let mut bounded: String = value
        .chars()
        .take(ADDITIONAL_PROMPT_CHARACTER_LIMIT)
        .collect();
    while bounded.len() > ADDITIONAL_PROMPT_BYTE_LIMIT {
        bounded.pop();
    }
    (!bounded.trim().is_empty()).then(|| bounded.trim().to_owned())
}

fn bounded_label(value: &str) -> String {
    value.chars().take(256).collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepositoryAiPreferencesError {
    ConfiguredPromptTooLong,
}

impl Display for RepositoryAiPreferencesError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("The configured repository AI prompt exceeds the 16 KB limit")
    }
}

impl Error for RepositoryAiPreferencesError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepositoryAiProviderError {
    NoProviderInstalled,
    Unsupported(String),
    NotInstalled(String),
}

impl Display for RepositoryAiProviderError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoProviderInstalled => {
                formatter.write_str("No supported AI provider CLI is installed")
            }
            Self::Unsupported(provider) => {
                write!(
                    formatter,
                    "The configured AI provider is unsupported: {provider}"
                )
            }
            Self::NotInstalled(provider) => write!(formatter, "{provider} CLI is not installed"),
        }
    }
}

impl Error for RepositoryAiProviderError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_catalog_is_the_exact_eleven_entry_launch_contract() {
        assert_eq!(PROVIDERS.len(), 11);
        assert_eq!(
            PROVIDERS
                .iter()
                .map(|provider| provider.id)
                .collect::<Vec<_>>(),
            [
                "claude",
                "opencode",
                "codex",
                "cursor",
                "copilot",
                "droid",
                "pi",
                "grok",
                "kiro",
                "xal",
                "antigravity",
            ]
        );
        assert_eq!(PROVIDERS[4].model_argument, None);
        assert!(
            PROVIDERS
                .iter()
                .enumerate()
                .all(|(index, provider)| index == 4 || provider.model_argument == Some("--model"))
        );
        let antigravity = PROVIDERS.last().unwrap();
        assert_eq!(antigravity.executable_names, ["agy", "antigravity"]);
        assert_eq!(antigravity.display_name, "Antigravity CLI");
        assert_eq!(PROVIDERS[5].icon_key, "factory");
    }

    #[test]
    fn provider_selection_handles_automatic_configured_and_missing_inputs() {
        let mut preferences = RepositoryAiPreferences::default();
        let installed = HashSet::from(["codex"]);
        assert_eq!(
            preferences
                .resolve_provider(RepositoryAiAction::Commit, &installed)
                .unwrap()
                .id,
            "codex"
        );
        preferences.commit.provider = "claude".to_owned();
        assert_eq!(
            preferences.resolve_provider(RepositoryAiAction::Commit, &installed),
            Err(RepositoryAiProviderError::NotInstalled(
                "Claude Code".to_owned()
            ))
        );
        preferences.commit.provider = "removed".to_owned();
        assert_eq!(
            preferences.resolve_provider(RepositoryAiAction::Commit, &installed),
            Err(RepositoryAiProviderError::Unsupported("removed".to_owned()))
        );
        preferences.commit.provider.clear();
        assert_eq!(
            preferences.resolve_provider(RepositoryAiAction::Commit, &HashSet::new()),
            Err(RepositoryAiProviderError::NoProviderInstalled)
        );
    }

    #[test]
    fn prompts_resolve_defaults_project_override_clear_and_additional_order() {
        let mut preferences = RepositoryAiPreferences::default();
        preferences.commit.prompt = " \n ".to_owned();
        assert_eq!(
            preferences
                .resolved_prompt(RepositoryAiAction::Commit, Some("ignored"), None)
                .unwrap(),
            COMMIT_PROMPT
        );
        preferences.create_pull_request.prompt = "Global PR".to_owned();
        assert_eq!(
            preferences
                .resolved_prompt(
                    RepositoryAiAction::CreatePullRequest,
                    Some("Project PR"),
                    Some("Mention rollback"),
                )
                .unwrap(),
            "Project PR\n\nMention rollback"
        );
        assert_eq!(normalized_project_prompt(Some(" \n ")), None);
        assert_eq!(
            normalized_project_prompt(Some("Project PR")),
            Some("Project PR".to_owned())
        );
        assert_eq!(use_global_prompt(), None);
    }

    #[test]
    fn prompts_enforce_character_and_byte_bounds_without_splitting_utf8() {
        let mut preferences = RepositoryAiPreferences::default();
        let additional = format!("{}tail", "é".repeat(2_000));
        let resolved = preferences
            .resolved_prompt(RepositoryAiAction::Commit, None, Some(&additional))
            .unwrap();
        let appended = resolved.split_once("\n\n").unwrap().1;
        assert_eq!(appended.chars().count(), 2_000);
        assert!(appended.len() <= ADDITIONAL_PROMPT_BYTE_LIMIT);

        preferences.commit.prompt = "x".repeat(CONFIGURED_PROMPT_BYTE_LIMIT + 1);
        assert_eq!(
            preferences.resolved_prompt(RepositoryAiAction::Commit, None, None),
            Err(RepositoryAiPreferencesError::ConfiguredPromptTooLong)
        );
    }

    #[test]
    fn typed_preferences_round_trip_without_crossing_action_fields() {
        let preferences = RepositoryAiPreferences {
            commit: RepositoryAiActionPreferences {
                provider: "codex".to_owned(),
                prompt: "Commit prompt".to_owned(),
            },
            create_pull_request: RepositoryAiActionPreferences {
                provider: "claude".to_owned(),
                prompt: "PR prompt".to_owned(),
            },
        };
        let encoded = serde_json::to_vec(&preferences).unwrap();
        let decoded: RepositoryAiPreferences = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, preferences);
    }
}
