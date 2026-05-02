import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "LayoutCommandRunner")

@MainActor
enum LayoutCommandRunner {
    static func run(_ pending: [LayoutWorkspaceBuilder.PendingCommand]) {
        for entry in pending {
            Task { await send(paneID: entry.paneID, command: entry.command) }
        }
    }

    private static func send(paneID: UUID, command: String) async {
        for _ in 0 ..< 100 {
            try? await Task.sleep(nanoseconds: 100_000_000)
            if let view = TerminalViewRegistry.shared.view(for: paneID), view.hasLiveSurface {
                view.sendText(command)
                view.sendReturnKey()
                return
            }
        }
        logger.error("Timed out waiting for pane \(paneID.uuidString) to dispatch layout command")
    }
}
