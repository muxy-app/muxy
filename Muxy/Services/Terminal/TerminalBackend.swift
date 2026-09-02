import Foundation

enum TerminalBackend: String, CaseIterable, Identifiable, Sendable {
    case ghostty

    static let fallback = TerminalBackend.ghostty

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .ghostty:
            "Ghostty"
        }
    }

    var capabilities: TerminalCapabilities {
        switch self {
        case .ghostty:
            .ghostty
        }
    }

    init(persisted value: String?) {
        guard let value, let backend = TerminalBackend(rawValue: value) else {
            self = .fallback
            return
        }
        self = backend
    }

    @MainActor
    func makeSurface(launch: TerminalLaunchRequest) -> any TerminalSurface {
        let surface: any TerminalSurface = switch self {
        case .ghostty:
            GhosttyTerminalNSView(
                workingDirectory: launch.workingDirectory,
                command: launch.command,
                commandInteractive: launch.commandInteractive,
                closesOnCommandExit: launch.closesOnCommandExit,
                workspaceContext: launch.workspaceContext,
                sessionBacking: launch.sessionBacking,
                createsRemoteTmuxSessionIfMissing: launch.createsRemoteTmuxSessionIfMissing
            )
        }
        precondition(surface.backend == self)
        precondition(surface.capabilities == capabilities)
        (surface as? any TerminalPaneAppearanceSurface)?.applyTerminalPaneConfiguration()
        return surface
    }

    @MainActor
    func makeQuickTerminalSurface(workingDirectory: String) -> any QuickTerminalSurface {
        switch self {
        case .ghostty:
            GhosttyTerminalNSView(workingDirectory: workingDirectory)
        }
    }
}

struct TerminalLaunchRequest {
    let workingDirectory: String
    let command: String?
    let commandInteractive: Bool
    let closesOnCommandExit: Bool
    let workspaceContext: WorkspaceContext
    let sessionBacking: TerminalSessionBacking
    let createsRemoteTmuxSessionIfMissing: Bool

    init(
        workingDirectory: String,
        command: String?,
        commandInteractive: Bool,
        closesOnCommandExit: Bool,
        workspaceContext: WorkspaceContext,
        sessionBacking: TerminalSessionBacking = .direct,
        createsRemoteTmuxSessionIfMissing: Bool = true
    ) {
        self.workingDirectory = workingDirectory
        self.command = command
        self.commandInteractive = commandInteractive
        self.closesOnCommandExit = closesOnCommandExit
        self.workspaceContext = workspaceContext
        self.sessionBacking = sessionBacking
        self.createsRemoteTmuxSessionIfMissing = createsRemoteTmuxSessionIfMissing
    }
}
