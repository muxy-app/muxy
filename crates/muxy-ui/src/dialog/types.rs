#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConfirmationResponse {
    #[default]
    Cancelled,
    Confirmed {
        dont_ask_again: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsentResponse {
    AllowAndRemember,
    Allow,
    Cancel,
    DenyAndRemember,
    Block,
    Trust,
}

#[derive(Clone, Debug, Default)]
pub struct DialogOptions {
    pub title: String,
    pub message: String,
    pub buttons: Vec<String>,
    pub default_button: Option<String>,
    pub cancel_button: Option<String>,
    pub input: Option<(String, String)>,
    pub style: String,
}

pub const ADDITIONAL_PROMPT_LIMIT: usize = 2000;

#[derive(Clone, Debug, Default)]
pub struct PromptChoices {
    pub picker: Option<PromptPicker>,
    pub checkboxes: Vec<PromptCheckbox>,
}

#[derive(Clone, Debug)]
pub struct PromptPicker {
    pub label: String,
    pub items: Vec<String>,
    pub selected: usize,
}

#[derive(Clone, Debug)]
pub struct PromptCheckbox {
    pub label: String,
    pub checked: bool,
    pub enabled: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PromptResponse {
    pub prompt: String,
    pub picked: Option<usize>,
    pub checked: Vec<bool>,
}
