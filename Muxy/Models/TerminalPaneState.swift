import Foundation

@MainActor
@Observable
final class TerminalPaneState: Identifiable {
    let id: UUID
    let projectPath: String
    var title: String
    var currentWorkingDirectory: String?
    let startupCommand: String?
    let startupCommandInteractive: Bool
    let externalEditorFilePath: String?
    let restoredSession: TerminalSessionSnapshot?
    var activeRestoredCommand: String?
    var restoreDecision: TerminalSessionRestoreDecision = .none
    var restoreConsumed = false
    let searchState = TerminalSearchState()
    let branchObserver = PaneBranchObserver()
    @ObservationIgnored private var titleDebounceTask: Task<Void, Never>?

    init(
        id: UUID = UUID(),
        projectPath: String,
        title: String = "Terminal",
        initialWorkingDirectory: String? = nil,
        startupCommand: String? = nil,
        startupCommandInteractive: Bool = false,
        externalEditorFilePath: String? = nil,
        restoredSession: TerminalSessionSnapshot? = nil
    ) {
        self.id = id
        self.projectPath = projectPath
        self.title = title
        self.currentWorkingDirectory = initialWorkingDirectory
        self.startupCommand = startupCommand
        self.startupCommandInteractive = startupCommandInteractive
        self.externalEditorFilePath = externalEditorFilePath
        self.restoredSession = restoredSession
        branchObserver.update(repoPath: initialWorkingDirectory ?? projectPath)
        if let restoredSession {
            let decision = TerminalSessionRestorePolicy.decision(for: restoredSession)
            restoreDecision = decision
            if case let .command(command) = decision {
                activeRestoredCommand = command
            }
        }
    }

    func consumeRestoredLaunch() -> (command: String?, interactive: Bool) {
        guard !restoreConsumed else {
            return (startupCommand, startupCommandInteractive)
        }
        restoreConsumed = true
        switch restoreDecision {
        case .none:
            return (startupCommand, startupCommandInteractive)
        case let .command(command):
            return (command, true)
        }
    }

    func setTitle(_ newTitle: String) {
        titleDebounceTask?.cancel()
        titleDebounceTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled, let self, self.title != newTitle else { return }
            self.title = newTitle
        }
    }

    func setWorkingDirectory(_ path: String) {
        currentWorkingDirectory = path
        branchObserver.update(repoPath: path)
    }
}
