import Foundation

enum SSHConnectionError {
    case refused(String)
    case authFailed(String)
    case hostKeyChanged(String)
    case timeout(String)
    case unknown(String)

    var title: String {
        switch self {
        case .refused: "Connection Refused"
        case .authFailed: "Authentication Failed"
        case .hostKeyChanged: "Host Key Changed"
        case .timeout: "Connection Timeout"
        case .unknown: "Connection Error"
        }
    }

    var message: String {
        switch self {
        case let .refused(host): "Could not connect to \(host): Connection refused"
        case let .authFailed(detail): detail
        case let .hostKeyChanged(detail): detail
        case let .timeout(host): "Connection to \(host) timed out"
        case let .unknown(detail): detail
        }
    }
}

struct TerminalPaneLaunch: Equatable {
    let command: String?
    let interactive: Bool
    let closesOnCommandExit: Bool
}

@MainActor
@Observable
final class TerminalPaneState: Identifiable {
    let id: UUID
    let projectPath: String
    var title: String
    var currentWorkingDirectory: String?
    let startupCommand: String?
    let startupCommandInteractive: Bool
    let closesOnStartupCommandExit: Bool
    let externalEditorFilePath: String?
    let restoredSession: TerminalSessionSnapshot?
    var envVars: [(key: String, value: String)] = []
    var remoteHostID: UUID?
    var sshError: SSHConnectionError?
    var sshStartTime: Date?
    var activeRestoredCommand: String?
    var restoreDecision: TerminalSessionRestoreDecision = .none
    var restoreConsumed = false
    var isOffline = false
    let searchState = TerminalSearchState()
    @ObservationIgnored private var titleDebounceTask: Task<Void, Never>?

    init(
        id: UUID = UUID(),
        projectPath: String,
        title: String = "Terminal",
        initialWorkingDirectory: String? = nil,
        startupCommand: String? = nil,
        startupCommandInteractive: Bool = false,
        closesOnStartupCommandExit: Bool = true,
        externalEditorFilePath: String? = nil,
        restoredSession: TerminalSessionSnapshot? = nil
    ) {
        self.id = id
        self.projectPath = projectPath
        self.title = title
        self.currentWorkingDirectory = initialWorkingDirectory
        self.startupCommand = startupCommand
        self.startupCommandInteractive = startupCommandInteractive
        self.closesOnStartupCommandExit = closesOnStartupCommandExit
        self.externalEditorFilePath = externalEditorFilePath
        self.restoredSession = restoredSession
        if let restoredSession {
            let decision = TerminalSessionRestorePolicy.decision(for: restoredSession)
            restoreDecision = decision
            if case let .command(command) = decision {
                activeRestoredCommand = command
            }
        }
    }

    func consumeRestoredLaunch() -> TerminalPaneLaunch {
        guard !restoreConsumed else {
            return TerminalPaneLaunch(
                command: startupCommand,
                interactive: startupCommandInteractive,
                closesOnCommandExit: closesOnStartupCommandExit
            )
        }
        restoreConsumed = true
        switch restoreDecision {
        case .none:
            return TerminalPaneLaunch(
                command: startupCommand,
                interactive: startupCommandInteractive,
                closesOnCommandExit: closesOnStartupCommandExit
            )
        case let .command(command):
            return TerminalPaneLaunch(command: command, interactive: true, closesOnCommandExit: true)
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
    }
}
