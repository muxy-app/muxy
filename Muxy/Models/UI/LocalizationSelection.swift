import Foundation

enum LocalizationSelection {
    struct Option: Equatable, Identifiable {
        let id: String
        let title: String
        let isAvailable: Bool
    }

    static let storageKey = "muxy.localization"
    static let builtinValue = ""
    static let builtinTitle = "English"

    @MainActor
    static func resolvedBinding(
        from storedValue: String,
        store: ExtensionStore = .shared
    ) -> ExtensionStore.LocalizationBinding? {
        guard let identifier = parse(storedValue) else { return nil }
        return store.localization(
            extensionID: identifier.extensionID,
            localizationID: identifier.localizationID
        )
    }

    @MainActor
    static func availableProviders(store: ExtensionStore = .shared) -> [ExtensionStore.LocalizationBinding] {
        store.localizations()
    }

    static func options(
        from bindings: [ExtensionStore.LocalizationBinding],
        selectedValue: String
    ) -> [Option] {
        var options = [
            Option(id: builtinValue, title: builtinTitle, isAvailable: true),
        ]
        options += bindings.map {
            Option(
                id: $0.id,
                title: "\($0.localization.title) — \($0.muxyExtension.displayName)",
                isAvailable: true
            )
        }
        guard !selectedValue.isEmpty, !options.contains(where: { $0.id == selectedValue }) else {
            return options
        }
        options.append(Option(
            id: selectedValue,
            title: unavailableTitle(for: selectedValue),
            isAvailable: false
        ))
        return options
    }

    static func value(extensionID: String, localizationID: String) -> String {
        "\(extensionID):\(localizationID)"
    }

    static func parse(_ storedValue: String) -> (extensionID: String, localizationID: String)? {
        guard !storedValue.isEmpty else { return nil }
        let parts = storedValue.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false)
        guard parts.count == 2, !parts[0].isEmpty, !parts[1].isEmpty else { return nil }
        return (String(parts[0]), String(parts[1]))
    }

    private static func unavailableTitle(for value: String) -> String {
        guard let identifier = parse(value) else {
            return "Unavailable Language"
        }
        return "\(identifier.extensionID) (\(identifier.localizationID), unavailable)"
    }
}
