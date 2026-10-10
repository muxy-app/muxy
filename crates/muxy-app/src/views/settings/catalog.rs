use muxy_core::tr_key;

use super::Category;

pub(super) struct Setting {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) description: &'static str,
    pub(super) category: Category,
    pub(super) section: &'static str,
}

pub(super) const SETTINGS: &[Setting] = &[
    Setting {
        id: "backup-export",
        label: tr_key!("Export backup"),
        description: tr_key!(
            "Save settings, themes, extension preferences, projects, workspaces, and terminal layouts to a .muxy file."
        ),
        category: Category::Backup,
        section: tr_key!("Backup & Restore"),
    },
    Setting {
        id: "backup-restore",
        label: tr_key!("Restore backup"),
        description: tr_key!(
            "Choose a .muxy backup or a 1.x settings.json file. Review the import before applying it on the next launch."
        ),
        category: Category::Backup,
        section: tr_key!("Backup & Restore"),
    },
    Setting {
        id: "backup-legacy",
        label: tr_key!("Import from Muxy 1.x"),
        description: tr_key!(
            "Import supported settings and local projects from the installed 1.x app, or choose a backup file. Unsupported options are skipped."
        ),
        category: Category::Backup,
        section: tr_key!("Migration"),
    },
    Setting {
        id: "backup-files",
        label: tr_key!("Configuration files"),
        description: tr_key!(
            "Open configuration files in the system editor. Restart Muxy after editing app settings. Terminal settings reload when Muxy or Settings becomes active."
        ),
        category: Category::Backup,
        section: tr_key!("Configuration"),
    },
    Setting {
        id: "ai-commit-provider",
        label: tr_key!("Commit provider"),
        description: tr_key!(
            "AI CLI that drafts commit messages. Auto uses the first installed provider."
        ),
        category: Category::Ai,
        section: tr_key!("Commit"),
    },
    Setting {
        id: "ai-commit-prompt",
        label: tr_key!("Commit prompt"),
        description: tr_key!(
            "AI drafts only the commit message. Muxy commits, and pushes when you choose to. An empty prompt uses the default. Do not include secrets."
        ),
        category: Category::Ai,
        section: tr_key!("Commit"),
    },
    Setting {
        id: "ai-create-pr-provider",
        label: tr_key!("Pull request provider"),
        description: tr_key!(
            "AI CLI that drafts pull requests. Auto uses the first installed provider."
        ),
        category: Category::Ai,
        section: tr_key!("Create Pull Request"),
    },
    Setting {
        id: "ai-create-pr-prompt",
        label: tr_key!("Pull request prompt"),
        description: tr_key!(
            "AI drafts the title, summary, branch name, and target branch. You review them before Muxy commits, pushes, and opens the pull request. An empty prompt uses the default."
        ),
        category: Category::Ai,
        section: tr_key!("Create Pull Request"),
    },
    Setting {
        id: "voice-auto-send",
        label: tr_key!("Send after recording"),
        description: tr_key!(
            "Press Return after inserting standalone voice dictation into the terminal."
        ),
        category: Category::Composer,
        section: tr_key!("Voice"),
    },
    Setting {
        id: "composer-language",
        label: tr_key!("Dictation language"),
        description: tr_key!("Choose an installed on-device speech language."),
        category: Category::Composer,
        section: tr_key!("Voice"),
    },
    Setting {
        id: "composer-clear-after",
        label: tr_key!("Clear after sending"),
        description: tr_key!("Clear the draft only after every terminal confirms delivery."),
        category: Category::Composer,
        section: tr_key!("Behavior"),
    },
    Setting {
        id: "composer-clear-close",
        label: tr_key!("Clear on close"),
        description: tr_key!("Discard the draft when closing Composer or switching projects."),
        category: Category::Composer,
        section: tr_key!("Behavior"),
    },
    Setting {
        id: "composer-images",
        label: tr_key!("Image submission"),
        description: tr_key!(
            "Paste image contents into compatible terminal apps or insert the saved image path."
        ),
        category: Category::Composer,
        section: tr_key!("Behavior"),
    },
    Setting {
        id: "composer-font",
        label: tr_key!("Composer font"),
        description: tr_key!("Font family used for composing text."),
        category: Category::Composer,
        section: tr_key!("Text"),
    },
    Setting {
        id: "composer-line-height",
        label: tr_key!("Composer line height"),
        description: tr_key!("Line spacing from 1.1 to 2.0."),
        category: Category::Composer,
        section: tr_key!("Text"),
    },
    Setting {
        id: "quick-enabled",
        label: tr_key!("Enable Quick Terminal"),
        description: tr_key!("Show a persistent home-directory shell with a shortcut."),
        category: Category::QuickTerminal,
        section: tr_key!("General"),
    },
    Setting {
        id: "quick-shortcut",
        label: tr_key!("Open Quick Terminal"),
        description: tr_key!("Record a system-wide shortcut to open Quick Terminal."),
        category: Category::QuickTerminal,
        section: tr_key!("Shortcut"),
    },
    Setting {
        id: "quick-size",
        label: tr_key!("Terminal size"),
        description: tr_key!("Width 480–1200; height 280–800 pixels."),
        category: Category::QuickTerminal,
        section: tr_key!("Size"),
    },
    Setting {
        id: "quick-transparency",
        label: tr_key!("Terminal transparency"),
        description: tr_key!("Transparency of the terminal background."),
        category: Category::QuickTerminal,
        section: tr_key!("Appearance"),
    },
    Setting {
        id: "quick-blur",
        label: tr_key!("Background vibrancy"),
        description: tr_key!("Vibrancy of the glass background."),
        category: Category::QuickTerminal,
        section: tr_key!("Appearance"),
    },
    Setting {
        id: "close-behavior",
        label: tr_key!("When closing tabs or panes"),
        description: tr_key!(
            "Close sessions unused by other panes, or detach to keep them running and reopen them from Existing Terminals."
        ),
        category: Category::General,
        section: tr_key!("Closing terminals"),
    },
    Setting {
        id: "confirm-process",
        label: tr_key!("Confirm before closing a running process"),
        description: tr_key!(
            "Ask before ending a running command. Detaching never requires confirmation."
        ),
        category: Category::General,
        section: tr_key!("Closing terminals"),
    },
    Setting {
        id: "file-opener",
        label: tr_key!("Open files with"),
        description: tr_key!(
            "Opens files you click in a terminal. An extension opener opens only the files it supports; other files open in the project editor."
        ),
        category: Category::General,
        section: tr_key!("Files"),
    },
    Setting {
        id: "width",
        label: tr_key!("Default window width"),
        description: tr_key!(
            "Width in pixels for a new workspace window when no saved bounds exist."
        ),
        category: Category::General,
        section: tr_key!("Window size"),
    },
    Setting {
        id: "height",
        label: tr_key!("Default window height"),
        description: tr_key!(
            "Height in pixels for a new workspace window when no saved bounds exist."
        ),
        category: Category::General,
        section: tr_key!("Window size"),
    },
    Setting {
        id: "worktree-template",
        label: tr_key!("Default worktree path template"),
        description: tr_key!(
            "For example ../{base-dir}.{branch}. Must include {branch}; also supports {project-name}. Relative paths start in the project folder. Overrides the default folder."
        ),
        category: Category::General,
        section: tr_key!("Worktrees"),
    },
    Setting {
        id: "worktree-folder",
        label: tr_key!("Default worktree folder"),
        description: tr_key!(
            "Worktrees go in <folder>/<project>/<name>. Leave both location settings empty to use ~/.muxy/worktrees."
        ),
        category: Category::General,
        section: tr_key!("Worktrees"),
    },
    Setting {
        id: "app-language",
        label: tr_key!("App language"),
        description: tr_key!(
            "English is built in. Additional languages can be provided by enabled extensions."
        ),
        category: Category::Appearance,
        section: tr_key!("Language"),
    },
    Setting {
        id: "more-languages",
        label: tr_key!("More languages"),
        description: tr_key!("Find and install language packs in Extensions."),
        category: Category::Appearance,
        section: tr_key!("Language"),
    },
    Setting {
        id: "light-theme",
        label: tr_key!("Light theme"),
        description: tr_key!(
            "Colors for the interface and terminals when macOS uses light appearance."
        ),
        category: Category::Appearance,
        section: tr_key!("Themes"),
    },
    Setting {
        id: "dark-theme",
        label: tr_key!("Dark theme"),
        description: tr_key!(
            "Colors for the interface and terminals when macOS uses dark appearance."
        ),
        category: Category::Appearance,
        section: tr_key!("Themes"),
    },
    Setting {
        id: "sidebar",
        label: tr_key!("Expand sidebar"),
        description: tr_key!("Show project names alongside their icons in the workspace sidebar."),
        category: Category::Appearance,
        section: tr_key!("Interface"),
    },
    Setting {
        id: "status-bar",
        label: tr_key!("Show status bar"),
        description: tr_key!(
            "Show connection and terminal information at the bottom of the workspace."
        ),
        category: Category::Appearance,
        section: tr_key!("Interface"),
    },
    Setting {
        id: "sidebar-collapsed-style",
        label: tr_key!("Collapsed sidebar style"),
        description: tr_key!(
            "Show project icons or hide the sidebar in Project Focused. Tab Focused and Agents Focused always hide it when collapsed."
        ),
        category: Category::Appearance,
        section: tr_key!("Interface"),
    },
    Setting {
        id: "sidebar-vibrancy",
        label: tr_key!("Sidebar vibrancy"),
        description: tr_key!("Use a translucent macOS background while the sidebar is expanded."),
        category: Category::Appearance,
        section: tr_key!("Interface"),
    },
    Setting {
        id: "sidebar-vibrancy-level",
        label: tr_key!("Sidebar vibrancy level"),
        description: tr_key!(
            "How much of the macOS background effect shows through the theme color, from 0 to 100. Collapsed sidebars stay solid."
        ),
        category: Category::Appearance,
        section: tr_key!("Interface"),
    },
    Setting {
        id: "tips",
        label: tr_key!("Show tips"),
        description: tr_key!("Show Muxy tips at the bottom of the built-in sidebar."),
        category: Category::Appearance,
        section: tr_key!("Interface"),
    },
    Setting {
        id: "auto-expand-worktrees",
        label: tr_key!("Expand worktrees when switching projects"),
        description: tr_key!("Reveal worktrees when selecting a project in Project Focused."),
        category: Category::Appearance,
        section: tr_key!("Interface"),
    },
    Setting {
        id: "worktree-unread",
        label: tr_key!("Show unread worktree indicators"),
        description: tr_key!("Show unread completion counts beside worktrees."),
        category: Category::Appearance,
        section: tr_key!("Interface"),
    },
    Setting {
        id: "extension-sidebar",
        label: tr_key!("Active sidebar"),
        description: tr_key!("Show an enabled extension's sidebar in place of the built-in one."),
        category: Category::Appearance,
        section: tr_key!("Interface"),
    },
    Setting {
        id: "font-family",
        label: tr_key!("Font family"),
        description: tr_key!("The typeface used to render text in every terminal pane."),
        category: Category::Terminal,
        section: tr_key!("Font"),
    },
    Setting {
        id: "font-fallbacks",
        label: tr_key!("Fallback fonts"),
        description: tr_key!("Fonts used, in order, for characters the main font lacks."),
        category: Category::Terminal,
        section: tr_key!("Font"),
    },
    Setting {
        id: "font-size",
        label: tr_key!("Font size (points)"),
        description: tr_key!(
            "Default terminal text size. Individual panes can still be zoomed independently."
        ),
        category: Category::Terminal,
        section: tr_key!("Font"),
    },
    Setting {
        id: "adjust-cell-height",
        label: tr_key!("Line spacing"),
        description: tr_key!(
            "Add space between terminal lines using pixels or a percentage, such as 10%%."
        ),
        category: Category::Terminal,
        section: tr_key!("Font"),
    },
    Setting {
        id: "adjust-cell-width",
        label: tr_key!("Character spacing"),
        description: tr_key!("Adjust character spacing by pixels or a percentage, such as 10%%."),
        category: Category::Terminal,
        section: tr_key!("Font"),
    },
    Setting {
        id: "font-family-bold",
        label: tr_key!("Bold font"),
        description: tr_key!("Font for bold text. Default uses the main font's bold style."),
        category: Category::Terminal,
        section: tr_key!("Font"),
    },
    Setting {
        id: "font-family-italic",
        label: tr_key!("Italic font"),
        description: tr_key!("Font for italic text. Default uses the main font's italic style."),
        category: Category::Terminal,
        section: tr_key!("Font"),
    },
    Setting {
        id: "font-family-bold-italic",
        label: tr_key!("Bold italic font"),
        description: tr_key!(
            "Font for bold italic text. Default uses the main font's bold italic style."
        ),
        category: Category::Terminal,
        section: tr_key!("Font"),
    },
    Setting {
        id: "font-feature",
        label: tr_key!("Font features"),
        description: tr_key!(
            "OpenType features separated by commas, such as ss01 or zero. Put a minus before one to turn it off, such as -calt."
        ),
        category: Category::Terminal,
        section: tr_key!("Font"),
    },
    Setting {
        id: "font-ligatures",
        label: tr_key!("Font ligatures"),
        description: tr_key!(
            "Combine programming symbols when the selected font supports ligatures."
        ),
        category: Category::Terminal,
        section: tr_key!("Font"),
    },
    Setting {
        id: "font-codepoint-map",
        label: tr_key!("Codepoint fonts"),
        description: tr_key!(
            "Use a specific font for a Unicode range, such as U+E000-U+F8FF for icons."
        ),
        category: Category::Terminal,
        section: tr_key!("Font"),
    },
    Setting {
        id: "font-thicken",
        label: tr_key!("Thicken font"),
        description: tr_key!("Draw terminal text with a heavier stroke."),
        category: Category::Terminal,
        section: tr_key!("Font"),
    },
    Setting {
        id: "font-thicken-strength",
        label: tr_key!("Thicken strength"),
        description: tr_key!("How much heavier text is drawn when Thicken font is on."),
        category: Category::Terminal,
        section: tr_key!("Font"),
    },
    Setting {
        id: "bold-is-bright",
        label: tr_key!("Bright colors for bold text"),
        description: tr_key!(
            "Use bright ANSI colors for bold text in the first eight palette colors."
        ),
        category: Category::Terminal,
        section: tr_key!("Font"),
    },
    Setting {
        id: "background-transparency",
        label: tr_key!("Terminal transparency"),
        description: tr_key!("Let the desktop show through the terminal background."),
        category: Category::Terminal,
        section: tr_key!("Background"),
    },
    Setting {
        id: "background-vibrancy",
        label: tr_key!("Background vibrancy"),
        description: tr_key!(
            "Reveal the native macOS material by reducing the solid background tint."
        ),
        category: Category::Terminal,
        section: tr_key!("Background"),
    },
    Setting {
        id: "background-opacity-cells",
        label: tr_key!("Apply opacity to colored cells"),
        description: tr_key!("Also make backgrounds drawn by terminal programs translucent."),
        category: Category::Terminal,
        section: tr_key!("Background"),
    },
    Setting {
        id: "window-padding-x",
        label: tr_key!("Horizontal padding (points)"),
        description: tr_key!("Adjust the space to the left and right of terminal text."),
        category: Category::Terminal,
        section: tr_key!("Background"),
    },
    Setting {
        id: "window-padding-y",
        label: tr_key!("Vertical padding (points)"),
        description: tr_key!("Adjust the space above and below terminal text."),
        category: Category::Terminal,
        section: tr_key!("Background"),
    },
    Setting {
        id: "window-padding-balance",
        label: tr_key!("Balance padding"),
        description: tr_key!("Distribute unused cell space equally around the terminal grid."),
        category: Category::Terminal,
        section: tr_key!("Background"),
    },
    Setting {
        id: "window-padding-color",
        label: tr_key!("Padding color"),
        description: tr_key!(
            "Fill padding with the background color, or extend the color of the nearest cells."
        ),
        category: Category::Terminal,
        section: tr_key!("Background"),
    },
    Setting {
        id: "background",
        label: tr_key!("Background color"),
        description: tr_key!(
            "Replaces the theme's terminal background, such as #1e1e2e. Leave empty to use the theme."
        ),
        category: Category::Terminal,
        section: tr_key!("Colors"),
    },
    Setting {
        id: "foreground",
        label: tr_key!("Text color"),
        description: tr_key!(
            "Replaces the theme's terminal text color. Leave empty to use the theme."
        ),
        category: Category::Terminal,
        section: tr_key!("Colors"),
    },
    Setting {
        id: "cursor-color",
        label: tr_key!("Cursor color"),
        description: tr_key!(
            "A color, cell-foreground, or cell-background. Leave empty to use the theme."
        ),
        category: Category::Terminal,
        section: tr_key!("Colors"),
    },
    Setting {
        id: "cursor-text",
        label: tr_key!("Cursor text color"),
        description: tr_key!(
            "Color of the character under a block cursor. Leave empty to use the theme."
        ),
        category: Category::Terminal,
        section: tr_key!("Colors"),
    },
    Setting {
        id: "selection-foreground",
        label: tr_key!("Selection text color"),
        description: tr_key!(
            "A color, cell-foreground, or cell-background. Leave empty to use the theme."
        ),
        category: Category::Terminal,
        section: tr_key!("Colors"),
    },
    Setting {
        id: "selection-background",
        label: tr_key!("Selection color"),
        description: tr_key!(
            "A color, cell-foreground, or cell-background. Leave empty to use the theme."
        ),
        category: Category::Terminal,
        section: tr_key!("Colors"),
    },
    Setting {
        id: "palette",
        label: tr_key!("ANSI colors"),
        description: tr_key!("Replaces the 16 ANSI colors. Leave a color empty to use the theme."),
        category: Category::Terminal,
        section: tr_key!("Colors"),
    },
    Setting {
        id: "cursor-style",
        label: tr_key!("Cursor shape"),
        description: tr_key!("Choose the initial cursor shape. Terminal programs can change it."),
        category: Category::Terminal,
        section: tr_key!("Cursor"),
    },
    Setting {
        id: "cursor-style-blink",
        label: tr_key!("Cursor blinking"),
        description: tr_key!("Choose the initial blink behavior. Terminal programs can change it."),
        category: Category::Terminal,
        section: tr_key!("Cursor"),
    },
    Setting {
        id: "adjust-cursor-thickness",
        label: tr_key!("Cursor thickness"),
        description: tr_key!(
            "Thicken bar, underline, and outlined cursors. Use 1 for one extra physical pixel, or 100%% to double thickness."
        ),
        category: Category::Terminal,
        section: tr_key!("Cursor"),
    },
    Setting {
        id: "cursor-opacity",
        label: tr_key!("Cursor opacity"),
        description: tr_key!("How opaque the cursor is."),
        category: Category::Terminal,
        section: tr_key!("Cursor"),
    },
    Setting {
        id: "copy-on-select",
        label: tr_key!("Copy on select"),
        description: tr_key!(
            "Copy selected terminal text to the clipboard as soon as you finish selecting."
        ),
        category: Category::Terminal,
        section: tr_key!("Selection"),
    },
    Setting {
        id: "selection-clear-on-typing",
        label: tr_key!("Clear selection when typing"),
        description: tr_key!("Remove the selected range when sending text to the terminal."),
        category: Category::Terminal,
        section: tr_key!("Selection"),
    },
    Setting {
        id: "selection-clear-on-copy",
        label: tr_key!("Clear selection after copying"),
        description: tr_key!("Remove the selected range after copying its text."),
        category: Category::Terminal,
        section: tr_key!("Selection"),
    },
    Setting {
        id: "scroll-precision",
        label: tr_key!("Trackpad scroll speed"),
        description: tr_key!("Adjust smooth scrolling speed."),
        category: Category::Terminal,
        section: tr_key!("Scrolling"),
    },
    Setting {
        id: "scroll-discrete",
        label: tr_key!("Mouse wheel scroll speed"),
        description: tr_key!("Adjust mouse wheel scrolling speed."),
        category: Category::Terminal,
        section: tr_key!("Scrolling"),
    },
    Setting {
        id: "scroll-on-keystroke",
        label: tr_key!("Scroll to bottom when typing"),
        description: tr_key!("Return to live output when sending a keystroke."),
        category: Category::Terminal,
        section: tr_key!("Scrolling"),
    },
    Setting {
        id: "scroll-on-output",
        label: tr_key!("Scroll to bottom on output"),
        description: tr_key!("Return to live output whenever the terminal receives new text."),
        category: Category::Terminal,
        section: tr_key!("Scrolling"),
    },
    Setting {
        id: "mouse-reporting",
        label: tr_key!("Allow programs to use the mouse"),
        description: tr_key!(
            "Send mouse events to terminal programs that request them. Hold Shift to select text instead."
        ),
        category: Category::Terminal,
        section: tr_key!("Input"),
    },
    Setting {
        id: "macos-option-as-alt",
        label: tr_key!("Option as Alt"),
        description: tr_key!(
            "Use both Option keys, neither, or just one as the terminal Alt modifier."
        ),
        category: Category::Terminal,
        section: tr_key!("Input"),
    },
    Setting {
        id: "directory",
        label: tr_key!("New pane directory"),
        description: tr_key!(
            "Start new splits in the project folder or the current pane's working directory."
        ),
        category: Category::Terminal,
        section: tr_key!("Input"),
    },
    Setting {
        id: "terminal-keybindings",
        label: tr_key!("Key bindings"),
        description: tr_key!(
            "Keys the terminal handles while it is focused. They take precedence over app shortcuts."
        ),
        category: Category::Terminal,
        section: tr_key!("Key bindings"),
    },
    Setting {
        id: "keybind-clear-defaults",
        label: tr_key!("Disable built-in bindings"),
        description: tr_key!(
            "Turn off Muxy's built-in terminal keys, such as Command-K to clear the screen, and keep only yours."
        ),
        category: Category::Terminal,
        section: tr_key!("Key bindings"),
    },
    Setting {
        id: "terminal-import-notes",
        label: tr_key!("Imported settings"),
        description: tr_key!(
            "Notes about imported terminal settings. Your original configuration is preserved."
        ),
        category: Category::Terminal,
        section: tr_key!("Migration"),
    },
    Setting {
        id: "server",
        label: tr_key!("Current device"),
        description: tr_key!("Manage the local server that keeps your terminal sessions running."),
        category: Category::Server,
        section: tr_key!("Connection"),
    },
    Setting {
        id: "default-shell",
        label: tr_key!("Default shell (executable path)"),
        description: tr_key!(
            "Shell for new sessions. Leave empty to use $SHELL, falling back to /bin/zsh."
        ),
        category: Category::Server,
        section: tr_key!("Sessions"),
    },
    Setting {
        id: "history-budget",
        label: tr_key!("History budget per session (MiB)"),
        description: tr_key!(
            "Retained terminal history, from 0 to 65536 MiB. Saved-history changes take effect at the next server start."
        ),
        category: Category::Server,
        section: tr_key!("Sessions"),
    },
    Setting {
        id: "shell-integration",
        label: tr_key!("Shell integration"),
        description: tr_key!(
            "Enable working-directory tracking and prompt navigation in new sessions."
        ),
        category: Category::Server,
        section: tr_key!("Sessions"),
    },
    Setting {
        id: "Stop Server",
        label: tr_key!("Stop Server"),
        description: tr_key!(
            "End all sessions on this device and stop the server. A confirmation is required."
        ),
        category: Category::Server,
        section: tr_key!("Server control"),
    },
    Setting {
        id: "Restart Server",
        label: tr_key!("Restart Server"),
        description: tr_key!(
            "End all sessions and start the server again. Saved terminal records are preserved."
        ),
        category: Category::Server,
        section: tr_key!("Server control"),
    },
    Setting {
        id: "mobile-access",
        label: tr_key!("Allow mobile devices"),
        description: tr_key!(
            "Let paired phones reach this device's server over the local network or a VPN. A paired phone gets full terminal access."
        ),
        category: Category::Mobile,
        section: tr_key!("Access"),
    },
    Setting {
        id: "mobile-port",
        label: tr_key!("Port"),
        description: tr_key!("Network port phones connect to, from 1024 to 65535."),
        category: Category::Mobile,
        section: tr_key!("Access"),
    },
    Setting {
        id: "mobile-status",
        label: tr_key!("Status"),
        description: tr_key!("Whether phones can reach this device right now."),
        category: Category::Mobile,
        section: tr_key!("Access"),
    },
    Setting {
        id: "mobile-pair",
        label: tr_key!("Pair a phone"),
        description: tr_key!("Show a one-time code to scan with the Muxy app."),
        category: Category::Mobile,
        section: tr_key!("Pairing"),
    },
    Setting {
        id: "mobile-devices",
        label: tr_key!("Paired devices"),
        description: tr_key!("Phones that can connect. Revoking one disconnects it immediately."),
        category: Category::Mobile,
        section: tr_key!("Devices"),
    },
];

pub(super) fn setting(id: &str) -> Option<&'static Setting> {
    SETTINGS.iter().find(|setting| setting.id == id)
}
