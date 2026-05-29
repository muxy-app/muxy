import AppKit

@MainActor
final class TerminalViewRegistry {
    static let shared = TerminalViewRegistry()

    private var views: [UUID: GhosttyTerminalNSView] = [:]
    private var paneIDs: [ObjectIdentifier: UUID] = [:]
    private var evictionWorkItems: [UUID: DispatchWorkItem] = [:]

    private init() {}

    func isOwnedByRemote(_ paneID: UUID) -> Bool {
        !PaneOwnershipStore.shared.isOwnedByMac(paneID)
    }

    func view(
        for paneID: UUID,
        workingDirectory: String,
        command: String? = nil,
        commandInteractive: Bool = false,
        closesOnCommandExit: Bool = true
    ) -> GhosttyTerminalNSView {
        if let existing = views[paneID] {
            return existing
        }
        let view = GhosttyTerminalNSView(
            workingDirectory: workingDirectory,
            command: command,
            commandInteractive: commandInteractive,
            closesOnCommandExit: closesOnCommandExit
        )
        views[paneID] = view
        paneIDs[ObjectIdentifier(view)] = paneID
        return view
    }

    func existingView(for paneID: UUID) -> GhosttyTerminalNSView? {
        views[paneID]
    }

    func removeView(for paneID: UUID) {
        evictionWorkItems[paneID]?.cancel()
        evictionWorkItems.removeValue(forKey: paneID)
        guard let view = views.removeValue(forKey: paneID) else { return }
        paneIDs.removeValue(forKey: ObjectIdentifier(view))
        TerminalCommandTracker.shared.removePane(paneID)
        view.tearDown()
    }

    func evictAllExceptWorkspace(_ activeKey: WorktreeKey) {
        guard GhosttyTerminalNSView.surfaceEvictionEnabled() else { return }
        for (paneID, view) in views {
            if view.worktreeKey == activeKey {
                evictionWorkItems[paneID]?.cancel()
                evictionWorkItems.removeValue(forKey: paneID)
            } else if view.surface != nil {
                evictionWorkItems[paneID]?.cancel()
                let workItem = DispatchWorkItem { [weak view] in
                    guard let view, view.surface != nil else { return }
                    view.destroySurface()
                }
                evictionWorkItems[paneID] = workItem
                DispatchQueue.main.asyncAfter(deadline: .now() + 3, execute: workItem)
            }
        }
    }

    func needsConfirmQuit(for paneID: UUID) -> Bool {
        views[paneID]?.needsConfirmQuit() ?? false
    }

    func view(for paneID: UUID) -> GhosttyTerminalNSView? {
        views[paneID]
    }

    func paneID(for view: GhosttyTerminalNSView) -> UUID? {
        paneIDs[ObjectIdentifier(view)]
    }

    func applyColorSchemeToAllViews(isDark: Bool) {
        for view in views.values {
            view.applyColorScheme(isDark: isDark)
        }
    }

    var liveViewCount: Int {
        views.count
    }

    var liveSurfaceCount: Int {
        views.values.reduce(0) { $1.surface != nil ? $0 + 1 : $0 }
    }
}

extension TerminalViewRegistry: TerminalViewRemoving {}
