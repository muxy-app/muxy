import Foundation
import SwiftUI

enum SettingsCategory: String, CaseIterable, Identifiable {
    case general
    case projects
    case appearance
    case terminal
    case editor
    case shortcuts
    case voice
    case notifications
    case mobile
    case ai
    case extensions
    case json

    var id: String { rawValue }

    var title: String {
        switch self {
        case .general: "App"
        case .projects: "Projects"
        case .appearance: "Interface"
        case .terminal: "Terminal"
        case .editor: "Editor"
        case .shortcuts: "Shortcuts"
        case .voice: "Voice"
        case .notifications: "Notifications"
        case .mobile: "Mobile"
        case .ai: "AI"
        case .extensions: "Extensions"
        case .json: "JSON"
        }
    }

    var symbolName: String {
        switch self {
        case .general: "gearshape"
        case .projects: "folder"
        case .appearance: "macwindow"
        case .terminal: "terminal"
        case .editor: "pencil.line"
        case .shortcuts: "keyboard"
        case .voice: "mic"
        case .notifications: "bell"
        case .mobile: "iphone"
        case .ai: "sparkles"
        case .extensions: "puzzlepiece.extension"
        case .json: "curlybraces"
        }
    }

    var developmentBadge: String? {
        switch self {
        case .extensions: "DEV"
        default: nil
        }
    }
}

struct SettingsCatalogItem: Identifiable, Equatable {
    let key: String
    let title: String
    let description: String
    let category: SettingsCategory
    let section: String
    let defaultValue: AnyHashable?
    let searchableText: String

    var id: String { key }

    init(
        key: String,
        title: String,
        description: String,
        category: SettingsCategory,
        section: String,
        defaultValue: AnyHashable? = nil,
        aliases: [String] = []
    ) {
        self.key = key
        self.title = title
        self.description = description
        self.category = category
        self.section = section
        self.defaultValue = defaultValue
        searchableText = ([key, title, description, category.title, section] + aliases)
            .joined(separator: " ")
            .lowercased()
    }
}

@MainActor
enum SettingsCatalog {
    static let userSettingsFilename = "settings.json"
    static let systemSettingsFilename = "default-settings.json"

    static let categories = SettingsCategory.allCases

    static let items: [SettingsCatalogItem] = [
        SettingsCatalogItem(
            key: UpdateChannel.storageKey,
            title: "Update Channel",
            description: "Controls whether Muxy receives stable releases or beta builds.",
            category: .general,
            section: "Updates",
            defaultValue: UpdateChannel.stable.rawValue,
            aliases: ["release", "beta"]
        ),
        SettingsCatalogItem(
            key: GeneralSettingsKeys.autoExpandWorktreesOnProjectSwitch,
            title: "Auto-expand Worktrees",
            description: "Automatically reveals worktrees when switching projects.",
            category: .appearance,
            section: "Sidebar",
            defaultValue: false
        ),
        SettingsCatalogItem(
            key: GeneralSettingsKeys.fileTreeSource,
            title: "File Tree Root Directory",
            description: "Controls whether the file tree follows the project or active terminal.",
            category: .projects,
            section: "File Tree",
            defaultValue: FileTreeSourcePreference.defaultValue.rawValue
        ),
        SettingsCatalogItem(
            key: ProjectPickerPreferences.storageKey,
            title: "Project Picker",
            description: "Chooses the picker used when opening projects.",
            category: .projects,
            section: "Projects",
            defaultValue: ProjectPickerMode.custom.rawValue
        ),
        SettingsCatalogItem(
            key: ProjectPickerDefaultLocation.storageKey,
            title: "Project Picker Default Path",
            description: "Sets the default folder for Muxy's project picker.",
            category: .projects,
            section: "Projects",
            defaultValue: "",
            aliases: ["folder", "path", "directory"]
        ),
        SettingsCatalogItem(
            key: ProjectLifecyclePreferences.keepOpenWhenNoTabsKey,
            title: "Keep Projects Open",
            description: "Keeps projects in the sidebar after closing the last tab.",
            category: .projects,
            section: "Projects",
            defaultValue: false
        ),
        SettingsCatalogItem(
            key: GeneralSettingsKeys.defaultWorktreeParentPath,
            title: "Default Worktree Path",
            description: "Sets the parent folder for new worktrees.",
            category: .projects,
            section: "Worktrees",
            defaultValue: "",
            aliases: ["folder", "path"]
        ),
        SettingsCatalogItem(
            key: GeneralSettingsKeys.autoCopyTerminalSelection,
            title: "Auto-copy Terminal Selection",
            description: "Copies terminal selections when the mouse is released.",
            category: .terminal,
            section: "Selection",
            defaultValue: false
        ),
        SettingsCatalogItem(
            key: TabCloseConfirmationPreferences.confirmRunningProcessKey,
            title: "Confirm Running Process Tab Close",
            description: "Asks before closing a terminal tab with a running process.",
            category: .terminal,
            section: "Tabs",
            defaultValue: true
        ),
        SettingsCatalogItem(
            key: QuitConfirmationPreferences.confirmQuitKey,
            title: "Confirm Quit",
            description: "Asks before quitting Muxy.",
            category: .general,
            section: "Quit",
            defaultValue: true
        ),
        SettingsCatalogItem(
            key: "muxy.sentry.consent",
            title: "Crash Reports",
            description: "Controls anonymous crash report consent when diagnostics are available.",
            category: .general,
            section: "Diagnostics",
            defaultValue: ""
        ),

        SettingsCatalogItem(
            key: "muxy.ui.scale",
            title: "Interface Size",
            description: "Controls the scale of the app interface.",
            category: .appearance,
            section: "Interface",
            defaultValue: UIScale.defaultPreset.rawValue,
            aliases: ["zoom", "density"]
        ),
        SettingsCatalogItem(
            key: "muxy.showStatusBar",
            title: "Show Status Bar",
            description: "Shows or hides the status bar.",
            category: .appearance,
            section: "Interface",
            defaultValue: true
        ),
        SettingsCatalogItem(
            key: "muxy.theme.light",
            title: "Light Terminal Theme",
            description: "Chooses the terminal theme for light appearance.",
            category: .terminal,
            section: "Appearance",
            defaultValue: ThemeService.defaultThemeName
        ),
        SettingsCatalogItem(
            key: "muxy.theme.dark",
            title: "Dark Terminal Theme",
            description: "Chooses the terminal theme for dark appearance.",
            category: .terminal,
            section: "Appearance",
            defaultValue: ThemeService.defaultThemeName
        ),
        SettingsCatalogItem(
            key: SidebarCollapsedStyle.storageKey,
            title: "Collapsed Sidebar Style",
            description: "Controls the sidebar appearance when collapsed.",
            category: .appearance,
            section: "Sidebar",
            defaultValue: SidebarCollapsedStyle.defaultValue.rawValue
        ),
        SettingsCatalogItem(
            key: SidebarExpandedStyle.storageKey,
            title: "Expanded Sidebar Style",
            description: "Controls the sidebar appearance when expanded.",
            category: .appearance,
            section: "Sidebar",
            defaultValue: SidebarExpandedStyle.defaultValue.rawValue
        ),
        SettingsCatalogItem(
            key: "muxy.vcsDisplayMode",
            title: "Source Control Display Mode",
            description: "Controls how source control is shown.",
            category: .appearance,
            section: "Source Control",
            defaultValue: VCSDisplayMode.attached.rawValue
        ),

        SettingsCatalogItem(
            key: "editor.defaultEditor",
            title: "Default Editor",
            description: "Chooses between Muxy's editor and a terminal editor command.",
            category: .editor,
            section: "Editor",
            defaultValue: EditorSettings.DefaultEditor.builtIn.rawValue
        ),
        SettingsCatalogItem(
            key: "editor.externalEditorCommand",
            title: "Editor Command",
            description: "Runs this command when the terminal editor is selected.",
            category: .editor,
            section: "Editor",
            defaultValue: "vim"
        ),
        SettingsCatalogItem(
            key: MarkdownPreviewPreferences.allowRemoteImagesKey,
            title: "Allow Remote Images",
            description: "Allows HTTPS images in Markdown preview.",
            category: .editor,
            section: "Markdown Preview",
            defaultValue: true
        ),
        SettingsCatalogItem(
            key: "editor.markdownPreviewFontFamily",
            title: "Markdown Preview Font Family",
            description: "Controls the Markdown preview font.",
            category: .editor,
            section: "Markdown Preview",
            defaultValue: EditorSettings.defaultMarkdownPreviewFontFamily
        ),
        SettingsCatalogItem(
            key: "editor.markdownPreviewFontScale",
            title: "Markdown Preview Zoom",
            description: "Controls Markdown preview zoom.",
            category: .editor,
            section: "Markdown Preview",
            defaultValue: Double(EditorSettings.defaultMarkdownPreviewFontScale)
        ),
        SettingsCatalogItem(
            key: "editor.htmlDefaultViewMode",
            title: "HTML Default View",
            description: "Chooses the default view mode for HTML files.",
            category: .editor,
            section: "HTML",
            defaultValue: EditorSettings.defaultHTMLViewMode.rawValue
        ),
        SettingsCatalogItem(
            key: "editor.richInputImageStrategy",
            title: "Rich Input Image Submission",
            description: "Chooses how rich input submits images.",
            category: .editor,
            section: "Rich Input",
            defaultValue: RichInputImageStrategy.clipboard.rawValue
        ),
        SettingsCatalogItem(
            key: RichInputPreferences.positionKey,
            title: "Rich Input Position",
            description: "Controls where the rich input panel appears.",
            category: .editor,
            section: "Rich Input",
            defaultValue: RichInputPreferences.defaultPosition.rawValue
        ),
        SettingsCatalogItem(
            key: RichInputPreferences.floatingKey,
            title: "Floating Rich Input",
            description: "Shows rich input as a floating panel.",
            category: .editor,
            section: "Rich Input",
            defaultValue: RichInputPreferences.defaultFloating
        ),
        SettingsCatalogItem(
            key: "editor.richInputFontFamily",
            title: "Rich Input Font Family",
            description: "Controls the rich input editor font family.",
            category: .editor,
            section: "Rich Input",
            defaultValue: EditorSettings.defaultRichInputFontFamily
        ),
        SettingsCatalogItem(
            key: "editor.richInputLineHeightMultiplier",
            title: "Rich Input Line Height",
            description: "Controls line height in rich input.",
            category: .editor,
            section: "Rich Input",
            defaultValue: Double(EditorSettings.defaultRichInputLineHeightMultiplier)
        ),
        SettingsCatalogItem(
            key: "editor.highlightCurrentLine",
            title: "Highlight Current Line",
            description: "Highlights the active line in the built-in editor.",
            category: .editor,
            section: "Appearance",
            defaultValue: true
        ),
        SettingsCatalogItem(
            key: "editor.showLineNumbers",
            title: "Show Line Numbers",
            description: "Shows line numbers in the built-in editor.",
            category: .editor,
            section: "Appearance",
            defaultValue: true
        ),
        SettingsCatalogItem(
            key: "editor.lineWrapping",
            title: "Wrap Lines",
            description: "Wraps long lines in the built-in editor.",
            category: .editor,
            section: "Appearance",
            defaultValue: false
        ),
        SettingsCatalogItem(
            key: "editor.fontFamily",
            title: "Editor Font Family",
            description: "Controls the built-in editor font family.",
            category: .editor,
            section: "Appearance",
            defaultValue: "SF Mono"
        ),
        SettingsCatalogItem(
            key: "editor.fontSize",
            title: "Editor Font Size",
            description: "Controls the built-in editor font size.",
            category: .editor,
            section: "Appearance",
            defaultValue: 13
        ),
        SettingsCatalogItem(
            key: "editor.lineHeightMultiplier",
            title: "Editor Line Height",
            description: "Controls line height in the built-in editor.",
            category: .editor,
            section: "Appearance",
            defaultValue: Double(EditorSettings.defaultLineHeightMultiplier)
        ),

        SettingsCatalogItem(
            key: SessionRestorePreferences.enabledKey,
            title: "Restore Terminal Sessions",
            description: "Restores terminal sessions when a project opens.",
            category: .terminal,
            section: "Session Restore",
            defaultValue: SessionRestorePreferences.defaultIsEnabled
        ),
        SettingsCatalogItem(
            key: SessionRestorePreferences.excludedCommandsKey,
            title: "Blocked Commands",
            description: "Commands that are never restored automatically.",
            category: .terminal,
            section: "Blocked Commands",
            defaultValue: SessionRestorePreferences.defaultExcludedCommands
        ),
        SettingsCatalogItem(
            key: "shortcuts.app",
            title: "App Shortcuts",
            description: "Configures Muxy keyboard shortcuts.",
            category: .shortcuts,
            section: "App Shortcuts",
            aliases: ["keybindings", "hotkeys"]
        ),
        SettingsCatalogItem(
            key: "shortcuts.customCommands",
            title: "Custom Commands",
            description: "Configures shortcuts that open command tabs.",
            category: .shortcuts,
            section: "Custom Commands",
            aliases: ["command layer"]
        ),
        SettingsCatalogItem(
            key: RecordingPreferences.autoSendKey,
            title: "Press Return After Inserting",
            description: "Presses Return after voice transcription is inserted.",
            category: .voice,
            section: "Voice Recording",
            defaultValue: RecordingPreferences.defaultAutoSend
        ),
        SettingsCatalogItem(
            key: RecordingPreferences.languageKey,
            title: "Recording Language",
            description: "Chooses the on-device speech recognition language.",
            category: .voice,
            section: "Language",
            defaultValue: RecordingPreferences.defaultLanguage
        ),
        SettingsCatalogItem(
            key: "muxy.notifications.toastEnabled",
            title: "Toast Notifications",
            description: "Shows toast notifications.",
            category: .notifications,
            section: "Delivery",
            defaultValue: true
        ),
        SettingsCatalogItem(
            key: "muxy.notifications.sound",
            title: "Notification Sound",
            description: "Chooses the notification sound.",
            category: .notifications,
            section: "Sound",
            defaultValue: NotificationSound.funk.rawValue
        ),
        SettingsCatalogItem(
            key: "muxy.notifications.toastPosition",
            title: "Toast Position",
            description: "Controls where toast notifications appear.",
            category: .notifications,
            section: "Toast",
            defaultValue: ToastPosition.topCenter.rawValue
        ),
        SettingsCatalogItem(
            key: "ai.providers",
            title: "AI Provider Notifications",
            description: "Controls AI provider notification integrations.",
            category: .notifications,
            section: "AI Providers"
        ),

        SettingsCatalogItem(
            key: MobileServerService.enabledKey,
            title: "Allow Mobile Connections",
            description: "Allows mobile devices to connect to this Mac.",
            category: .mobile,
            section: "Mobile",
            defaultValue: false
        ),
        SettingsCatalogItem(
            key: MobileServerService.portKey,
            title: "Mobile Port",
            description: "Controls the local server port for mobile pairing.",
            category: .mobile,
            section: "Mobile",
            defaultValue: MobileServerService.defaultPort
        ),
        SettingsCatalogItem(
            key: "mobile.pairing",
            title: "Pair Mobile Device",
            description: "Shows the QR code used to pair a mobile device.",
            category: .mobile,
            section: "Pair Mobile Device"
        ),
        SettingsCatalogItem(
            key: "mobile.approvedDevices",
            title: "Approved Devices",
            description: "Manages mobile devices that can connect.",
            category: .mobile,
            section: "Approved Devices"
        ),

        SettingsCatalogItem(
            key: AIAssistantSettings.providerKey,
            title: "AI Assistant Tool",
            description: "Chooses the CLI tool used for commit and PR generation.",
            category: .ai,
            section: "Provider",
            defaultValue: AIAssistantProvider.claude.rawValue
        ),
        SettingsCatalogItem(
            key: AIAssistantSettings.claudeModelKey,
            title: "Claude Model",
            description: "Optional Claude model override.",
            category: .ai,
            section: "Provider",
            defaultValue: ""
        ),
        SettingsCatalogItem(
            key: AIAssistantSettings.codexModelKey,
            title: "Codex Model",
            description: "Optional Codex model override.",
            category: .ai,
            section: "Provider",
            defaultValue: ""
        ),
        SettingsCatalogItem(
            key: AIAssistantSettings.opencodeModelKey,
            title: "OpenCode Model",
            description: "Optional OpenCode model override.",
            category: .ai,
            section: "Provider",
            defaultValue: ""
        ),
        SettingsCatalogItem(
            key: AIAssistantSettings.piModelKey,
            title: "Pi Model",
            description: "Optional Pi model override.",
            category: .ai,
            section: "Provider",
            defaultValue: ""
        ),
        SettingsCatalogItem(
            key: AIAssistantSettings.customCommandKey,
            title: "Custom AI Command",
            description: "Command used when the custom AI provider is selected.",
            category: .ai,
            section: "Provider",
            defaultValue: ""
        ),
        SettingsCatalogItem(
            key: AIAssistantSettings.commitPromptKey,
            title: "Commit Prompt",
            description: "Prompt used to generate commit messages.",
            category: .ai,
            section: "Commit Prompt",
            defaultValue: ""
        ),
        SettingsCatalogItem(
            key: AIAssistantSettings.prPromptKey,
            title: "Pull Request Prompt",
            description: "Prompt used to generate pull request drafts.",
            category: .ai,
            section: "Pull Request Prompt",
            defaultValue: ""
        ),

        SettingsCatalogItem(
            key: AIUsageSettingsStore.usageEnabledKey,
            title: "Enable AI Usage",
            description: "Shows the AI usage board in the sidebar.",
            category: .ai,
            section: "AI Usage",
            defaultValue: false
        ),
        SettingsCatalogItem(
            key: AIUsageSettingsStore.usageDisplayModeKey,
            title: "Usage Display Mode",
            description: "Shows used or remaining AI quota.",
            category: .ai,
            section: "AI Usage",
            defaultValue: AIUsageSettingsStore.defaultUsageDisplayMode.rawValue
        ),
        SettingsCatalogItem(
            key: AIUsageSettingsStore.autoRefreshIntervalKey,
            title: "Auto Refresh",
            description: "Controls how often AI usage data refreshes.",
            category: .ai,
            section: "AI Usage",
            defaultValue: AIUsageSettingsStore.defaultAutoRefreshInterval.rawValue
        ),
        SettingsCatalogItem(
            key: AIUsageSettingsStore.showSecondaryLimitsKey,
            title: "Show Secondary Limits",
            description: "Shows weekly and monthly quotas next to primary usage.",
            category: .ai,
            section: "AI Usage",
            defaultValue: AIUsageSettingsStore.defaultShowSecondaryLimits
        ),
        SettingsCatalogItem(
            key: "aiUsage.providers",
            title: "Tracked Usage Providers",
            description: "Chooses which providers appear on the usage board.",
            category: .ai,
            section: "AI Usage"
        ),
    ]

    static let jsonEditableItems = items.filter { item in
        item.defaultValue != nil
    }

    static func matchingItems(query: String) -> [SettingsCatalogItem] {
        let normalized = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !normalized.isEmpty else { return items }
        return items.filter { $0.searchableText.contains(normalized) }
    }

    static func categoryMatches(_ category: SettingsCategory, query: String) -> Bool {
        let normalized = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return true }
        return category.title.localizedCaseInsensitiveContains(normalized)
            || matchingItems(query: normalized).contains { $0.category == category }
    }

    static func sectionMatches(query: String, category: SettingsCategory?, section: String) -> Bool {
        let normalized = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return true }
        return matchingItems(query: normalized).contains { item in
            item.section == section && (category == nil || item.category == category)
        }
    }
}
