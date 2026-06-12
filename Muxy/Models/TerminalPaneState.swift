import Foundation
import MuxySSH

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
    var sshConfiguration: SSHConnectionConfiguration?
    var sshError: SSHConnectionError?
    var sshStartTime: Date?
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
        sshConfiguration: SSHConnectionConfiguration? = nil
    ) {
        self.id = id
        self.projectPath = projectPath
        self.title = title
        self.currentWorkingDirectory = initialWorkingDirectory
        self.startupCommand = startupCommand
        self.startupCommandInteractive = startupCommandInteractive
        self.closesOnStartupCommandExit = closesOnStartupCommandExit
        self.externalEditorFilePath = externalEditorFilePath
        self.sshConfiguration = sshConfiguration
    }

    func consumeRestoredLaunch() -> TerminalPaneLaunch {
        if sshConfiguration != nil {
            return TerminalPaneLaunch(command: nil, interactive: false, closesOnCommandExit: false)
        }
        return TerminalPaneLaunch(
            command: startupCommand,
            interactive: startupCommandInteractive,
            closesOnCommandExit: closesOnStartupCommandExit
        )
    }

    var migratedSSHStartupCommand: String? {
        guard let startupCommand else { return nil }
        let trimmed = startupCommand.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.hasPrefix("/usr/bin/ssh "), !trimmed.hasPrefix("'/usr/bin/ssh' "), !trimmed.hasPrefix("ssh ")
        else {
            return nil
        }
        return startupCommand
    }

    func setTitle(_ newTitle: String) {
        titleDebounceTask?.cancel()
        titleDebounceTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled, let self, self.title != newTitle else { return }
            self.title = newTitle
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
