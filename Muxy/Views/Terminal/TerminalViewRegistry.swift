import AppKit

@MainActor
final class TerminalViewRegistry {
    static let shared = TerminalViewRegistry()

    private var views: [UUID: GhosttyTerminalNSView] = [:]
    private var paneIDs: [ObjectIdentifier: UUID] = [:]
    private var focusedPaneID: UUID?

    private init() {}

    func view(for paneID: UUID, workingDirectory: String, command: String? = nil) -> GhosttyTerminalNSView {
        if let existing = views[paneID] {
            return existing
        }
        let view = GhosttyTerminalNSView(workingDirectory: workingDirectory, command: command)
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
        if focusedPaneID == paneID {
            focusedPaneID = nil
        }
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

    func setFocusedPane(_ paneID: UUID) {
        focusedPaneID = paneID
    }

    func focusedView(excluding view: GhosttyTerminalNSView) -> GhosttyTerminalNSView? {
        guard let focusedPaneID,
              let focusedView = views[focusedPaneID],
              focusedView !== view
        else { return nil }
        return focusedView
    }
}

extension TerminalViewRegistry: TerminalViewRemoving {}
