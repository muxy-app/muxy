//! Shared shortcut catalog and configuration interface. No UI or app dependencies.

use crate::capability::Feature;

pub const WORKSPACE_CLIPBOARD_CONTEXT: &str =
    "WorkspaceTabs && !TextInput && !BareInput && !MultilineInput && !TerminalSearchInput";

#[derive(Clone, Copy, Debug)]
pub struct Shortcut {
    pub id: &'static str,
    pub keys: &'static [&'static str],
    pub contexts: &'static [Option<&'static str>],
    pub key_contexts: &'static [&'static [Option<&'static str>]],
}

impl Shortcut {
    /// The name Settings shows, also the key of its translation.
    pub fn label(&self) -> String {
        self.id.replace(['_', '.'], " ")
    }

    /// The feature whose backend the shortcut's command needs.
    pub fn feature(&self) -> Option<Feature> {
        FEATURES
            .iter()
            .find(|(id, _)| id.name() == self.id)
            .map(|(_, feature)| *feature)
    }
}

const FEATURES: [(ShortcutId, Feature); 9] = [
    (ShortcutId::InstallCommandLineTool, Feature::CommandLineTool),
    (ShortcutId::HideApp, Feature::AppHiding),
    (ShortcutId::HideOthers, Feature::AppHiding),
    (ShortcutId::ShowAll, Feature::AppHiding),
    (ShortcutId::ComposerVoice, Feature::Voice),
    (ShortcutId::ToggleVoiceRecording, Feature::Voice),
    (ShortcutId::VoiceFinish, Feature::Voice),
    (ShortcutId::VoiceCancel, Feature::Voice),
    (ShortcutId::VoicePause, Feature::Voice),
];

pub trait ShortcutSettings {
    fn keys(&self, id: &str, context: Option<&str>) -> Vec<String>;
}

#[derive(Debug)]
pub struct Defaults;

impl ShortcutSettings for Defaults {
    fn keys(&self, id: &str, context: Option<&str>) -> Vec<String> {
        find(id).map_or_else(Vec::new, |shortcut| {
            shortcut
                .keys
                .iter()
                .zip(shortcut.key_contexts)
                .filter(|(_, contexts)| contexts.contains(&context))
                .map(|(key, _)| (*key).to_owned())
                .collect()
        })
    }
}

pub fn find(id: &str) -> Option<&'static Shortcut> {
    ALL.iter().find(|shortcut| shortcut.id == id)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShortcutId {
    NewTab,
    ExistingTerminals,
    DetachTerminal,
    CloseTab,
    SplitRight,
    SplitDown,
    FocusPaneLeft,
    FocusPaneRight,
    FocusPaneUp,
    FocusPaneDown,
    ToggleZoomPane,
    EqualizeSplits,
    ClosePane,
    NextTab,
    PreviousTab,
    PreviousProject,
    NextProject,
    AddProject,
    SelectTab1,
    SelectTab2,
    SelectTab3,
    SelectTab4,
    SelectTab5,
    SelectTab6,
    SelectTab7,
    SelectTab8,
    SelectTab9,
    SelectProject1,
    SelectProject2,
    SelectProject3,
    SelectProject4,
    SelectProject5,
    SelectProject6,
    SelectProject7,
    SelectProject8,
    SelectProject9,
    Copy,
    Paste,
    Find,
    FindNext,
    FindPrevious,
    ScrollToBottom,
    IncreaseFontSize,
    DecreaseFontSize,
    NewHomeTab,
    NewTabInProject,
    NewWorktree,
    SwitchWorktree,
    ToggleSidebar,
    ToggleAgentsPanel,
    ToggleAgentSessions,
    ToggleFullScreen,
    ToggleThemePicker,
    NavigateBack,
    NavigateForward,
    Quit,
    HideApp,
    HideOthers,
    Minimize,
    TextInputBackspace,
    TextInputDelete,
    TextInputLeft,
    TextInputRight,
    TextInputWordLeft,
    TextInputWordRight,
    TextInputHome,
    TextInputEnd,
    TextInputSelectLeft,
    TextInputSelectRight,
    TextInputSelectWordLeft,
    TextInputSelectWordRight,
    TextInputSelectHome,
    TextInputSelectEnd,
    TextInputSelectAll,
    TextInputCopy,
    TextInputCut,
    TextInputPaste,
    TextInputUndo,
    TextInputRedo,
    TextInputDocumentStart,
    TextInputDocumentEnd,
    TextInputShowCharacterPalette,
    TextInputInsertNewline,
    TextInputUp,
    TextInputDown,
    TextInputSelectUp,
    TextInputSelectDown,
    TextInputDeleteWord,
    TextInputPageUp,
    TextInputPageDown,
    TextInputSelectPageUp,
    TextInputSelectPageDown,
    TextInputSelectDocumentStart,
    TextInputSelectDocumentEnd,
    TextInputSubmit,
    TextInputCancel,
    PopoverSelectNext,
    PopoverSelectPrevious,
    PopoverConfirm,
    PopoverSecondaryConfirm,
    PopoverTabPressed,
    PopoverNavigateBack,
    PopoverDismiss,
    PopoverNextTab,
    PopoverPreviousTab,
    MenuDismissMenu,
    MenuHighlightPrevious,
    MenuHighlightNext,
    MenuConfirmHighlighted,
    MenuOpenSubmenu,
    MenuCloseSubmenu,
    ButtonActivate,
    EndAllSessionsAndQuit,
    ShowAll,
    Zoom,
    OpenConfiguration,
    TerminalCloseFind,
    PreviousPrompt,
    NextPrompt,
    SelectCommandOutput,
    OpenSettings,
    CloseSettings,
    SearchSettings,
    FocusSettingsNavbar,
    NextSettingsControl,
    PreviousSettingsControl,
    InstallCommandLineTool,
    ToggleCommandPalette,
    ToggleComposer,
    ComposerSubmit,
    ComposerInsert,
    ComposerClose,
    ComposerVoice,
    ToggleVoiceRecording,
    VoiceFinish,
    VoiceCancel,
    VoicePause,
    NewBrowserTab,
    BrowserFocusAddress,
    BrowserReload,
    BrowserBack,
    BrowserForward,
    BrowserZoomIn,
    BrowserZoomOut,
    BrowserZoomReset,
    BrowserCopyUrl,
    BrowserInspect,
}

impl ShortcutId {
    pub const TABS: [Self; 9] = [
        Self::SelectTab1,
        Self::SelectTab2,
        Self::SelectTab3,
        Self::SelectTab4,
        Self::SelectTab5,
        Self::SelectTab6,
        Self::SelectTab7,
        Self::SelectTab8,
        Self::SelectTab9,
    ];

    pub const PROJECTS: [Self; 9] = [
        Self::SelectProject1,
        Self::SelectProject2,
        Self::SelectProject3,
        Self::SelectProject4,
        Self::SelectProject5,
        Self::SelectProject6,
        Self::SelectProject7,
        Self::SelectProject8,
        Self::SelectProject9,
    ];

    pub fn name(self) -> &'static str {
        self.definition().id
    }

    pub fn definition(self) -> &'static Shortcut {
        &ALL[self as usize]
    }
}

macro_rules! platform_keys {
    ($mac:expr, $linux:expr) => {
        if cfg!(target_os = "macos") {
            $mac
        } else {
            $linux
        }
    };
}

pub const ALL: &[Shortcut] = &[
    Shortcut {
        id: "new_tab",
        keys: platform_keys!(&["cmd-t"], &["ctrl-shift-t"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "existing_terminals",
        keys: platform_keys!(&["cmd-alt-t"], &["ctrl-shift-alt-t"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "detach_terminal",
        keys: &[],
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[],
    },
    Shortcut {
        id: "close_tab",
        keys: platform_keys!(&["cmd-shift-w"], &["ctrl-alt-w"]),
        contexts: &[Some("WorkspaceTabs"), Some("QuickTerminal")],
        key_contexts: &[&[Some("WorkspaceTabs"), Some("QuickTerminal")]],
    },
    Shortcut {
        id: "split_right",
        keys: platform_keys!(&["cmd-d"], &["ctrl-shift-d"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "split_down",
        keys: platform_keys!(&["cmd-shift-d"], &["ctrl-shift-alt-d"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "focus_pane_left",
        keys: platform_keys!(&["cmd-alt-left"], &["alt-left"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "focus_pane_right",
        keys: platform_keys!(&["cmd-alt-right"], &["alt-right"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "focus_pane_up",
        keys: platform_keys!(&["cmd-alt-up"], &["alt-up"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "focus_pane_down",
        keys: platform_keys!(&["cmd-alt-down"], &["alt-down"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "toggle_zoom_pane",
        keys: platform_keys!(&["cmd-shift-enter"], &["ctrl-shift-enter"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "equalize_splits",
        keys: platform_keys!(&["cmd-ctrl-="], &["ctrl-alt-="]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "close_pane",
        keys: platform_keys!(&["cmd-w"], &["ctrl-shift-w"]),
        contexts: &[Some("WorkspaceTabs"), Some("QuickTerminal")],
        key_contexts: &[&[Some("WorkspaceTabs"), Some("QuickTerminal")]],
    },
    Shortcut {
        id: "next_tab",
        keys: platform_keys!(&["cmd-]", "ctrl-tab"], &["ctrl-pagedown", "ctrl-tab"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")], &[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "previous_tab",
        keys: platform_keys!(
            &["cmd-[", "ctrl-shift-tab"],
            &["ctrl-pageup", "ctrl-shift-tab"]
        ),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")], &[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "previous_project",
        keys: platform_keys!(&["ctrl-[", "cmd-alt-["], &["ctrl-[", "ctrl-alt-{"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")], &[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "next_project",
        keys: platform_keys!(&["ctrl-]", "cmd-alt-]"], &["ctrl-]", "ctrl-alt-}"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")], &[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "add_project",
        keys: platform_keys!(&["cmd-o"], &["ctrl-shift-o"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_tab1",
        keys: platform_keys!(&["cmd-1"], &["alt-1"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_tab2",
        keys: platform_keys!(&["cmd-2"], &["alt-2"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_tab3",
        keys: platform_keys!(&["cmd-3"], &["alt-3"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_tab4",
        keys: platform_keys!(&["cmd-4"], &["alt-4"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_tab5",
        keys: platform_keys!(&["cmd-5"], &["alt-5"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_tab6",
        keys: platform_keys!(&["cmd-6"], &["alt-6"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_tab7",
        keys: platform_keys!(&["cmd-7"], &["alt-7"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_tab8",
        keys: platform_keys!(&["cmd-8"], &["alt-8"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_tab9",
        keys: platform_keys!(&["cmd-9"], &["alt-9"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_project1",
        keys: &["ctrl-1"],
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_project2",
        keys: &["ctrl-2"],
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_project3",
        keys: &["ctrl-3"],
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_project4",
        keys: &["ctrl-4"],
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_project5",
        keys: &["ctrl-5"],
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_project6",
        keys: &["ctrl-6"],
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_project7",
        keys: &["ctrl-7"],
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_project8",
        keys: &["ctrl-8"],
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "select_project9",
        keys: &["ctrl-9"],
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "copy",
        keys: platform_keys!(&["cmd-c"], &["ctrl-shift-c"]),
        contexts: &[
            Some(WORKSPACE_CLIPBOARD_CONTEXT),
            Some("QuickTerminal && !TextInput && !TerminalSearchInput"),
        ],
        key_contexts: &[&[
            Some(WORKSPACE_CLIPBOARD_CONTEXT),
            Some("QuickTerminal && !TextInput && !TerminalSearchInput"),
        ]],
    },
    Shortcut {
        id: "paste",
        keys: platform_keys!(&["cmd-v"], &["ctrl-shift-v"]),
        contexts: &[
            Some(WORKSPACE_CLIPBOARD_CONTEXT),
            Some("QuickTerminal && !TextInput && !TerminalSearchInput"),
        ],
        key_contexts: &[&[
            Some(WORKSPACE_CLIPBOARD_CONTEXT),
            Some("QuickTerminal && !TextInput && !TerminalSearchInput"),
        ]],
    },
    Shortcut {
        id: "find",
        keys: platform_keys!(&["cmd-f"], &["ctrl-shift-f"]),
        contexts: &[Some("WorkspaceTabs"), Some("QuickTerminal")],
        key_contexts: &[&[Some("WorkspaceTabs"), Some("QuickTerminal")]],
    },
    Shortcut {
        id: "find_next",
        keys: platform_keys!(&["cmd-g"], &["ctrl-shift-g"]),
        contexts: &[Some("WorkspaceTabs"), Some("QuickTerminal")],
        key_contexts: &[&[Some("WorkspaceTabs"), Some("QuickTerminal")]],
    },
    Shortcut {
        id: "find_previous",
        keys: platform_keys!(&["cmd-shift-g"], &["ctrl-alt-g"]),
        contexts: &[Some("WorkspaceTabs"), Some("QuickTerminal")],
        key_contexts: &[&[Some("WorkspaceTabs"), Some("QuickTerminal")]],
    },
    Shortcut {
        id: "scroll_to_bottom",
        keys: platform_keys!(&["cmd-end"], &["ctrl-shift-end"]),
        contexts: &[Some("WorkspaceTabs"), Some("QuickTerminal")],
        key_contexts: &[&[Some("WorkspaceTabs"), Some("QuickTerminal")]],
    },
    Shortcut {
        id: "increase_font_size",
        keys: platform_keys!(&["cmd-+", "cmd-=", "cmd-shift-="], &["ctrl-+", "ctrl-="]),
        contexts: &[Some("WorkspaceTabs"), Some("QuickTerminal")],
        key_contexts: &[
            &[Some("WorkspaceTabs"), Some("QuickTerminal")],
            &[Some("WorkspaceTabs"), Some("QuickTerminal")],
            &[Some("WorkspaceTabs"), Some("QuickTerminal")],
        ],
    },
    Shortcut {
        id: "decrease_font_size",
        keys: platform_keys!(&["cmd--"], &["ctrl--"]),
        contexts: &[Some("WorkspaceTabs"), Some("QuickTerminal")],
        key_contexts: &[&[Some("WorkspaceTabs"), Some("QuickTerminal")]],
    },
    Shortcut {
        id: "new_home_tab",
        keys: platform_keys!(&["cmd-n"], &["ctrl-shift-n"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "new_tab_in_project",
        keys: platform_keys!(&["cmd-shift-n"], &["ctrl-alt-n"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "new_worktree",
        keys: &["cmd-alt-n"],
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "switch_worktree",
        keys: &["cmd-alt-w"],
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "toggle_sidebar",
        keys: platform_keys!(&["cmd-b"], &["ctrl-shift-b"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "toggle_agents_panel",
        keys: platform_keys!(&["cmd-alt-a"], &["ctrl-shift-alt-a"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "toggle_agent_sessions",
        keys: &[],
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[],
    },
    Shortcut {
        id: "toggle_full_screen",
        keys: platform_keys!(&["cmd-ctrl-f"], &["f11"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "toggle_theme_picker",
        keys: platform_keys!(&["cmd-shift-k"], &["ctrl-alt-k"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "navigate_back",
        keys: platform_keys!(&["cmd-ctrl-left"], &["ctrl-shift-left"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "navigate_forward",
        keys: platform_keys!(&["cmd-ctrl-right"], &["ctrl-shift-right"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "quit",
        keys: platform_keys!(&["cmd-q"], &["ctrl-shift-q"]),
        contexts: &[None],
        key_contexts: &[&[None]],
    },
    Shortcut {
        id: "hide_app",
        keys: platform_keys!(&["cmd-h"], &["ctrl-shift-h"]),
        contexts: &[None],
        key_contexts: &[&[None]],
    },
    Shortcut {
        id: "hide_others",
        keys: platform_keys!(&["cmd-alt-h"], &["ctrl-shift-alt-h"]),
        contexts: &[None],
        key_contexts: &[&[None]],
    },
    Shortcut {
        id: "minimize",
        keys: platform_keys!(&["cmd-m"], &["ctrl-shift-m"]),
        contexts: &[None],
        key_contexts: &[&[None]],
    },
    Shortcut {
        id: "text_input.backspace",
        keys: &["backspace"],
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.delete",
        keys: &["delete"],
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.left",
        keys: &["left"],
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.right",
        keys: &["right"],
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.word_left",
        keys: platform_keys!(&["alt-left"], &["ctrl-left"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.word_right",
        keys: platform_keys!(&["alt-right"], &["ctrl-right"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.home",
        keys: platform_keys!(&["cmd-left"], &["home"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.end",
        keys: platform_keys!(&["cmd-right"], &["end"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.select_left",
        keys: &["shift-left"],
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.select_right",
        keys: &["shift-right"],
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.select_word_left",
        keys: platform_keys!(&["shift-alt-left"], &["shift-ctrl-left"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.select_word_right",
        keys: platform_keys!(&["shift-alt-right"], &["shift-ctrl-right"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.select_home",
        keys: platform_keys!(&["shift-cmd-left"], &["shift-home"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.select_end",
        keys: platform_keys!(&["shift-cmd-right"], &["shift-end"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.select_all",
        keys: platform_keys!(&["cmd-a"], &["ctrl-a"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
            Some("TerminalPane"),
            Some("TerminalFindOpen"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
            Some("TerminalPane"),
            Some("TerminalFindOpen"),
        ]],
    },
    Shortcut {
        id: "text_input.copy",
        keys: platform_keys!(&["cmd-c"], &["ctrl-c"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.cut",
        keys: platform_keys!(&["cmd-x"], &["ctrl-x"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.paste",
        keys: platform_keys!(&["cmd-v"], &["ctrl-v"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.undo",
        keys: platform_keys!(&["cmd-z"], &["ctrl-z"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.redo",
        keys: platform_keys!(&["shift-cmd-z"], &["shift-ctrl-z"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.document_start",
        keys: platform_keys!(&["home", "cmd-up"], &["ctrl-home", "ctrl-up"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[
            &[
                Some("TextInput"),
                Some("BareInput"),
                Some("TerminalSearchInput"),
                Some("MultilineInput"),
            ],
            &[Some("MultilineInput")],
        ],
    },
    Shortcut {
        id: "text_input.document_end",
        keys: platform_keys!(&["end", "cmd-down"], &["ctrl-end", "ctrl-down"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[
            &[
                Some("TextInput"),
                Some("BareInput"),
                Some("TerminalSearchInput"),
                Some("MultilineInput"),
            ],
            &[Some("MultilineInput")],
        ],
    },
    Shortcut {
        id: "text_input.show_character_palette",
        keys: platform_keys!(&["ctrl-cmd-space"], &["ctrl-space"]),
        contexts: &[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ],
        key_contexts: &[&[
            Some("TextInput"),
            Some("BareInput"),
            Some("TerminalSearchInput"),
            Some("MultilineInput"),
        ]],
    },
    Shortcut {
        id: "text_input.insert_newline",
        keys: &["enter"],
        contexts: &[Some("MultilineInput")],
        key_contexts: &[&[Some("MultilineInput")]],
    },
    Shortcut {
        id: "text_input.up",
        keys: &["up"],
        contexts: &[Some("MultilineInput")],
        key_contexts: &[&[Some("MultilineInput")]],
    },
    Shortcut {
        id: "text_input.down",
        keys: &["down"],
        contexts: &[Some("MultilineInput")],
        key_contexts: &[&[Some("MultilineInput")]],
    },
    Shortcut {
        id: "text_input.select_up",
        keys: &["shift-up"],
        contexts: &[Some("MultilineInput")],
        key_contexts: &[&[Some("MultilineInput")]],
    },
    Shortcut {
        id: "text_input.select_down",
        keys: &["shift-down"],
        contexts: &[Some("MultilineInput")],
        key_contexts: &[&[Some("MultilineInput")]],
    },
    Shortcut {
        id: "text_input.delete_word",
        keys: platform_keys!(&["alt-backspace"], &["ctrl-backspace"]),
        contexts: &[
            Some("MultilineInput"),
            Some("TextInput"),
            Some("TerminalSearchInput"),
        ],
        key_contexts: &[&[
            Some("MultilineInput"),
            Some("TextInput"),
            Some("TerminalSearchInput"),
        ]],
    },
    Shortcut {
        id: "text_input.page_up",
        keys: &["pageup"],
        contexts: &[Some("MultilineInput")],
        key_contexts: &[&[Some("MultilineInput")]],
    },
    Shortcut {
        id: "text_input.page_down",
        keys: &["pagedown"],
        contexts: &[Some("MultilineInput")],
        key_contexts: &[&[Some("MultilineInput")]],
    },
    Shortcut {
        id: "text_input.select_page_up",
        keys: &["shift-pageup"],
        contexts: &[Some("MultilineInput")],
        key_contexts: &[&[Some("MultilineInput")]],
    },
    Shortcut {
        id: "text_input.select_page_down",
        keys: &["shift-pagedown"],
        contexts: &[Some("MultilineInput")],
        key_contexts: &[&[Some("MultilineInput")]],
    },
    Shortcut {
        id: "text_input.select_document_start",
        keys: platform_keys!(&["shift-cmd-up"], &["ctrl-shift-home"]),
        contexts: &[Some("MultilineInput")],
        key_contexts: &[&[Some("MultilineInput")]],
    },
    Shortcut {
        id: "text_input.select_document_end",
        keys: platform_keys!(&["shift-cmd-down"], &["ctrl-shift-end"]),
        contexts: &[Some("MultilineInput")],
        key_contexts: &[&[Some("MultilineInput")]],
    },
    Shortcut {
        id: "text_input.submit",
        keys: &["enter"],
        contexts: &[Some("TextInput"), Some("TerminalSearchInput")],
        key_contexts: &[&[Some("TextInput"), Some("TerminalSearchInput")]],
    },
    Shortcut {
        id: "text_input.cancel",
        keys: &["escape"],
        contexts: &[Some("TextInput"), Some("TerminalSearchInput")],
        key_contexts: &[&[Some("TextInput"), Some("TerminalSearchInput")]],
    },
    Shortcut {
        id: "popover.select_next",
        keys: &["down"],
        contexts: &[Some("Picker")],
        key_contexts: &[&[Some("Picker")]],
    },
    Shortcut {
        id: "popover.select_previous",
        keys: &["up", "shift-tab"],
        contexts: &[Some("Picker")],
        key_contexts: &[&[Some("Picker")], &[Some("Picker")]],
    },
    Shortcut {
        id: "popover.confirm",
        keys: &["enter"],
        contexts: &[Some("Picker"), Some("QuickTerminalConfirmation")],
        key_contexts: &[&[Some("Picker"), Some("QuickTerminalConfirmation")]],
    },
    Shortcut {
        id: "popover.secondary_confirm",
        keys: platform_keys!(
            &["alt-enter", "cmd-enter"],
            &["alt-enter", "ctrl-shift-enter"]
        ),
        contexts: &[Some("Picker")],
        key_contexts: &[&[Some("Picker")], &[Some("Picker")]],
    },
    Shortcut {
        id: "popover.tab_pressed",
        keys: &["tab"],
        contexts: &[Some("Picker")],
        key_contexts: &[&[Some("Picker")]],
    },
    Shortcut {
        id: "popover.navigate_back",
        keys: &["alt-backspace", "alt-left"],
        contexts: &[Some("Picker")],
        key_contexts: &[&[Some("Picker")], &[Some("Picker")]],
    },
    Shortcut {
        id: "popover.dismiss",
        keys: &["escape"],
        contexts: &[Some("Picker"), Some("QuickTerminalConfirmation")],
        key_contexts: &[&[Some("Picker"), Some("QuickTerminalConfirmation")]],
    },
    Shortcut {
        id: "popover.next_tab",
        keys: &["ctrl-tab"],
        contexts: &[Some("Picker")],
        key_contexts: &[&[Some("Picker")]],
    },
    Shortcut {
        id: "popover.previous_tab",
        keys: &["ctrl-shift-tab"],
        contexts: &[Some("Picker")],
        key_contexts: &[&[Some("Picker")]],
    },
    Shortcut {
        id: "menu.dismiss_menu",
        keys: &["escape"],
        contexts: &[Some("Menu")],
        key_contexts: &[&[Some("Menu")]],
    },
    Shortcut {
        id: "menu.highlight_previous",
        keys: &["up"],
        contexts: &[Some("Menu")],
        key_contexts: &[&[Some("Menu")]],
    },
    Shortcut {
        id: "menu.highlight_next",
        keys: &["down"],
        contexts: &[Some("Menu")],
        key_contexts: &[&[Some("Menu")]],
    },
    Shortcut {
        id: "menu.confirm_highlighted",
        keys: &["enter"],
        contexts: &[Some("Menu")],
        key_contexts: &[&[Some("Menu")]],
    },
    Shortcut {
        id: "menu.open_submenu",
        keys: &["right"],
        contexts: &[Some("Menu")],
        key_contexts: &[&[Some("Menu")]],
    },
    Shortcut {
        id: "menu.close_submenu",
        keys: &["left"],
        contexts: &[Some("Menu")],
        key_contexts: &[&[Some("Menu")]],
    },
    Shortcut {
        id: "button.activate",
        keys: &["enter", "space"],
        contexts: &[Some("Button")],
        key_contexts: &[&[Some("Button")], &[Some("Button")]],
    },
    Shortcut {
        id: "end_all_sessions_and_quit",
        keys: &[],
        contexts: &[None],
        key_contexts: &[],
    },
    Shortcut {
        id: "show_all",
        keys: &[],
        contexts: &[None],
        key_contexts: &[],
    },
    Shortcut {
        id: "zoom",
        keys: &[],
        contexts: &[None],
        key_contexts: &[],
    },
    Shortcut {
        id: "open_configuration",
        keys: &[],
        contexts: &[None],
        key_contexts: &[],
    },
    Shortcut {
        id: "terminal.close_find",
        keys: &["escape"],
        contexts: &[Some("TerminalFindOpen && !TerminalSearchInput")],
        key_contexts: &[&[Some("TerminalFindOpen && !TerminalSearchInput")]],
    },
    Shortcut {
        id: "previous_prompt",
        keys: platform_keys!(&["cmd-up", "cmd-shift-up"], &["ctrl-shift-up"]),
        contexts: &[
            Some(WORKSPACE_CLIPBOARD_CONTEXT),
            Some("QuickTerminal && !TextInput && !TerminalSearchInput"),
        ],
        key_contexts: &[
            &[
                Some(WORKSPACE_CLIPBOARD_CONTEXT),
                Some("QuickTerminal && !TextInput && !TerminalSearchInput"),
            ],
            &[
                Some(WORKSPACE_CLIPBOARD_CONTEXT),
                Some("QuickTerminal && !TextInput && !TerminalSearchInput"),
            ],
        ],
    },
    Shortcut {
        id: "next_prompt",
        keys: platform_keys!(&["cmd-down", "cmd-shift-down"], &["ctrl-shift-down"]),
        contexts: &[
            Some(WORKSPACE_CLIPBOARD_CONTEXT),
            Some("QuickTerminal && !TextInput && !TerminalSearchInput"),
        ],
        key_contexts: &[
            &[
                Some(WORKSPACE_CLIPBOARD_CONTEXT),
                Some("QuickTerminal && !TextInput && !TerminalSearchInput"),
            ],
            &[
                Some(WORKSPACE_CLIPBOARD_CONTEXT),
                Some("QuickTerminal && !TextInput && !TerminalSearchInput"),
            ],
        ],
    },
    Shortcut {
        id: "select_command_output",
        keys: &[],
        contexts: &[
            Some(WORKSPACE_CLIPBOARD_CONTEXT),
            Some("QuickTerminal && !TextInput && !TerminalSearchInput"),
        ],
        key_contexts: &[],
    },
    Shortcut {
        id: "open_settings",
        keys: platform_keys!(&["cmd-,"], &["ctrl-,"]),
        contexts: &[None],
        key_contexts: &[&[None]],
    },
    Shortcut {
        id: "settings.close",
        keys: platform_keys!(&["cmd-w"], &["ctrl-shift-w"]),
        contexts: &[Some("SettingsWindow")],
        key_contexts: &[&[Some("SettingsWindow")]],
    },
    Shortcut {
        id: "settings.search",
        keys: platform_keys!(&["cmd-f"], &["ctrl-shift-f"]),
        contexts: &[Some("SettingsWindow")],
        key_contexts: &[&[Some("SettingsWindow")]],
    },
    Shortcut {
        id: "settings.focus_navbar",
        keys: platform_keys!(&["cmd-shift-e"], &["ctrl-shift-e"]),
        contexts: &[Some("SettingsWindow")],
        key_contexts: &[&[Some("SettingsWindow")]],
    },
    Shortcut {
        id: "settings.next_control",
        keys: &["tab"],
        contexts: &[Some("SettingsWindow")],
        key_contexts: &[&[Some("SettingsWindow")]],
    },
    Shortcut {
        id: "settings.previous_control",
        keys: &["shift-tab"],
        contexts: &[Some("SettingsWindow")],
        key_contexts: &[&[Some("SettingsWindow")]],
    },
    Shortcut {
        id: "install_command_line_tool",
        keys: &[],
        contexts: &[None],
        key_contexts: &[],
    },
    Shortcut {
        id: "toggle_command_palette",
        keys: platform_keys!(&["cmd-shift-p"], &["ctrl-shift-p"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "toggle_composer",
        keys: platform_keys!(&["cmd-i"], &["ctrl-shift-i"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "composer.submit",
        keys: platform_keys!(&["cmd-enter"], &["ctrl-enter"]),
        contexts: &[Some("Composer")],
        key_contexts: &[&[Some("Composer")]],
    },
    Shortcut {
        id: "composer.insert",
        keys: platform_keys!(&["cmd-shift-enter"], &["ctrl-shift-enter"]),
        contexts: &[Some("Composer")],
        key_contexts: &[&[Some("Composer")]],
    },
    Shortcut {
        id: "composer.close",
        keys: &["escape"],
        contexts: &[Some("Composer && !Picker")],
        key_contexts: &[&[Some("Composer && !Picker")]],
    },
    Shortcut {
        id: "composer.voice",
        keys: &[],
        contexts: &[Some("Composer")],
        key_contexts: &[],
    },
    Shortcut {
        id: "toggle_voice_recording",
        keys: platform_keys!(&["cmd-shift-i"], &["ctrl-alt-i"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "voice.finish",
        keys: &["enter"],
        contexts: &[Some("VoiceRecording")],
        key_contexts: &[&[Some("VoiceRecording")]],
    },
    Shortcut {
        id: "voice.cancel",
        keys: &["escape"],
        contexts: &[Some("VoiceRecording")],
        key_contexts: &[&[Some("VoiceRecording")]],
    },
    Shortcut {
        id: "voice.pause",
        keys: &["space"],
        contexts: &[Some("VoiceRecording")],
        key_contexts: &[&[Some("VoiceRecording")]],
    },
    Shortcut {
        id: "new_browser_tab",
        keys: platform_keys!(&["cmd-alt-b"], &["ctrl-alt-b"]),
        contexts: &[Some("WorkspaceTabs")],
        key_contexts: &[&[Some("WorkspaceTabs")]],
    },
    Shortcut {
        id: "browser.focus_address",
        keys: platform_keys!(&["cmd-l"], &["ctrl-l"]),
        contexts: &[Some("Browser")],
        key_contexts: &[&[Some("Browser")]],
    },
    Shortcut {
        id: "browser.reload",
        keys: platform_keys!(&["cmd-r"], &["ctrl-r"]),
        contexts: &[Some("Browser")],
        key_contexts: &[&[Some("Browser")]],
    },
    Shortcut {
        id: "browser.back",
        keys: platform_keys!(&["cmd-["], &["alt-left"]),
        contexts: &[Some("Browser")],
        key_contexts: &[&[Some("Browser")]],
    },
    Shortcut {
        id: "browser.forward",
        keys: platform_keys!(&["cmd-]"], &["alt-right"]),
        contexts: &[Some("Browser")],
        key_contexts: &[&[Some("Browser")]],
    },
    Shortcut {
        id: "browser.zoom_in",
        keys: platform_keys!(&["cmd-=", "cmd-+", "cmd-shift-="], &["ctrl-=", "ctrl-+"]),
        contexts: &[Some("Browser")],
        key_contexts: &[&[Some("Browser")], &[Some("Browser")], &[Some("Browser")]],
    },
    Shortcut {
        id: "browser.zoom_out",
        keys: platform_keys!(&["cmd--"], &["ctrl--"]),
        contexts: &[Some("Browser")],
        key_contexts: &[&[Some("Browser")]],
    },
    Shortcut {
        id: "browser.zoom_reset",
        keys: platform_keys!(&["cmd-0"], &["ctrl-0"]),
        contexts: &[Some("Browser")],
        key_contexts: &[&[Some("Browser")]],
    },
    Shortcut {
        id: "browser.copy_url",
        keys: platform_keys!(&["cmd-shift-c"], &["ctrl-shift-c"]),
        contexts: &[Some("Browser")],
        key_contexts: &[&[Some("Browser")]],
    },
    Shortcut {
        id: "browser.inspect",
        keys: platform_keys!(&["cmd-alt-i"], &["ctrl-shift-i"]),
        contexts: &[Some("Browser")],
        key_contexts: &[&[Some("Browser")]],
    },
];
