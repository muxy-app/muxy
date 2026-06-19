import Foundation
import SwiftUI

enum SettingsCategory: String, CaseIterable, Identifiable {
    case general
    case projects
    case remoteDevices
    case appearance
    case sidebar
    case terminal
    case richInput
    case shortcuts
    case commands
    case voice
    case notifications
    case mobile
    case backup
    case json

    var id: String { rawValue }

    var title: String {
        switch self {
        case .general: "App"
        case .projects: "Projects"
        case .remoteDevices: "Remote Devices"
        case .appearance: "Interface"
        case .sidebar: "Sidebar"
        case .terminal: "Terminal"
        case .richInput: "Rich Input"
        case .shortcuts: "Shortcuts"
        case .commands: "Commands"
        case .voice: "Voice"
        case .notifications: "Notifications"
        case .mobile: "Mobile"
        case .backup: "Backup"
        case .json: "JSON"
        }
    }

    var symbolName: String {
        switch self {
        case .general: "gearshape"
        case .projects: "folder"
        case .remoteDevices: "server.rack"
        case .appearance: "macwindow"
        case .sidebar: "sidebar.left"
        case .terminal: "terminal"
        case .richInput: "text.cursor"
        case .shortcuts: "keyboard"
        case .commands: "command"
        case .voice: "mic"
        case .notifications: "bell"
        case .mobile: "iphone"
        case .backup: "externaldrive"
        case .json: "curlybraces"
        }
    }
}

enum SettingsRoute: Hashable, Identifiable {
    case builtin(SettingsCategory)
    case ext(String)

    var id: String {
        switch self {
        case let .builtin(category): "builtin.\(category.rawValue)"
        case let .ext(extensionID): "ext.\(extensionID)"
        }
    }

    init?(storedID: String) {
        if storedID.hasPrefix("builtin.") {
            let rawCategory = String(storedID.dropFirst("builtin.".count))
            guard let category = SettingsCategory(rawValue: rawCategory) else { return nil }
            self = .builtin(category)
            return
        }

        if storedID.hasPrefix("ext.") {
            let extensionID = String(storedID.dropFirst("ext.".count))
            guard !extensionID.isEmpty else { return nil }
            self = .ext(extensionID)
            return
        }

        return nil
    }
}

enum SettingsRouteSelectionStore {
    static let storageKey = "muxy.settings.selectedRoute"
    static let fallbackRoute = SettingsRoute.builtin(.general)

    static func load(defaults: UserDefaults = .standard) -> SettingsRoute {
        guard let storedID = defaults.string(forKey: storageKey),
              let route = SettingsRoute(storedID: storedID)
        else { return fallbackRoute }
        return route
    }

    static func save(_ route: SettingsRoute, defaults: UserDefaults = .standard) {
        defaults.set(route.id, forKey: storageKey)
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
            category: .sidebar,
            section: "Sidebar",
            defaultValue: false
        ),
        SettingsCatalogItem(
            key: HomeProjectPreferences.visibleKey,
            title: "Show Home",
            description: "Shows the permanent Home project at the top of the sidebar.",
            category: .sidebar,
            section: "Sidebar",
            defaultValue: HomeProjectPreferences.defaultVisible
        ),
        SettingsCatalogItem(
            key: SidebarSelection.storageKey,
            title: "Active Sidebar",
            description: "Chooses the built-in sidebar or one provided by an extension.",
            category: .sidebar,
            section: "Sidebar",
            defaultValue: SidebarSelection.builtinValue,
            aliases: ["extension sidebar", "webview sidebar"]
        ),
        SettingsCatalogItem(
            key: WorktreeListPreferences.showUnreadIndicatorKey,
            title: "Show Worktree Unread Indicator",
            description: "Shows a dot on worktrees with unread notifications in the worktree switcher.",
            category: .sidebar,
            section: "Worktrees",
            defaultValue: WorktreeListPreferences.defaultShowUnreadIndicator,
            aliases: ["unread", "badge", "notification dot", "omnibox"]
        ),
        SettingsCatalogItem(
            key: WorktreeListPreferences.orderByMRUKey,
            title: "Order Worktrees by Recent Use",
            description: "Sorts the worktree switcher with the active worktree first, then by most-recently-used.",
            category: .sidebar,
            section: "Worktrees",
            defaultValue: WorktreeListPreferences.defaultOrderByMRU,
            aliases: ["mru", "recent", "sort", "order", "omnibox"]
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
            key: "muxy.remoteDevices.manage",
            title: "Remote Devices",
            description: "Adds and manages reusable SSH connections used by remote workspaces.",
            category: .remoteDevices,
            section: "Remote Devices",
            aliases: ["ssh", "server", "host", "remote", "connection", "device"]
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
            key: TabWidthPreferences.maxWidthKey,
            title: "Tab header width",
            description: "Sets the maximum tab header width in pixels; the widest setting lets tabs fill the titlebar.",
            category: .appearance,
            section: "Interface",
            defaultValue: TabWidthPreferences.defaultMaxWidth,
            aliases: ["tabs", "tab width", "full-width"]
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
            key: ResourceUsagePreferences.visibleKey,
            title: "Show Resource Usage in Status Bar",
            description: "Shows app and subprocess CPU and memory usage in the status bar. Disabling it stops the sampling.",
            category: .appearance,
            section: "Interface",
            defaultValue: ResourceUsagePreferences.defaultVisible
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
            category: .sidebar,
            section: "Sidebar",
            defaultValue: SidebarCollapsedStyle.defaultValue.rawValue
        ),
        SettingsCatalogItem(
            key: SidebarExpandedStyle.storageKey,
            title: "Expanded Sidebar Style",
            description: "Controls the sidebar appearance when expanded.",
            category: .sidebar,
            section: "Sidebar",
            defaultValue: SidebarExpandedStyle.defaultValue.rawValue
        ),
        SettingsCatalogItem(
            key: "editor.richInputImageStrategy",
            title: "Rich Input Image Submission",
            description: "Chooses how rich input submits images.",
            category: .richInput,
            section: "Rich Input",
            defaultValue: RichInputImageStrategy.clipboard.rawValue
        ),
        SettingsCatalogItem(
            key: RichInputPreferences.positionKey,
            title: "Rich Input Position",
            description: "Controls where the rich input panel appears.",
            category: .richInput,
            section: "Rich Input",
            defaultValue: RichInputPreferences.defaultPosition.rawValue
        ),
        SettingsCatalogItem(
            key: RichInputPreferences.floatingKey,
            title: "Floating Rich Input",
            description: "Shows rich input as a floating panel.",
            category: .richInput,
            section: "Rich Input",
            defaultValue: RichInputPreferences.defaultFloating
        ),
        SettingsCatalogItem(
            key: "editor.richInputFontFamily",
            title: "Rich Input Font Family",
            description: "Controls the rich input editor font family.",
            category: .richInput,
            section: "Rich Input",
            defaultValue: EditorSettings.defaultRichInputFontFamily
        ),
        SettingsCatalogItem(
            key: "editor.richInputLineHeightMultiplier",
            title: "Rich Input Line Height",
            description: "Controls line height in rich input.",
            category: .richInput,
            section: "Rich Input",
            defaultValue: Double(EditorSettings.defaultRichInputLineHeightMultiplier)
        ),

        SettingsCatalogItem(
            key: TerminalOfflinePreferences.enabledKey,
            title: "Free Idle Background Terminals",
            description: "Frees a background tab's terminal after it stays idle, reclaiming memory.",
            category: .terminal,
            section: "Memory",
            defaultValue: TerminalOfflinePreferences.defaultIsEnabled
        ),
        SettingsCatalogItem(
            key: TerminalOfflinePreferences.idleThresholdKey,
            title: "Idle Timeout (seconds)",
            description: "How long a background tab stays idle before its terminal is freed.",
            category: .terminal,
            section: "Memory",
            defaultValue: TerminalOfflinePreferences.defaultIdleThreshold
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
            title: "Commands",
            description: "Configures shortcuts that open command tabs.",
            category: .commands,
            section: "Commands",
            aliases: ["command layer", "custom commands", "shortcuts"]
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
            key: NotificationSettings.Key.toastEnabled,
            title: "Toast Notifications",
            description: "Shows toast notifications.",
            category: .notifications,
            section: "Delivery",
            defaultValue: NotificationSettings.Default.toastEnabled
        ),
        SettingsCatalogItem(
            key: NotificationSettings.Key.desktopEnabled,
            title: "Desktop Notifications",
            description: "Shows a macOS notification when Muxy is not frontmost.",
            category: .notifications,
            section: "Delivery",
            defaultValue: NotificationSettings.Default.desktopEnabled
        ),
        SettingsCatalogItem(
            key: NotificationSettings.Key.sound,
            title: "Notification Sound",
            description: "Chooses the notification sound.",
            category: .notifications,
            section: "Sound",
            defaultValue: NotificationSettings.Default.sound.rawValue
        ),
        SettingsCatalogItem(
            key: NotificationSettings.Key.toastPosition,
            title: "Toast Position",
            description: "Controls where toast notifications appear.",
            category: .notifications,
            section: "Toast",
            defaultValue: NotificationSettings.Default.toastPosition.rawValue
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
            key: "backup.export",
            title: "Export Muxy",
            description: "Saves settings, projects, remote devices and customizations to a file.",
            category: .backup,
            section: "Export",
            aliases: ["backup", "migrate", "transfer"]
        ),
        SettingsCatalogItem(
            key: "backup.import",
            title: "Import Muxy",
            description: "Restores a backup and replaces all current Muxy data.",
            category: .backup,
            section: "Import",
            aliases: ["backup", "restore", "migrate"]
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
