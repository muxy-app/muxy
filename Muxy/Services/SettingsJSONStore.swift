import Foundation
import os

private let settingsJSONLogger = Logger(subsystem: "app.muxy", category: "SettingsJSONStore")

@MainActor
enum SettingsJSONStore {
    private static var defaultsObserver: NSObjectProtocol?
    private static var isApplyingSettings = false
    private static var isSyncingFile = false

    static var userSettingsURL: URL {
        MuxyFileStorage.fileURL(filename: SettingsCatalog.userSettingsFilename)
    }

    static var systemSettingsText: String {
        prettyJSONString(defaultSettingsDictionary())
    }

    static func loadUserSettingsText() -> String {
        ensureUserSettingsFileExists()
        let text = (try? String(contentsOf: userSettingsURL, encoding: .utf8)) ?? "{}"
        return (try? prettifiedSettingsText(text)) ?? text
    }

    static func saveUserSettingsText(_ text: String) throws {
        let data = Data(text.utf8)
        let object = try JSONSerialization.jsonObject(with: data)
        guard let dictionary = object as? [String: Any] else {
            throw SettingsJSONError.topLevelObjectRequired
        }
        let settings = try validatedSettings(from: dictionary)
        try Data(prettyJSONString(dictionary).utf8).write(to: userSettingsURL, options: .atomic)
        try FileManager.default.setAttributes([.posixPermissions: FilePermissions.privateFile], ofItemAtPath: userSettingsURL.path)
        isApplyingSettings = true
        apply(settings)
        isApplyingSettings = false
        syncUserSettingsFileWithCurrentSettings()
    }

    static func prettifiedSettingsText(_ text: String) throws -> String {
        let data = Data(text.utf8)
        let object = try JSONSerialization.jsonObject(with: data)
        guard let dictionary = object as? [String: Any] else {
            throw SettingsJSONError.topLevelObjectRequired
        }
        return prettyJSONString(dictionary)
    }

    static func resetUserSettingsFile() {
        let current = currentSettingsDictionary()
        let text = prettyJSONString(current)
        do {
            try text.write(to: userSettingsURL, atomically: true, encoding: .utf8)
            try FileManager.default.setAttributes([.posixPermissions: FilePermissions.privateFile], ofItemAtPath: userSettingsURL.path)
        } catch {
            settingsJSONLogger.error("Failed to reset user settings file: \(error.localizedDescription)")
        }
    }

    static func beginAutomaticUserSettingsSync() {
        guard defaultsObserver == nil else { return }
        defaultsObserver = NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification,
            object: UserDefaults.standard,
            queue: .main
        ) { _ in
            Task { @MainActor in
                guard !isApplyingSettings else { return }
                syncUserSettingsFileWithCurrentSettings()
            }
        }
        syncUserSettingsFileWithCurrentSettings()
    }

    static func syncUserSettingsFileWithCurrentSettings() {
        guard !isApplyingSettings, !isSyncingFile else { return }
        isSyncingFile = true
        defer { isSyncingFile = false }
        var dictionary = existingUserSettingsDictionary()
        for (key, value) in currentSettingsDictionary() {
            dictionary[key] = value
        }
        do {
            try Data(prettyJSONString(dictionary).utf8).write(to: userSettingsURL, options: .atomic)
            try FileManager.default.setAttributes([.posixPermissions: FilePermissions.privateFile], ofItemAtPath: userSettingsURL.path)
        } catch {
            settingsJSONLogger.error("Failed to sync user settings file: \(error.localizedDescription)")
        }
    }

    private static func ensureUserSettingsFileExists() {
        guard !FileManager.default.fileExists(atPath: userSettingsURL.path) else { return }
        resetUserSettingsFile()
    }

    private static func defaultSettingsDictionary() -> [String: Any] {
        var dictionary: [String: Any] = Dictionary(uniqueKeysWithValues: SettingsCatalog.jsonEditableItems.compactMap { item in
            guard let value = item.defaultValue else { return nil }
            return (item.key, jsonValue(value))
        })
        dictionary["shortcuts.app"] = keyBindingsJSONObject(KeyBinding.defaults)
        dictionary["shortcuts.customCommands"] = commandShortcutsJSONObject(CommandShortcutConfiguration())
        dictionary["ai.providers"] = notificationProviderSettings(defaultValue: true)
        dictionary["aiUsage.providers"] = aiUsageProviderSettings(defaultValue: false)
        dictionary["mobile.approvedDevices"] = []
        return dictionary
    }

    private static func currentSettingsDictionary() -> [String: Any] {
        var dictionary = Dictionary(uniqueKeysWithValues: SettingsCatalog.jsonEditableItems.map { item in
            let value = currentValue(for: item) ?? item.defaultValue.map(jsonValue) ?? NSNull()
            return (item.key, value)
        })
        dictionary["shortcuts.app"] = keyBindingsJSONObject(KeyBindingStore.shared.bindings)
        dictionary["shortcuts.customCommands"] = commandShortcutsJSONObject(CommandShortcutConfiguration(
            prefixCombo: CommandShortcutStore.shared.prefixCombo,
            shortcuts: CommandShortcutStore.shared.shortcuts
        ))
        dictionary["ai.providers"] = notificationProviderSettings()
        dictionary["aiUsage.providers"] = aiUsageProviderSettings()
        dictionary["mobile.approvedDevices"] = codableJSONObject(ApprovedDevicesStore.shared.devices) ?? []
        return dictionary
    }

    private static func validatedSettings(from dictionary: [String: Any]) throws -> [String: Any] {
        let itemsByKey = Dictionary(uniqueKeysWithValues: SettingsCatalog.jsonEditableItems.map { ($0.key, $0) })
        var settings: [String: Any] = [:]
        for (key, value) in dictionary {
            if isSpecialJSONSetting(key) {
                settings[key] = try validatedSpecialValue(value, key: key)
                continue
            }
            guard let item = itemsByKey[key] else { continue }
            settings[key] = try validatedValue(value, for: item)
        }
        return settings
    }

    private static func apply(_ dictionary: [String: Any]) {
        for (key, value) in dictionary {
            if applySpecialSetting(key: key, value: value) {
                continue
            }
            if applyEditorSetting(key: key, value: value) {
                continue
            }
            if value is NSNull {
                UserDefaults.standard.removeObject(forKey: key)
            } else {
                UserDefaults.standard.set(value, forKey: key)
            }
        }
    }

    private static func existingUserSettingsDictionary() -> [String: Any] {
        guard let data = try? Data(contentsOf: userSettingsURL),
              let object = try? JSONSerialization.jsonObject(with: data),
              let dictionary = object as? [String: Any]
        else { return [:] }
        return dictionary
    }

    private static func validatedValue(_ value: Any, for item: SettingsCatalogItem) throws -> Any {
        guard !(value is NSNull) else { return value }
        guard let defaultValue = item.defaultValue?.base else { throw SettingsJSONError.unsupportedValue(item.key) }
        if defaultValue is Bool, value is Bool { return value }
        if defaultValue is String, let string = value as? String {
            try validateAllowedString(string, key: item.key)
            return string
        }
        if defaultValue is Int, let int = value as? Int {
            try validateAllowedInt(int, key: item.key)
            return int
        }
        if defaultValue is UInt16, let int = value as? Int {
            try validateAllowedInt(int, key: item.key)
            return int
        }
        if defaultValue is Double, let number = value as? NSNumber {
            let double = number.doubleValue
            try validateAllowedDouble(double, key: item.key)
            return double
        }
        if defaultValue is CGFloat, let number = value as? NSNumber {
            let double = number.doubleValue
            try validateAllowedDouble(double, key: item.key)
            return double
        }
        if defaultValue is [String], let strings = value as? [String] { return strings }
        throw SettingsJSONError.invalidValue(item.key)
    }

    private static func validateAllowedString(_ value: String, key: String) throws {
        let allowedValues: [String: Set<String>] = [
            UpdateChannel.storageKey: Set(UpdateChannel.allCases.map(\.rawValue)),
            GeneralSettingsKeys.fileTreeSource: Set(FileTreeSourcePreference.allCases.map(\.rawValue)),
            ProjectPickerPreferences.storageKey: Set(ProjectPickerMode.allCases.map(\.rawValue)),
            SentryConsent.storageKey: Set(["", SentryConsent.allowed.rawValue, SentryConsent.denied.rawValue]),
            "muxy.ui.scale": Set(UIScale.Preset.allCases.map(\.rawValue)),
            SidebarCollapsedStyle.storageKey: Set(SidebarCollapsedStyle.allCases.map(\.rawValue)),
            SidebarExpandedStyle.storageKey: Set(SidebarExpandedStyle.allCases.map(\.rawValue)),
            "muxy.vcsDisplayMode": Set(VCSDisplayMode.allCases.map(\.rawValue)),
            RichInputPreferences.positionKey: Set(RichInputPanelPosition.allCases.map(\.rawValue)),
            "editor.defaultEditor": Set(EditorSettings.DefaultEditor.allCases.map(\.rawValue)),
            "editor.htmlDefaultViewMode": Set(EditorMarkdownViewMode.allCases.map(\.rawValue)),
            "editor.richInputImageStrategy": Set(RichInputImageStrategy.allCases.map(\.rawValue)),
            "muxy.notifications.sound": Set(NotificationSound.allCases.map(\.rawValue)),
            "muxy.notifications.toastPosition": Set(ToastPosition.allCases.map(\.rawValue)),
            AIAssistantSettings.providerKey: Set(AIAssistantProvider.allCases.map(\.rawValue)),
            AIUsageSettingsStore.usageDisplayModeKey: Set(AIUsageDisplayMode.allCases.map(\.rawValue)),
        ]
        guard let allowed = allowedValues[key] else { return }
        guard allowed.contains(value) else { throw SettingsJSONError.invalidValue(key) }
    }

    private static func validateAllowedInt(_ value: Int, key: String) throws {
        if key == MobileServerService.portKey {
            guard let port = UInt16(exactly: value), MobileServerService.isValid(port: port) else {
                throw SettingsJSONError.invalidValue(key)
            }
            return
        }
        if key == AIUsageSettingsStore.autoRefreshIntervalKey {
            guard AIUsageAutoRefreshInterval(rawValue: value) != nil else { throw SettingsJSONError.invalidValue(key) }
        }
    }

    private static func validateAllowedDouble(_ value: Double, key: String) throws {
        switch key {
        case "editor.fontSize":
            guard (8 ... 36).contains(value) else { throw SettingsJSONError.invalidValue(key) }
        case "editor.markdownPreviewFontScale":
            guard (Double(EditorSettings.minMarkdownPreviewFontScale) ... Double(EditorSettings.maxMarkdownPreviewFontScale))
                .contains(value)
            else { throw SettingsJSONError.invalidValue(key) }
        case "editor.lineHeightMultiplier",
             "editor.richInputLineHeightMultiplier":
            guard (Double(EditorSettings.minLineHeightMultiplier) ... Double(EditorSettings.maxLineHeightMultiplier))
                .contains(value)
            else { throw SettingsJSONError.invalidValue(key) }
        default:
            return
        }
    }

    private static func currentValue(for item: SettingsCatalogItem) -> Any? {
        let settings = EditorSettings.shared
        return switch item.key {
        case SentryConsent.storageKey: SentryService.shared.consent?.rawValue ?? ""
        case "muxy.ui.scale": UIScale.shared.preset.rawValue
        case "muxy.theme.light": ThemeService.shared.currentLightThemeName() ?? ThemeService.defaultThemeName
        case "muxy.theme.dark": ThemeService.shared.currentDarkThemeName() ?? ThemeService.defaultThemeName
        case ProjectPickerDefaultLocation.storageKey: UserDefaults.standard.string(forKey: item.key) ?? ""
        case "editor.defaultEditor": settings.defaultEditor.rawValue
        case "editor.externalEditorCommand": settings.externalEditorCommand
        case "editor.markdownPreviewFontFamily": settings.markdownPreviewFontFamily
        case "editor.markdownPreviewFontScale": Double(settings.markdownPreviewFontScale)
        case "editor.htmlDefaultViewMode": settings.htmlDefaultViewMode.rawValue
        case "editor.richInputImageStrategy": settings.richInputImageStrategy.rawValue
        case "editor.richInputFontFamily": settings.richInputFontFamily
        case "editor.richInputLineHeightMultiplier": Double(settings.richInputLineHeightMultiplier)
        case "editor.highlightCurrentLine": settings.highlightCurrentLine
        case "editor.showLineNumbers": settings.showLineNumbers
        case "editor.lineWrapping": settings.lineWrapping
        case "editor.fontFamily": settings.fontFamily
        case "editor.fontSize": Double(settings.fontSize)
        case "editor.lineHeightMultiplier": Double(settings.lineHeightMultiplier)
        default: UserDefaults.standard.object(forKey: item.key)
        }
    }

    private static func isSpecialJSONSetting(_ key: String) -> Bool {
        switch key {
        case "shortcuts.app",
             "shortcuts.customCommands",
             "ai.providers",
             "aiUsage.providers",
             "mobile.approvedDevices":
            true
        default:
            false
        }
    }

    private static func validatedSpecialValue(_ value: Any, key: String) throws -> Any {
        switch key {
        case "shortcuts.app":
            guard let bindings = keyBindings(from: value), !bindings.isEmpty else { throw SettingsJSONError.invalidValue(key) }
        case "shortcuts.customCommands":
            guard let configuration = commandShortcutConfiguration(from: value), isValidKeyCombo(configuration.prefixCombo),
                  configuration.shortcuts.allSatisfy({ isValidKeyCombo($0.combo) })
            else { throw SettingsJSONError.invalidValue(key) }
        case "ai.providers",
             "aiUsage.providers":
            guard let values = value as? [String: Any], values.values.allSatisfy({ $0 is Bool }) else {
                throw SettingsJSONError.invalidValue(key)
            }
        case "mobile.approvedDevices":
            guard (codableValue(from: value) as [ApprovedDevice]?) != nil else { throw SettingsJSONError.invalidValue(key) }
        default:
            throw SettingsJSONError.invalidValue(key)
        }
        return value
    }

    private static func applySpecialSetting(key: String, value: Any) -> Bool {
        switch key {
        case SentryConsent.storageKey:
            guard let rawValue = value as? String else { return false }
            if rawValue.isEmpty {
                UserDefaults.standard.removeObject(forKey: key)
            } else if let consent = SentryConsent(rawValue: rawValue) {
                SentryService.shared.setConsent(consent)
            }
        case "muxy.ui.scale":
            guard let rawValue = value as? String, let preset = UIScale.Preset(rawValue: rawValue) else { return false }
            UIScale.shared.preset = preset
        case "muxy.theme.light":
            guard let value = value as? String else { return false }
            ThemeService.shared.applyLightTheme(value)
        case "muxy.theme.dark":
            guard let value = value as? String else { return false }
            ThemeService.shared.applyDarkTheme(value)
        case ProjectPickerDefaultLocation.storageKey:
            guard let value = value as? String else { return false }
            if value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                ProjectPickerDefaultLocation.resetToAppDefault()
            } else {
                ProjectPickerDefaultLocation.setCustomPath(value)
            }
        case "shortcuts.app":
            guard let bindings = keyBindings(from: value) else { return true }
            KeyBindingStore.shared.replaceBindings(bindings)
        case "shortcuts.customCommands":
            guard let configuration = commandShortcutConfiguration(from: value) else { return true }
            CommandShortcutStore.shared.replaceConfiguration(configuration)
        case "ai.providers":
            guard let values = value as? [String: Any] else { return true }
            for provider in AIProviderRegistry.shared.providers {
                guard let enabled = values[provider.id] as? Bool else { continue }
                provider.isEnabled = enabled
            }
            AIProviderRegistry.shared.installAll()
        case "aiUsage.providers":
            guard let values = value as? [String: Any] else { return true }
            for provider in AIUsageProviderCatalog.providers {
                guard let enabled = values[provider.id] as? Bool else { continue }
                AIUsageProviderTrackingStore.setTracked(enabled, providerID: provider.id)
            }
            AIUsageService.shared.recomposeSnapshots()
        case "mobile.approvedDevices":
            guard let devices: [ApprovedDevice] = codableValue(from: value) else { return true }
            ApprovedDevicesStore.shared.replaceDevices(devices)
        default:
            return false
        }
        return true
    }

    private static func applyEditorSetting(key: String, value: Any) -> Bool {
        guard !(value is NSNull) else { return false }
        let settings = EditorSettings.shared
        switch key {
        case "editor.defaultEditor":
            guard let rawValue = value as? String, let editor = EditorSettings.DefaultEditor(rawValue: rawValue) else { return false }
            settings.defaultEditor = editor
        case "editor.externalEditorCommand":
            guard let value = value as? String else { return false }
            settings.externalEditorCommand = value
        case "editor.markdownPreviewFontFamily":
            guard let value = value as? String else { return false }
            settings.markdownPreviewFontFamily = value
        case "editor.markdownPreviewFontScale":
            guard let value = doubleValue(value) else { return false }
            settings.markdownPreviewFontScale = CGFloat(value)
        case "editor.htmlDefaultViewMode":
            guard let rawValue = value as? String, let mode = EditorMarkdownViewMode(rawValue: rawValue) else { return false }
            settings.htmlDefaultViewMode = mode
        case "editor.richInputImageStrategy":
            guard let rawValue = value as? String, let strategy = RichInputImageStrategy(rawValue: rawValue) else { return false }
            settings.richInputImageStrategy = strategy
        case "editor.richInputFontFamily":
            guard let value = value as? String else { return false }
            settings.richInputFontFamily = value
        case "editor.richInputLineHeightMultiplier":
            guard let value = doubleValue(value) else { return false }
            settings.richInputLineHeightMultiplier = CGFloat(value)
        case "editor.highlightCurrentLine":
            guard let value = value as? Bool else { return false }
            settings.highlightCurrentLine = value
        case "editor.showLineNumbers":
            guard let value = value as? Bool else { return false }
            settings.showLineNumbers = value
        case "editor.lineWrapping":
            guard let value = value as? Bool else { return false }
            settings.lineWrapping = value
        case "editor.fontFamily":
            guard let value = value as? String else { return false }
            settings.fontFamily = value
        case "editor.fontSize":
            guard let value = doubleValue(value) else { return false }
            settings.fontSize = CGFloat(value)
        case "editor.lineHeightMultiplier":
            guard let value = doubleValue(value) else { return false }
            settings.lineHeightMultiplier = CGFloat(value)
        default:
            return false
        }
        return true
    }

    private static func doubleValue(_ value: Any) -> Double? {
        if let value = value as? Double { return value }
        if let value = value as? Int { return Double(value) }
        if let value = value as? NSNumber { return value.doubleValue }
        return nil
    }

    private static func keyBindingsJSONObject(_ bindings: [KeyBinding]) -> Any {
        Dictionary(uniqueKeysWithValues: bindings.map { binding in
            (binding.action.rawValue, codableJSONObject(binding.combo) ?? [:])
        })
    }

    private static func keyBindings(from value: Any) -> [KeyBinding]? {
        guard let dictionary = value as? [String: Any] else { return nil }
        var bindings: [KeyBinding] = []
        for (key, value) in dictionary {
            guard let action = ShortcutAction(rawValue: key), let combo: KeyCombo = codableValue(from: value),
                  isValidAppKeyCombo(combo)
            else {
                return nil
            }
            bindings.append(KeyBinding(action: action, combo: combo))
        }
        return bindings
    }

    private static func isValidKeyCombo(_ combo: KeyCombo) -> Bool {
        !combo.key.isEmpty
            && KeyCombo.normalized(key: combo.key) == combo.key
            && KeyCombo.normalized(modifiers: combo.modifiers) == combo.modifiers
    }

    private static func isValidAppKeyCombo(_ combo: KeyCombo) -> Bool {
        !combo.isAssigned || isValidKeyCombo(combo)
    }

    private static func commandShortcutsJSONObject(_ configuration: CommandShortcutConfiguration) -> Any {
        codableJSONObject(StoredCommandShortcutJSON(
            prefixCombo: configuration.prefixCombo,
            shortcuts: configuration.shortcuts
        )) ?? [:]
    }

    private static func commandShortcutConfiguration(from value: Any) -> CommandShortcutConfiguration? {
        guard let stored: StoredCommandShortcutJSON = codableValue(from: value) else { return nil }
        return CommandShortcutConfiguration(prefixCombo: stored.prefixCombo, shortcuts: stored.shortcuts)
    }

    private static func notificationProviderSettings(defaultValue: Bool? = nil) -> [String: Bool] {
        Dictionary(uniqueKeysWithValues: AIProviderRegistry.shared.providers.map { provider in
            (provider.id, defaultValue ?? provider.isEnabled)
        })
    }

    private static func aiUsageProviderSettings(defaultValue: Bool? = nil) -> [String: Bool] {
        Dictionary(uniqueKeysWithValues: AIUsageProviderCatalog.providers.map { provider in
            (provider.id, defaultValue ?? AIUsageProviderTrackingStore.isTracked(providerID: provider.id))
        })
    }

    private static func codableJSONObject(_ value: some Encodable) -> Any? {
        guard let data = try? JSONEncoder().encode(value) else { return nil }
        return try? JSONSerialization.jsonObject(with: data)
    }

    private static func codableValue<Value: Decodable>(from value: Any) -> Value? {
        guard JSONSerialization.isValidJSONObject(value),
              let data = try? JSONSerialization.data(withJSONObject: value)
        else { return nil }
        return try? JSONDecoder().decode(Value.self, from: data)
    }

    private static func jsonValue(_ value: AnyHashable) -> Any {
        if let array = value.base as? [String] { return array }
        if let value = value.base as? UInt16 { return Int(value) }
        return value.base
    }

    private static func prettyJSONString(_ dictionary: [String: Any]) -> String {
        guard let data = try? JSONSerialization.data(
            withJSONObject: dictionary,
            options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        ), let text = String(data: data, encoding: .utf8)
        else { return "{}" }
        return text + "\n"
    }
}

private struct StoredCommandShortcutJSON: Codable {
    let prefixCombo: KeyCombo
    let shortcuts: [CommandShortcut]
}

enum SettingsJSONError: LocalizedError {
    case topLevelObjectRequired
    case unsupportedValue(String)
    case invalidValue(String)

    var errorDescription: String? {
        switch self {
        case .topLevelObjectRequired:
            "Settings JSON must be an object."
        case let .unsupportedValue(key):
            "Unsupported JSON value for \"\(key)\"."
        case let .invalidValue(key):
            "Invalid JSON value for \"\(key)\"."
        }
    }
}
