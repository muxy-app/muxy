import Foundation

enum FileOpenerSelection {
    static let storageKey = "muxy.defaultFileOpener"
    static let builtinValue = ""

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

    static func value(extensionID: String, openerID: String) -> String {
        "\(extensionID):\(openerID)"
    }

    static func parse(_ storedValue: String) -> (extensionID: String, openerID: String)? {
        guard !storedValue.isEmpty else { return nil }
        let parts = storedValue.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false)
        guard parts.count == 2, !parts[0].isEmpty, !parts[1].isEmpty else { return nil }
        return (String(parts[0]), String(parts[1]))
    }
}
