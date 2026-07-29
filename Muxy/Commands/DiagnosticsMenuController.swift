import AppKit

@MainActor
final class DiagnosticsMenuController {
    static let shared = DiagnosticsMenuController()

    private static let appKitWindowMenuTitle = "Window"

    private var topLevelItem: NSMenuItem?
    private var exportItem: NSMenuItem?
    private var toggleItem: NSMenuItem?
    private var revealItem: NSMenuItem?

    private init() {}

    func install() {
        DispatchQueue.main.async { [weak self] in
            self?.ensureInstalled()
        }
    }

    private func ensureInstalled() {
        guard let mainMenu = NSApp.mainMenu else { return }
        if let topLevelItem, mainMenu.index(of: topLevelItem) >= 0 {
            return
        }

        let menu = NSMenu(title: L10n.string("Diagnostics"))
        menu.autoenablesItems = false

        let exportItem = NSMenuItem(
            title: L10n.string("Export Diagnostics..."),
            action: #selector(exportSnapshot),
            keyEquivalent: ""
        )
        exportItem.target = self
        menu.addItem(exportItem)
        self.exportItem = exportItem

        let toggle = NSMenuItem(
            title: toggleTitle(),
            action: #selector(togglePeriodicLogging),
            keyEquivalent: ""
        )
        toggle.target = self
        menu.addItem(toggle)
        toggleItem = toggle

        menu.addItem(.separator())

        let revealItem = NSMenuItem(
            title: L10n.string("Reveal Logs Folder"),
            action: #selector(revealLogs),
            keyEquivalent: ""
        )
        revealItem.target = self
        menu.addItem(revealItem)
        self.revealItem = revealItem

        let topLevel = NSMenuItem(title: L10n.string("Diagnostics"), action: nil, keyEquivalent: "")
        topLevel.submenu = menu
        topLevelItem = topLevel

        let insertIndex = mainMenu.indexOfItem(withTitle: Self.appKitWindowMenuTitle)
        if insertIndex >= 0 {
            mainMenu.insertItem(topLevel, at: insertIndex)
        } else {
            mainMenu.addItem(topLevel)
        }
    }

    private func toggleTitle() -> String {
        MemoryDiagnostics.shared.isPeriodicLoggingEnabled
            ? L10n.string("Disable Periodic Logging")
            : L10n.string("Enable Periodic Logging")
    }

    func refreshLocalization() {
        topLevelItem?.title = L10n.string("Diagnostics")
        topLevelItem?.submenu?.title = L10n.string("Diagnostics")
        exportItem?.title = L10n.string("Export Diagnostics...")
        toggleItem?.title = toggleTitle()
        revealItem?.title = L10n.string("Reveal Logs Folder")
    }

    @objc
    private func exportSnapshot() {
        if let url = MemoryDiagnostics.shared.exportSnapshot() {
            NSWorkspace.shared.activateFileViewerSelecting([url])
        }
    }

    @objc
    private func togglePeriodicLogging() {
        let current = MemoryDiagnostics.shared.isPeriodicLoggingEnabled
        MemoryDiagnostics.shared.setPeriodicLoggingEnabled(!current)
        toggleItem?.title = toggleTitle()
    }

    @objc
    private func revealLogs() {
        guard let library = FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask).first else {
            return
        }
        let dir = library.appendingPathComponent("Logs/Muxy", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        NSWorkspace.shared.activateFileViewerSelecting([dir])
    }
}
