import AppKit
import SwiftUI

final class AppModalWindow: NSWindow {
    private var outsideClickMonitor: Any?

    func startOutsideClickMonitor() {
        stopOutsideClickMonitor()
        outsideClickMonitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] event in
            guard let self, let sheetParent, event.window === sheetParent else { return event }
            close()
            return nil
        }
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.modifierFlags.intersection(.deviceIndependentFlagsMask) == .command,
           event.charactersIgnoringModifiers?.lowercased() == "w"
        {
            close()
            return true
        }
        return super.performKeyEquivalent(with: event)
    }

    override func cancelOperation(_ sender: Any?) {
        close()
    }

    override func close() {
        stopOutsideClickMonitor()
        guard let sheetParent else {
            super.close()
            return
        }
        sheetParent.endSheet(self)
    }

    private func stopOutsideClickMonitor() {
        guard let outsideClickMonitor else { return }
        NSEvent.removeMonitor(outsideClickMonitor)
        self.outsideClickMonitor = nil
    }
}

@MainActor
struct AppModalConfig {
    let title: String
    let size: CGSize
    let existing: NSWindow?
    let delegate: NSWindowDelegate
    let onClosed: () -> Void
}

@MainActor
enum AppModalPresenter {
    static func present(
        _ config: AppModalConfig,
        @ViewBuilder content: () -> some View
    ) -> NSWindow? {
        if let existing = config.existing {
            NSApp.activate(ignoringOtherApps: true)
            let previousBehavior = existing.collectionBehavior
            existing.collectionBehavior.insert(.moveToActiveSpace)
            existing.makeKeyAndOrderFront(nil)
            existing.collectionBehavior = previousBehavior
            return existing
        }
        guard let parent = AppDelegate.activateMainWindowOnCurrentSpace()
            ?? NSApp.keyWindow ?? NSApp.mainWindow
        else { return nil }
        let host = NSHostingController(
            rootView: content()
                .frame(width: config.size.width, height: config.size.height)
                .preferredColorScheme(MuxyTheme.colorScheme)
                .environment(ExtensionStore.shared)
                .environment(ExtensionSettingsStore.shared)
        )
        let window = AppModalWindow(contentViewController: host)
        window.title = config.title
        window.styleMask = [.titled, .closable]
        window.isOpaque = true
        window.backgroundColor = MuxyTheme.nsBg
        window.delegate = config.delegate
        let onClosed = config.onClosed
        parent.beginSheet(window) { [weak window] _ in
            guard window != nil else { return }
            onClosed()
        }
        window.startOutsideClickMonitor()
        return window
    }
}
