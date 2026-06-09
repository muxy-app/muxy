import Foundation

@MainActor
enum TerminalSurfaceMaterializer {
    static func materialize(paneID: UUID, appState: AppState) -> GhosttyTerminalNSView? {
        if let view = TerminalViewRegistry.shared.existingView(for: paneID) {
            return view.ensureLiveSurfaceForExternalIO() ? view : nil
        }
        guard let location = appState.locatePane(paneID: paneID) else { return nil }
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
        return view.surface != nil ? view : nil
    }
}
