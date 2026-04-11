import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "WorktreeSetupRunner")

@MainActor
enum WorktreeSetupRunner {
    static func run(sourceProjectPath: String, paneID: UUID) async {
        guard let config = WorktreeConfig.load(fromProjectPath: sourceProjectPath),
              !config.setup.isEmpty
        else { return }

        let commands = config.setup
            .map(\.command)
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
            .joined(separator: " && ")
        guard !commands.isEmpty else { return }

        for _ in 0 ..< 50 {
            try? await Task.sleep(nanoseconds: 100_000_000)
            if let view = TerminalViewRegistry.shared.view(for: paneID), view.hasLiveSurface {
                view.sendText(commands)
                view.sendReturnKey()
                return
            }
        }
        logger.error("Timed out waiting for pane \(paneID.uuidString) before sending setup commands")
    }
}
