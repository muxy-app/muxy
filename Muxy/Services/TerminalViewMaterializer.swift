import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "TerminalViewMaterializer")

@MainActor
enum TerminalViewMaterializer {
    static func ensureMaterialized(paneID: UUID, appState: AppState) -> GhosttyTerminalNSView? {
        if let existing = TerminalViewRegistry.shared.existingView(for: paneID) {
            return existing
        }
        guard let location = appState.locatePane(paneID: paneID) else {
            return nil
        }
        let pane = location.pane
        let view = TerminalViewRegistry.shared.view(
            for: paneID,
            workingDirectory: pane.currentWorkingDirectory ?? pane.projectPath,
            command: pane.startupCommand,
            commandInteractive: pane.startupCommandInteractive,
            closesOnCommandExit: pane.closesOnStartupCommandExit
        )
        if view.envVars.isEmpty {
            view.envVars = TerminalEnvVarBuilder.build(paneID: paneID, worktreeKey: location.worktreeKey)
        }
        view.materializeHeadless()
        if view.surface == nil {
            logger.warning("Headless materialization left pane \(paneID) without a surface")
        }
        return view
    }
}
