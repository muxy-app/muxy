import Foundation

enum FileOpenerSelection {
    struct Option: Equatable, Identifiable {
        let id: String
        let title: String
        let isAvailable: Bool
    }

    static let storageKey = "muxy.defaultFileOpener"
    static let builtinValue = ""
    static let builtinTitle = "Built-in (Top Bar Project Target)"

    @MainActor
    static func resolvedBinding(
        from storedValue: String,
        relativePath: String? = nil,
        store: ExtensionStore = .shared
    ) -> ExtensionStore.FileOpenerBinding? {
        guard let identifier = parse(storedValue) else { return nil }
        return store.fileOpener(
            extensionID: identifier.extensionID,
            openerID: identifier.openerID,
            relativePath: relativePath
        )
    }

    @MainActor
    static func availableOpeners(store: ExtensionStore = .shared) -> [ExtensionStore.FileOpenerBinding] {
        store.fileOpeners()
    }

    static func options(
        from bindings: [ExtensionStore.FileOpenerBinding],
        selectedValue: String
    ) -> [Option] {
        var options = [
            Option(id: builtinValue, title: builtinTitle, isAvailable: true),
        ]
        options += bindings.map {
            Option(id: $0.id, title: title(for: $0), isAvailable: true)
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

    static func value(extensionID: String, openerID: String) -> String {
        "\(extensionID):\(openerID)"
    }

    static func parse(_ storedValue: String) -> (extensionID: String, openerID: String)? {
        guard !storedValue.isEmpty else { return nil }
        let parts = storedValue.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false)
        guard parts.count == 2, !parts[0].isEmpty, !parts[1].isEmpty else { return nil }
        return (String(parts[0]), String(parts[1]))
    }

    private static func title(for binding: ExtensionStore.FileOpenerBinding) -> String {
        guard let title = binding.opener.title, !title.isEmpty else {
            return binding.muxyExtension.displayName
        }
        return "\(binding.muxyExtension.displayName) (\(title))"
    }

    private static func unavailableTitle(for value: String) -> String {
        guard let identifier = parse(value) else {
            return "Unavailable Extension Opener"
        }
        return "\(identifier.extensionID) (\(identifier.openerID), unavailable)"
    }
}
