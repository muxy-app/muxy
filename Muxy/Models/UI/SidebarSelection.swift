import Foundation

enum SidebarSelection {
    static let storageKey = "muxy.activeSidebar"
    static let builtinValue = ""

    @MainActor
    static func resolvedExtensionID(
        from storedValue: String,
        store: ExtensionStore = .shared
    ) -> String? {
        guard !storedValue.isEmpty else { return nil }
        guard let status = store.statuses.first(where: { $0.id == storedValue }),
              status.isEnabled,
              status.muxyExtension.manifest.sidebar != nil
        else { return nil }
        return storedValue
    }

    @MainActor
    static func availableProviders(store: ExtensionStore = .shared) -> [ExtensionStore.ExtensionStatus] {
        store.statuses.filter { $0.isEnabled && $0.muxyExtension.manifest.sidebar != nil }
    }
}
