import Foundation

@MainActor
enum TerminalSurfaceMaterializer {
    static func materialize(paneID: UUID, appState: AppState) -> (any TerminalSurface)? {
        if let view = TerminalViewRegistry.shared.existingView(for: paneID) {
            return view.ensureLiveSurfaceForExternalIO() ? view : nil
        }
        guard let location = appState.locatePane(paneID: paneID) else { return nil }
        guard let workspaceContext = appState.resolvedWorkspaceContext(for: location.worktreeKey) else { return nil }
        let pane = location.pane
        pane.resolveRemoteSessionMode(in: workspaceContext)
        let view = TerminalViewRegistry.shared.view(
            for: paneID,
            sessionID: pane.sessionID,
            workingDirectory: pane.currentWorkingDirectory ?? pane.projectPath,
            command: pane.startupCommand,
            commandInteractive: pane.startupCommandInteractive,
            closesOnCommandExit: pane.closesOnStartupCommandExit,
            workspaceContext: workspaceContext,
            remoteSessionMode: pane.remoteSessionMode,
            remoteTmuxDestination: pane.remoteTmuxDestination,
            createsRemoteTmuxSessionIfMissing: pane.createsRemoteTmuxSessionIfMissing
        )
        if view.envVars.isEmpty {
            view.envVars = TerminalEnvVarBuilder.build(paneID: paneID, worktreeKey: location.worktreeKey)
        }
        view.sessionMetadata = TerminalSessionMetadataBuilder.build(
            worktreeKey: location.worktreeKey,
            tabID: appState.locateTab(forPane: paneID)?.tab.id,
            title: pane.title
        )
        view.materializeHeadless()
        return view.hasLiveSurface ? view : nil
    }
}
