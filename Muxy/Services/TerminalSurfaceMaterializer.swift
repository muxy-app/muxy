import Foundation

@MainActor
@Observable
final class TerminalMaterializationStores {
    static let shared = TerminalMaterializationStores()
    weak var projectGroupStore: ProjectGroupStore?

    private init() {}
}

@MainActor
enum TerminalSurfaceMaterializer {
    static func materialize(paneID: UUID, appState: AppState) -> GhosttyTerminalNSView? {
        if let view = TerminalViewRegistry.shared.existingView(for: paneID) {
            return view.ensureLiveSurfaceForExternalIO() ? view : nil
        }
        guard let location = appState.locatePane(paneID: paneID) else { return nil }
        let pane = location.pane
        let workspaceContext = workspaceContext(for: location.worktreeKey)
        let sshConfiguration = sshConfiguration(for: pane, workspaceContext: workspaceContext)
        pane.sshConfiguration = sshConfiguration
        let view = TerminalViewRegistry.shared.view(
            for: paneID,
            workingDirectory: pane.currentWorkingDirectory ?? pane.projectPath,
            command: pane.startupCommand,
            commandInteractive: pane.startupCommandInteractive,
            closesOnCommandExit: pane.closesOnStartupCommandExit,
            sshConfiguration: sshConfiguration,
            workspaceContext: workspaceContext
        )
        if view.envVars.isEmpty {
            view.envVars = TerminalEnvVarBuilder.build(paneID: paneID, worktreeKey: location.worktreeKey)
        }
        view.materializeHeadless()
        return view.surface != nil ? view : nil
    }

    private static func workspaceContext(for worktreeKey: WorktreeKey) -> WorkspaceContext {
        guard let projectGroupStore = TerminalMaterializationStores.shared.projectGroupStore else {
            return .local
        }
        if let project = projectGroupStore.remoteProjects.first(where: { $0.id == worktreeKey.projectID }) {
            return projectGroupStore.workspaceContext(for: project)
        }
        return .local
    }

    private static func sshConfiguration(
        for pane: TerminalPaneState,
        workspaceContext: WorkspaceContext
    ) -> SSHConnectionConfiguration? {
        guard case let .ssh(destination) = workspaceContext else { return nil }
        guard SSHImplementationMode.current == .native else { return nil }
        return SSHConnectionConfiguration.make(
            destination: destination,
            remotePath: pane.currentWorkingDirectory ?? pane.projectPath,
            command: pane.migratedSSHStartupCommand
        )
    }
}
