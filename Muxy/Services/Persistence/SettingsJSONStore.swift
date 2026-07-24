import Foundation
import os

private let settingsJSONLogger = Logger(subsystem: "app.muxy", category: "SettingsJSONStore")

@MainActor
enum SettingsJSONStore {
    typealias QuickTerminalShortcutUpdater = @MainActor (QuickTerminalShortcut) throws -> Void
    typealias QuickTerminalEnabledUpdater = @MainActor (Bool) -> Void
    typealias QuickTerminalEnabledResetter = @MainActor () -> Void

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

    static func saveUserSettingsText(
        _ text: String,
        quickTerminalShortcutUpdater: QuickTerminalShortcutUpdater = {
            try QuickTerminalShortcutService.shared.updateShortcut($0)
        },
        quickTerminalEnabledUpdater: QuickTerminalEnabledUpdater = {
            QuickTerminalPreferences.setEnabled($0)
        },
        quickTerminalEnabledResetter: QuickTerminalEnabledResetter = {
            QuickTerminalPreferences.resetEnabled()
        }
    ) throws {
        let data = Data(text.utf8)
        let object = try JSONSerialization.jsonObject(with: data)
        guard let dictionary = object as? [String: Any] else {
            throw SettingsJSONError.topLevelObjectRequired
        }
        let settings = try validatedSettings(from: dictionary)
        let previousData = try? Data(contentsOf: userSettingsURL)
        do {
            try Data(prettyJSONString(dictionary).utf8).write(to: userSettingsURL, options: .atomic)
            try FileManager.default.setAttributes(
                [.posixPermissions: FilePermissions.privateFile],
                ofItemAtPath: userSettingsURL.path
            )
            isApplyingSettings = true
            try apply(
                settings,
                quickTerminalShortcutUpdater: quickTerminalShortcutUpdater,
                quickTerminalEnabledUpdater: quickTerminalEnabledUpdater,
                quickTerminalEnabledResetter: quickTerminalEnabledResetter
            )
            isApplyingSettings = false
        } catch {
            isApplyingSettings = false
            restoreUserSettingsFile(previousData)
            throw error
        }
        syncUserSettingsFileWithCurrentSettings()
    }

    static func applyUserSettingsFile() throws {
        let text = try String(contentsOf: userSettingsURL, encoding: .utf8)
        try saveUserSettingsText(text)
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

    @discardableResult
    static func syncUserSettingsFileWithCurrentSettings() -> Bool {
        guard !isApplyingSettings, !isSyncingFile else { return false }
        isSyncingFile = true
        defer { isSyncingFile = false }
        var dictionary = existingUserSettingsDictionary()
        for (key, value) in currentSettingsDictionary() {
            dictionary[key] = value
        }
        let data = Data(prettyJSONString(dictionary).utf8)
        if (try? Data(contentsOf: userSettingsURL)) == data {
            return false
        }
        do {
            try data.write(to: userSettingsURL, options: .atomic)
            try FileManager.default.setAttributes([.posixPermissions: FilePermissions.privateFile], ofItemAtPath: userSettingsURL.path)
            return true
        } catch {
            settingsJSONLogger.error("Failed to sync user settings file: \(error.localizedDescription)")
            return false
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
        dictionary["shortcuts.quickTerminal"] = codableJSONObject(QuickTerminalShortcut.default) ?? [:]
        dictionary["shortcuts.customCommands"] = commandShortcutsJSONObject(CommandShortcutConfiguration())
        dictionary["ai.providers"] = notificationProviderSettings(defaultValue: true)
        dictionary["mobile.approvedDevices"] = []
        return dictionary
    }

    private static func currentSettingsDictionary() -> [String: Any] {
        var dictionary = Dictionary(uniqueKeysWithValues: SettingsCatalog.jsonEditableItems.map { item in
            let value = currentValue(for: item) ?? item.defaultValue.map(jsonValue) ?? NSNull()
            return (item.key, value)
        })
        dictionary["shortcuts.app"] = keyBindingsJSONObject(KeyBindingStore.shared.bindings)
        dictionary["shortcuts.quickTerminal"] = codableJSONObject(QuickTerminalShortcutService.shared.shortcut) ?? [:]
        dictionary["shortcuts.customCommands"] = commandShortcutsJSONObject(CommandShortcutConfiguration(
            prefixCombo: CommandShortcutStore.shared.prefixCombo,
            shortcuts: CommandShortcutStore.shared.shortcuts
        ))
        dictionary["ai.providers"] = notificationProviderSettings()
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
        try validateQuickTerminalShortcutConflicts(in: settings)
        return settings
    }

    private static func validateQuickTerminalShortcutConflicts(in settings: [String: Any]) throws {
        let shortcut: QuickTerminalShortcut = settings["shortcuts.quickTerminal"]
            .flatMap { codableValue(from: $0) }
            ?? QuickTerminalShortcutService.shared.shortcut
        guard let combo = shortcut.keyCombo else { return }

        let bindings = settings["shortcuts.app"].flatMap(keyBindings(from:))
            ?? KeyBindingStore.shared.bindings
        guard !bindings.contains(where: { $0.combo == combo }) else {
            throw SettingsJSONError.invalidValue("shortcuts.quickTerminal")
        }

        let commandConfiguration = settings["shortcuts.customCommands"].flatMap(commandShortcutConfiguration(from:))
            ?? CommandShortcutConfiguration(
                prefixCombo: CommandShortcutStore.shared.prefixCombo,
                shortcuts: CommandShortcutStore.shared.shortcuts
            )
        guard commandConfiguration.prefixCombo != combo,
              !commandConfiguration.shortcuts.contains(where: { $0.combo == combo }),
              ExtensionShortcutStore.shared.conflictingShortcut(
                  for: combo,
                  excludingExtensionID: nil,
                  commandID: nil
              ) == nil
        else {
            throw SettingsJSONError.invalidValue("shortcuts.quickTerminal")
        }
    }

    private static func apply(
        _ dictionary: [String: Any],
        quickTerminalShortcutUpdater: QuickTerminalShortcutUpdater,
        quickTerminalEnabledUpdater: QuickTerminalEnabledUpdater,
        quickTerminalEnabledResetter: QuickTerminalEnabledResetter
    ) throws {
        if let quickTerminalShortcut = dictionary["shortcuts.quickTerminal"] {
            _ = try applySpecialSetting(
                key: "shortcuts.quickTerminal",
                value: quickTerminalShortcut,
                quickTerminalShortcutUpdater: quickTerminalShortcutUpdater
            )
        }
        if let enabled = dictionary[QuickTerminalPreferences.enabledKey] as? Bool {
            quickTerminalEnabledUpdater(enabled)
        } else if dictionary[QuickTerminalPreferences.enabledKey] is NSNull {
            quickTerminalEnabledResetter()
        }
        for (key, value) in dictionary where key != "shortcuts.quickTerminal"
            && key != QuickTerminalPreferences.enabledKey
        {
            if try applySpecialSetting(
                key: key,
                value: value,
                quickTerminalShortcutUpdater: quickTerminalShortcutUpdater
            ) {
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

    private static func restoreUserSettingsFile(_ data: Data?) {
        do {
            if let data {
                try data.write(to: userSettingsURL, options: .atomic)
                try FileManager.default.setAttributes(
                    [.posixPermissions: FilePermissions.privateFile],
                    ofItemAtPath: userSettingsURL.path
                )
            } else if FileManager.default.fileExists(atPath: userSettingsURL.path) {
                try FileManager.default.removeItem(at: userSettingsURL)
            }
        } catch {
            settingsJSONLogger.error("Failed to restore the user settings file: \(error.localizedDescription)")
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
        if item.key == TabWidthPreferences.maxWidthKey {
            guard !isBooleanValue(value), let double = doubleValue(value) else { throw SettingsJSONError.invalidValue(item.key) }
            try validateAllowedDouble(double, key: item.key)
            return double
        }
        guard let defaultValue = item.defaultValue?.base else { throw SettingsJSONError.unsupportedValue(item.key) }
        if defaultValue is Bool, value is Bool {
            return value
        }
        if defaultValue is String, let string = value as? String {
            try validateAllowedString(string, key: item.key)
            return string
        }
        if defaultValue is Int, !isBooleanValue(value), let int = value as? Int {
            try validateAllowedInt(int, key: item.key)
            return int
        }
        if defaultValue is UInt16, !isBooleanValue(value), let int = value as? Int {
            try validateAllowedInt(int, key: item.key)
            return int
        }
        if defaultValue is Double, !isBooleanValue(value), let double = doubleValue(value) {
            try validateAllowedDouble(double, key: item.key)
            return double
        }
        if defaultValue is CGFloat, !isBooleanValue(value), let double = doubleValue(value) {
            try validateAllowedDouble(double, key: item.key)
            return double
        }
        if defaultValue is [String], let strings = value as? [String] {
            return strings
        }
        throw SettingsJSONError.invalidValue(item.key)
    }

    private static func validateAllowedString(_ value: String, key: String) throws {
        if key == GeneralSettingsKeys.defaultWorktreePathTemplate {
            guard WorktreeLocationResolver.normalizedLocation(value) != nil else { return }
            guard WorktreeLocationResolver.pathTemplateValidationMessage(value) == nil else {
                throw SettingsJSONError.invalidValue(key)
            }
            return
        }
        if RepositoryAIAction.allCases.contains(where: { $0.providerKey == key }) {
            let providerIDs = Set(AIProviderRegistry.shared.agentLaunchProviders.map(\.id))
            guard value.isEmpty || providerIDs.contains(value) else {
                throw SettingsJSONError.invalidValue(key)
            }
            return
        }
        let allowedValues: [String: Set<String>] = [
            UpdateChannel.storageKey: Set(UpdateChannel.allCases.map(\.rawValue)),
            ProjectPickerPreferences.storageKey: Set(ProjectPickerMode.allCases.map(\.rawValue)),
            SentryConsent.storageKey: Set(["", SentryConsent.allowed.rawValue, SentryConsent.denied.rawValue]),
            "muxy.ui.scale": Set(UIScale.Preset.allCases.map(\.rawValue)),
            AppBackgroundStyle.storageKey: Set(AppBackgroundStyle.allCases.map(\.rawValue)),
            SidebarCollapsedStyle.storageKey: Set(SidebarCollapsedStyle.allCases.map(\.rawValue)),
            SidebarExpandedStyle.storageKey: Set(SidebarExpandedStyle.allCases.map(\.rawValue)),
            RichInputPreferences.positionKey: Set(PanelPosition.allCases.map(\.rawValue)),
            "editor.richInputImageStrategy": Set(RichInputImageStrategy.allCases.map(\.rawValue)),
            NotificationSettings.Key.sound: Set(NotificationSound.allCases.map(\.rawValue)),
            NotificationSettings.Key.toastPosition: Set(ToastPosition.allCases.map(\.rawValue)),
        ]
        guard let allowed = allowedValues[key] else { return }
        guard allowed.contains(value) else { throw SettingsJSONError.invalidValue(key) }
    }

    private static func validateAllowedInt(_ value: Int, key: String) throws {
        switch key {
        case MobileServerService.portKey:
            guard let port = UInt16(exactly: value), MobileServerService.isValid(port: port) else {
                throw SettingsJSONError.invalidValue(key)
            }
        case QuickTerminalSizePreferences.widthKey:
            guard QuickTerminalSizePreferences.widthRange.contains(value) else {
                throw SettingsJSONError.invalidValue(key)
            }
        case QuickTerminalSizePreferences.heightKey:
            guard QuickTerminalSizePreferences.heightRange.contains(value) else {
                throw SettingsJSONError.invalidValue(key)
            }
        case QuickTerminalAppearancePreferences.transparencyKey:
            guard QuickTerminalAppearancePreferences.transparencyRange.contains(value) else {
                throw SettingsJSONError.invalidValue(key)
            }
        case QuickTerminalAppearancePreferences.blurIntensityKey:
            guard QuickTerminalAppearancePreferences.blurIntensityRange.contains(value) else {
                throw SettingsJSONError.invalidValue(key)
            }
        default:
            break
        }
    }

    private static func validateAllowedDouble(_ value: Double, key: String) throws {
        switch key {
        case TabWidthPreferences.maxWidthKey:
            guard TabWidthPreferences.isAllowedStoredValue(value) else { throw SettingsJSONError.invalidValue(key) }
        case "editor.richInputLineHeightMultiplier":
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
        case "editor.richInputImageStrategy": settings.richInputImageStrategy.rawValue
        case "editor.richInputFontFamily": settings.richInputFontFamily
        case "editor.richInputLineHeightMultiplier": Double(settings.richInputLineHeightMultiplier)
        default: UserDefaults.standard.object(forKey: item.key)
        }
    }

    private static func isSpecialJSONSetting(_ key: String) -> Bool {
        switch key {
        case "shortcuts.app",
             "shortcuts.quickTerminal",
             "shortcuts.customCommands",
             "ai.providers",
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
        case "shortcuts.quickTerminal":
            guard let shortcut: QuickTerminalShortcut = codableValue(from: value),
                  let canonicalShortcut = shortcut.canonicalizedForCurrentKeyboardLayout(),
                  let canonicalValue = codableJSONObject(canonicalShortcut)
            else {
                throw SettingsJSONError.invalidValue(key)
            }
            return canonicalValue
        case "shortcuts.customCommands":
            guard let configuration = commandShortcutConfiguration(from: value), isValidKeyCombo(configuration.prefixCombo),
                  configuration.shortcuts.allSatisfy({ isValidKeyCombo($0.combo) })
            else { throw SettingsJSONError.invalidValue(key) }
        case "ai.providers":
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

    private static func applySpecialSetting(
        key: String,
        value: Any,
        quickTerminalShortcutUpdater: QuickTerminalShortcutUpdater
    ) throws -> Bool {
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
        case "shortcuts.quickTerminal":
            guard let shortcut: QuickTerminalShortcut = codableValue(from: value) else { return true }
            try quickTerminalShortcutUpdater(shortcut)
        case "shortcuts.customCommands":
            guard let configuration = commandShortcutConfiguration(from: value) else { return true }
            CommandShortcutStore.shared.replaceConfiguration(configuration)
        case "ai.providers":
            guard let values = value as? [String: Any] else { return true }
            for provider in AIProviderRegistry.shared.providers {
                guard let enabled = values[provider.id] as? Bool else { continue }
                provider.isEnabled = enabled
            }
            Task { @MainActor in
                await AIProviderRegistry.shared.installAll()
            }
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
        case "editor.richInputImageStrategy":
            guard let rawValue = value as? String, let strategy = RichInputImageStrategy(rawValue: rawValue) else { return false }
            settings.richInputImageStrategy = strategy
        case "editor.richInputFontFamily":
            guard let value = value as? String else { return false }
            settings.richInputFontFamily = value
        case "editor.richInputLineHeightMultiplier":
            guard let value = doubleValue(value) else { return false }
            settings.richInputLineHeightMultiplier = CGFloat(value)
        default:
            return false
        }
        return true
    }

    private static func doubleValue(_ value: Any) -> Double? {
        if let value = value as? Double {
            return value
        }
        if let value = value as? Int {
            return Double(value)
        }
        if let value = value as? NSNumber {
            return value.doubleValue
        }
        return nil
    }

    private static func isBooleanValue(_ value: Any) -> Bool {
        CFGetTypeID(value as CFTypeRef) == CFBooleanGetTypeID()
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
        combo.isAssigned && combo.isCanonical
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
        if let array = value.base as? [String] {
            return array
        }
        if let value = value.base as? UInt16 {
            return Int(value)
        }
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
