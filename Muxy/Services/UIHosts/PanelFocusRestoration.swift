import AppKit

@MainActor
final class PanelFocusRestoration {
    static let shared = PanelFocusRestoration()

    private final class Snapshot {
        weak var window: NSWindow?
        weak var responder: NSResponder?
        weak var panelView: NSView?
        weak var panelControlResponder: NSResponder?
        var precedingPanelID: String?

        init(
            window: NSWindow,
            responder: NSResponder,
            panelView: NSView,
            precedingPanelID: String?
        ) {
            self.window = window
            self.responder = responder
            self.panelView = panelView
            self.precedingPanelID = precedingPanelID
        }
    }

    private var snapshots: [String: Snapshot] = [:]

    func captureBeforeClaim(panelID: String, panelView: NSView) {
        if let snapshot = snapshots[panelID] {
            snapshot.panelView = panelView
            snapshot.panelControlResponder = nil
            retargetSnapshots(dependingOn: panelID, responder: panelView)
            return
        }
        guard let window = panelView.window,
              let responder = window.firstResponder,
              !Self.contains(responder, in: panelView)
        else { return }
        let precedingPanelID = snapshots.first { _, snapshot in
            Self.owns(responder, snapshot: snapshot)
        }?.key
        snapshots[panelID] = Snapshot(
            window: window,
            responder: responder,
            panelView: panelView,
            precedingPanelID: precedingPanelID
        )
    }

    func setPanelControlFocused(panelID: String, focused: Bool) {
        guard let snapshot = snapshots[panelID] else { return }
        guard focused, let window = snapshot.window else {
            snapshot.panelControlResponder = nil
            return
        }
        snapshot.panelControlResponder = window.firstResponder
    }

    func restoreAfterClosing(panelID: String) {
        guard let snapshot = snapshots.removeValue(forKey: panelID) else { return }
        rebaseSnapshots(dependingOn: panelID, closedSnapshot: snapshot)
        guard let window = snapshot.window,
              let responder = snapshot.responder,
              Self.owns(window.firstResponder, snapshot: snapshot)
        else { return }
        window.makeFirstResponder(responder)
    }

    private func rebaseSnapshots(
        dependingOn closedPanelID: String,
        closedSnapshot: Snapshot
    ) {
        guard let responder = closedSnapshot.responder else { return }
        for snapshot in snapshots.values where snapshot.precedingPanelID == closedPanelID {
            snapshot.responder = responder
            snapshot.precedingPanelID = closedSnapshot.precedingPanelID
        }
    }

    private func retargetSnapshots(dependingOn panelID: String, responder: NSResponder) {
        for snapshot in snapshots.values where snapshot.precedingPanelID == panelID {
            snapshot.responder = responder
        }
    }

    private static func owns(_ responder: NSResponder?, snapshot: Snapshot) -> Bool {
        guard let responder else { return false }
        if responder === snapshot.panelControlResponder {
            return true
        }
        guard let panelView = snapshot.panelView else { return false }
        return contains(responder, in: panelView)
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
