import AppKit

@MainActor
final class TerminalViewRegistry {
    static let shared = TerminalViewRegistry()

    private var views: [UUID: GhosttyTerminalNSView] = [:]
    private var paneIDs: [ObjectIdentifier: UUID] = [:]

    private init() {}

    func isOwnedByRemote(_ paneID: UUID) -> Bool {
        !PaneOwnershipStore.shared.isOwnedByMac(paneID)
    }

    func view(
        for paneID: UUID,
        workingDirectory: String,
        command: String? = nil,
        commandInteractive: Bool = false
    ) -> GhosttyTerminalNSView {
        if let existing = views[paneID] {
            return existing
        }
        let view = GhosttyTerminalNSView(
            workingDirectory: workingDirectory,
            command: command,
            commandInteractive: commandInteractive
        )
        views[paneID] = view
        paneIDs[ObjectIdentifier(view)] = paneID
        return view
    }

    func existingView(for paneID: UUID) -> GhosttyTerminalNSView? {
        views[paneID]
    }

    func removeView(for paneID: UUID) {
        guard let view = views.removeValue(forKey: paneID) else { return }
        paneIDs.removeValue(forKey: ObjectIdentifier(view))
        view.tearDown()
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
}

extension TerminalViewRegistry: TerminalViewRemoving {}
