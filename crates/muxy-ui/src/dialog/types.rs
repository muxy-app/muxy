#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConfirmationResponse {
    #[default]
    Cancelled,
    Confirmed {
        dont_ask_again: bool,
    },
}

/// A choice in an extension consent prompt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsentResponse {
    AllowAndRemember,
    Allow,
    Cancel,
    DenyAndRemember,
    Block,
    /// The user asked to allow everything from the extension; the caller
    /// confirms before saving it.
    Trust,
}

/// A labelled native alert, confirmation, or text prompt.
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

/// Choices shown between a prompt sheet's message and its buttons.
#[derive(Clone, Debug, Default)]
pub struct PromptChoices {
    pub picker: Option<PromptPicker>,
    pub checkboxes: Vec<PromptCheckbox>,
}

/// A labelled pop-up menu and the item selected first.
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

/// What a prompt sheet was confirmed with.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PromptResponse {
    pub prompt: String,
    /// The picker's selected item, when the sheet had a picker.
    pub picked: Option<usize>,
    /// Each checkbox's final state, in order.
    pub checked: Vec<bool>,
}
