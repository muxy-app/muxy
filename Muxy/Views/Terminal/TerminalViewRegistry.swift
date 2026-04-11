import AppKit

@MainActor
final class TerminalViewRegistry {
    static let shared = TerminalViewRegistry()

    private var views: [UUID: GhosttyTerminalNSView] = [:]

    private init() {}

    func view(for paneID: UUID, workingDirectory: String) -> GhosttyTerminalNSView {
        if let existing = views[paneID] {
            return existing
        }
        let view = GhosttyTerminalNSView(workingDirectory: workingDirectory)
        views[paneID] = view
        return view
    }

    func existingView(for paneID: UUID) -> GhosttyTerminalNSView? {
        views[paneID]
    }

    func removeView(for paneID: UUID) {
        guard let view = views.removeValue(forKey: paneID) else { return }
        view.tearDown()
    }

    func needsConfirmQuit(for paneID: UUID) -> Bool {
        views[paneID]?.needsConfirmQuit() ?? false
    }

    func view(for paneID: UUID) -> GhosttyTerminalNSView? {
        views[paneID]
    }
}

extension TerminalViewRegistry: TerminalViewRemoving {}
