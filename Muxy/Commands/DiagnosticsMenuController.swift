import AppKit

@MainActor
final class DiagnosticsMenuController {
    static let shared = DiagnosticsMenuController()

    private var toggleItem: NSMenuItem?

    private init() {}

    func install() {
        DispatchQueue.main.async { [weak self] in
            self?.ensureInstalled()
        }
    }

    private func ensureInstalled() {
        guard let mainMenu = NSApp.mainMenu else { return }
        if mainMenu.indexOfItem(withTitle: "Diagnostics") >= 0 { return }

        let menu = NSMenu(title: "Diagnostics")
        menu.autoenablesItems = false

        let exportItem = NSMenuItem(
            title: "Export Diagnostics...",
            action: #selector(exportSnapshot),
            keyEquivalent: ""
        )
        exportItem.target = self
        menu.addItem(exportItem)

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
            title: "Reveal Logs Folder",
            action: #selector(revealLogs),
            keyEquivalent: ""
        )
        revealItem.target = self
        menu.addItem(revealItem)

        let topLevel = NSMenuItem(title: "Diagnostics", action: nil, keyEquivalent: "")
        topLevel.submenu = menu

        let insertIndex = mainMenu.indexOfItem(withTitle: "Window")
        if insertIndex >= 0 {
            mainMenu.insertItem(topLevel, at: insertIndex)
        } else {
            mainMenu.addItem(topLevel)
        }
    }

    private func toggleTitle() -> String {
        MemoryDiagnostics.shared.isPeriodicLoggingEnabled
            ? "Disable Periodic Logging"
            : "Enable Periodic Logging"
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
