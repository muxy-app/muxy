import AppKit

@MainActor
final class PanelFocusRestoration {
    static let shared = PanelFocusRestoration()

    private final class Snapshot {
        weak var window: NSWindow?
        weak var responder: NSResponder?
        weak var panelView: NSView?

        init(window: NSWindow, responder: NSResponder, panelView: NSView) {
            self.window = window
            self.responder = responder
            self.panelView = panelView
        }
    }

    private var snapshots: [String: Snapshot] = [:]

    func captureBeforeClaim(panelID: String, panelView: NSView) {
        if let snapshot = snapshots[panelID] {
            snapshot.panelView = panelView
            return
        }
        guard let window = panelView.window,
              let responder = window.firstResponder,
              !Self.contains(responder, in: panelView)
        else { return }
        snapshots[panelID] = Snapshot(
            window: window,
            responder: responder,
            panelView: panelView
        )
    }

    func restoreAfterClosing(panelID: String) {
        guard let snapshot = snapshots.removeValue(forKey: panelID) else { return }
        rebaseSnapshots(dependingOn: snapshot)
        guard let window = snapshot.window,
              let responder = snapshot.responder,
              let panelView = snapshot.panelView,
              Self.contains(window.firstResponder, in: panelView)
        else { return }
        window.makeFirstResponder(responder)
    }

    private func rebaseSnapshots(dependingOn closedSnapshot: Snapshot) {
        guard let panelView = closedSnapshot.panelView,
              let responder = closedSnapshot.responder
        else { return }
        for snapshot in snapshots.values where Self.contains(snapshot.responder, in: panelView) {
            snapshot.responder = responder
        }
    }

    private static func contains(_ responder: NSResponder?, in view: NSView) -> Bool {
        guard let responder else { return false }
        if responder === view {
            return true
        }
        guard let responderView = responder as? NSView else { return false }
        return responderView.isDescendant(of: view)
    }
}
