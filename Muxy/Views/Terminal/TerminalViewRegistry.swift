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
        views.removeValue(forKey: paneID)
    }
}

extension TerminalViewRegistry: TerminalViewRemoving {}
