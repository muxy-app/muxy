import Foundation

struct TerminalPaneLaunch: Equatable {
    let command: String?
    let interactive: Bool
    let closesOnCommandExit: Bool
}

@MainActor
@Observable
final class TerminalPaneState: Identifiable {
    nonisolated static let defaultTitle = "Terminal"

    let id: UUID
    var sessionID: UUID
    let projectPath: String
    var title: String
    private(set) var usesDefaultTitle: Bool
    var currentWorkingDirectory: String?
    let startupCommand: String?
    let startupCommandInteractive: Bool
    let closesOnStartupCommandExit: Bool
    let externalEditorFilePath: String?
    private(set) var remoteSessionMode: SSHRemoteSessionMode?
    private(set) var remoteTmuxDestination: SSHDestination?
    let createsRemoteTmuxSessionIfMissing: Bool
    var isOffline = false
    var sessionRecoveryFailed = false
    let searchState = TerminalSearchState()
    @ObservationIgnored private var titleDebounceTask: Task<Void, Never>?

    init(
        id: UUID = UUID(),
        sessionID: UUID? = nil,
        projectPath: String,
        title: String? = nil,
        usesDefaultTitle: Bool? = nil,
        initialWorkingDirectory: String? = nil,
        startupCommand: String? = nil,
        startupCommandInteractive: Bool = false,
        closesOnStartupCommandExit: Bool = true,
        externalEditorFilePath: String? = nil,
        remoteSessionMode: SSHRemoteSessionMode? = nil,
        remoteTmuxDestination: SSHDestination? = nil,
        createsRemoteTmuxSessionIfMissing: Bool = true
    ) {
        self.id = id
        self.sessionID = sessionID ?? id
        self.projectPath = projectPath
        self.title = title ?? Self.defaultTitle
        self.usesDefaultTitle = usesDefaultTitle ?? (title == nil)
        self.currentWorkingDirectory = initialWorkingDirectory
        self.startupCommand = startupCommand
        self.startupCommandInteractive = startupCommandInteractive
        self.closesOnStartupCommandExit = closesOnStartupCommandExit
        self.externalEditorFilePath = externalEditorFilePath
        self.remoteSessionMode = remoteSessionMode
        self.remoteTmuxDestination = remoteTmuxDestination
        self.createsRemoteTmuxSessionIfMissing = createsRemoteTmuxSessionIfMissing
    }

    func consumeRestoredLaunch() -> TerminalPaneLaunch {
        TerminalPaneLaunch(
            command: startupCommand,
            interactive: startupCommandInteractive,
            closesOnCommandExit: closesOnStartupCommandExit
        )
    }

    func resolveRemoteSessionMode(in workspaceContext: WorkspaceContext) {
        guard let destination = workspaceContext.sshDestination else { return }
        if remoteSessionMode == nil {
            remoteSessionMode = destination.remoteSessionMode
        }
        if remoteSessionMode == .tmux, remoteTmuxDestination == nil {
            remoteTmuxDestination = destination
        }
    }

    var remoteTmuxSession: RemoteTmuxSession? {
        guard remoteSessionMode == .tmux, let remoteTmuxDestination else { return nil }
        return RemoteTmuxSession(id: sessionID, destination: remoteTmuxDestination)
    }

    func setTitle(_ newTitle: String) {
        titleDebounceTask?.cancel()
        titleDebounceTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled, let self else { return }
            guard self.title != newTitle || self.usesDefaultTitle else { return }
            self.title = newTitle
            self.usesDefaultTitle = false
            self.notifyTabUpdated()
        }
    }

    func setWorkingDirectory(_ path: String) {
        guard currentWorkingDirectory != path else { return }
        currentWorkingDirectory = path
        notifyTabUpdated()
    }

    private func notifyTabUpdated() {
        guard let appState = NotificationStore.shared.appState else { return }
        ExtensionEventEmitter.emitTabUpdated(forPane: id, appState: appState)
    }
}
