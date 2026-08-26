import Foundation

enum GlobalWorkspaceTrigger: String, CaseIterable, Codable, Identifiable {
    case doubleCommand
    case doubleControl
    case doubleOption
    case custom

    var id: String { rawValue }

    var title: String {
        switch self {
        case .doubleCommand:
            "Double Command"
        case .doubleControl:
            "Double Control"
        case .doubleOption:
            "Double Option"
        case .custom:
            "Custom Shortcut"
        }
    }

    var modifier: DoubleModifierShortcutBackend.Modifier? {
        switch self {
        case .doubleCommand:
            .command
        case .doubleControl:
            .control
        case .doubleOption:
            .option
        case .custom:
            nil
        }
    }
}

struct GlobalWorkspaceShortcutConfiguration: Codable, Equatable {
    let trigger: GlobalWorkspaceTrigger
    let customShortcut: QuickTerminalShortcut?

    init(trigger: GlobalWorkspaceTrigger, customShortcut: QuickTerminalShortcut? = nil) {
        self.trigger = trigger
        self.customShortcut = customShortcut
    }
}

enum GlobalWorkspacePreferences {
    static let jsonShortcutKey = "shortcuts.globalWorkspace"
    static let enabledKey = "muxy.globalHotkey.enabled"
    static let triggerKey = "muxy.globalHotkey.trigger"
    static let doubleTapIntervalMillisecondsKey = "muxy.globalHotkey.doubleTapIntervalMilliseconds"
    static let toggleToHideKey = "muxy.globalHotkey.toggleToHide"
    static let customShortcutKey = "muxy.globalHotkey.customShortcut"

    static let defaultEnabled = false
    static let defaultTrigger = GlobalWorkspaceTrigger.doubleCommand
    static let defaultDoubleTapIntervalMilliseconds = 300.0
    static let minimumDoubleTapIntervalMilliseconds = 100.0
    static let maximumDoubleTapIntervalMilliseconds = 1000.0
    static let doubleTapIntervalStepMilliseconds = 25.0
    static let defaultToggleToHide = true

    static func isEnabled(defaults: UserDefaults = .standard) -> Bool {
        guard defaults.object(forKey: enabledKey) != nil else { return defaultEnabled }
        return defaults.bool(forKey: enabledKey)
    }

    static func trigger(defaults: UserDefaults = .standard) -> GlobalWorkspaceTrigger {
        guard let rawValue = defaults.string(forKey: triggerKey),
              let trigger = GlobalWorkspaceTrigger(rawValue: rawValue)
        else { return defaultTrigger }
        return trigger
    }

    static func doubleTapIntervalMilliseconds(defaults: UserDefaults = .standard) -> Double {
        guard defaults.object(forKey: doubleTapIntervalMillisecondsKey) != nil else {
            return defaultDoubleTapIntervalMilliseconds
        }
        return clampedDoubleTapIntervalMilliseconds(defaults.double(forKey: doubleTapIntervalMillisecondsKey))
    }

    static func doubleTapInterval(defaults: UserDefaults = .standard) -> TimeInterval {
        doubleTapIntervalMilliseconds(defaults: defaults) / 1000
    }

    static func toggleToHide(defaults: UserDefaults = .standard) -> Bool {
        guard defaults.object(forKey: toggleToHideKey) != nil else { return defaultToggleToHide }
        return defaults.bool(forKey: toggleToHideKey)
    }

    static func customShortcut(defaults: UserDefaults = .standard) -> QuickTerminalShortcut? {
        guard let data = defaults.data(forKey: customShortcutKey),
              let shortcut = try? JSONDecoder().decode(QuickTerminalShortcut.self, from: data),
              case .keyCombo = shortcut
        else { return nil }
        return shortcut
    }

    static func shortcutConfiguration(defaults: UserDefaults = .standard) -> GlobalWorkspaceShortcutConfiguration {
        GlobalWorkspaceShortcutConfiguration(
            trigger: trigger(defaults: defaults),
            customShortcut: customShortcut(defaults: defaults)
        )
    }

    @MainActor
    static func setCustomShortcut(
        _ shortcut: QuickTerminalShortcut,
        defaults: UserDefaults = .standard
    ) throws {
        guard case .keyCombo = shortcut,
              let canonicalized = shortcut.canonicalizedForCurrentKeyboardLayout()
        else { throw QuickTerminalShortcutError.invalidShortcut }
        try defaults.set(JSONEncoder().encode(canonicalized), forKey: customShortcutKey)
        defaults.set(GlobalWorkspaceTrigger.custom.rawValue, forKey: triggerKey)
    }

    @MainActor
    static func setShortcutConfiguration(
        _ configuration: GlobalWorkspaceShortcutConfiguration,
        defaults: UserDefaults = .standard
    ) throws {
        if configuration.trigger == .custom {
            guard let customShortcut = configuration.customShortcut else {
                throw QuickTerminalShortcutError.invalidShortcut
            }
            try setCustomShortcut(customShortcut, defaults: defaults)
            return
        }
        defaults.removeObject(forKey: customShortcutKey)
        defaults.set(configuration.trigger.rawValue, forKey: triggerKey)
    }

    static func resetToDefaults(defaults: UserDefaults = .standard) {
        defaults.removeObject(forKey: enabledKey)
        defaults.removeObject(forKey: triggerKey)
        defaults.removeObject(forKey: doubleTapIntervalMillisecondsKey)
        defaults.removeObject(forKey: toggleToHideKey)
        defaults.removeObject(forKey: customShortcutKey)
    }

    static func clampedDoubleTapIntervalMilliseconds(_ value: Double) -> Double {
        min(max(value, minimumDoubleTapIntervalMilliseconds), maximumDoubleTapIntervalMilliseconds)
    }
}
