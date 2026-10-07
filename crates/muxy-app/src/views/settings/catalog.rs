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
            "Open configuration files in the system editor. Restart Muxy after editing app settings, or reload terminal configuration from Terminal settings."
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
        section: tr_key!("Commit and Push"),
    },
    Setting {
        id: "ai-commit-prompt",
        label: tr_key!("Commit prompt"),
        description: tr_key!(
            "AI drafts only the commit message. You review it before Muxy commits every change and pushes. An empty prompt uses the default. Do not include secrets."
        ),
        category: Category::Ai,
        section: tr_key!("Commit and Push"),
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
        id: "worktree-order",
        label: tr_key!("Order worktrees by recent use"),
        description: tr_key!("List recently selected worktrees first, after the primary worktree."),
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
        section: tr_key!("Text"),
    },
    Setting {
        id: "font-size",
        label: tr_key!("Font size (points)"),
        description: tr_key!(
            "Default terminal text size. Individual panes can still be zoomed independently."
        ),
        category: Category::Terminal,
        section: tr_key!("Text"),
    },
    Setting {
        id: "adjust-cell-height",
        label: tr_key!("Cell height adjustment (pixels or %%)"),
        description: tr_key!(
            "Add space between terminal lines using pixels or a percentage, such as 10%%."
        ),
        category: Category::Terminal,
        section: tr_key!("Text"),
    },
    Setting {
        id: "copy-on-select",
        label: tr_key!("Copy on select"),
        description: tr_key!(
            "Copy selected terminal text to the clipboard as soon as you finish selecting."
        ),
        category: Category::Terminal,
        section: tr_key!("Behavior"),
    },
    Setting {
        id: "directory",
        label: tr_key!("New pane directory"),
        description: tr_key!(
            "Start new splits in the project folder or the current pane's working directory."
        ),
        category: Category::Terminal,
        section: tr_key!("Behavior"),
    },
    Setting {
        id: "ghostty-configuration",
        label: tr_key!("Ghostty configuration"),
        description: tr_key!(
            "Edit ghostty.conf to customize terminal appearance and key aliases such as Shift+Enter. Save the file, then reload to apply changes."
        ),
        category: Category::Terminal,
        section: tr_key!("Configuration"),
    },
    Setting {
        id: "terminal-configuration-warnings",
        label: tr_key!("Configuration warnings"),
        description: tr_key!("Unsupported Ghostty options in ghostty.conf are ignored."),
        category: Category::Terminal,
        section: tr_key!("Configuration"),
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
        id: "sandbox-executable",
        label: tr_key!("Sandbox: nono executable"),
        description: tr_key!(
            "Absolute path to the supported nono executable. Sandboxed terminals are experimental."
        ),
        category: Category::Server,
        section: tr_key!("Sandboxed terminals"),
    },
    Setting {
        id: "sandbox-network",
        label: tr_key!("Sandbox network: blocked or domains"),
        description: tr_key!(
            "Choose blocked networking or an approved domain list. Changes apply to new terminals."
        ),
        category: Category::Server,
        section: tr_key!("Sandboxed terminals"),
    },
    Setting {
        id: "sandbox-domains",
        label: tr_key!("Sandbox approved domains (comma separated)"),
        description: tr_key!("Exact hostnames the terminal may reach through its network proxy."),
        category: Category::Server,
        section: tr_key!("Sandboxed terminals"),
    },
    Setting {
        id: "sandbox-tools",
        label: tr_key!("Sandbox read-only tool paths (semicolon separated)"),
        description: tr_key!(
            "Approve specific tool installations and runtime files outside the workspace."
        ),
        category: Category::Server,
        section: tr_key!("Sandboxed terminals"),
    },
    Setting {
        id: "sandbox-environment",
        label: tr_key!("Sandbox approved environment names (comma separated)"),
        description: tr_key!(
            "Explicitly forward selected variables from the server. Values are not saved in settings."
        ),
        category: Category::Server,
        section: tr_key!("Sandboxed terminals"),
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
