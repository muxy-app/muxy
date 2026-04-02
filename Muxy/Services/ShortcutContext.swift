import AppKit

@MainActor
enum ShortcutContext {
    static let mainWindowIdentifier = NSUserInterfaceItemIdentifier("app.muxy.main-window")

    static func isMainWindow(_ window: NSWindow?) -> Bool {
        window?.identifier == mainWindowIdentifier
    }

    static func activeScopes(for window: NSWindow?) -> Set<ShortcutScope> {
        if isMainWindow(window) {
            return [.global, .mainWindow]
        }
        return [.global]
    }
}
