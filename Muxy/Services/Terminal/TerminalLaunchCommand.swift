import Foundation

enum TerminalLaunchCommand {
    struct RemoteShellConfiguration {
        let interactive: Bool
        let keepsShellOpen: Bool
        let recoveryToken: UUID
    }

    static let environmentKey = "MUXY_STARTUP_COMMAND"
    static let remoteReconnectAttemptLimit = 5
    static let remoteReconnectResetInterval = 60
    static let remoteReconnectMinimumSessionDuration = 10
    private static let remoteReconnectRequiredTitlePrefix = "__MUXY_SSH_RECONNECT_REQUIRED__"

    static func shellCommand(
        interactive: Bool,
        keepsShellOpen: Bool = false,
        shell: String = UserShell.path()
    ) -> String {
        let flags = startupShellFlags(interactive: interactive)
        let escapedShell = ShellEscaper.escape(shell)
        return "\(escapedShell) \(flags) -c '\(script(shell: shell, keepsShellOpen: keepsShellOpen))' \(escapedShell)"
    }

    static func remoteShellCommand(
        destination: SSHDestination,
        workingDirectory: String,
        startupCommand: String?,
        configuration: RemoteShellConfiguration
    ) -> String {
        let initialCommand = remoteCommand(
            destination: destination,
            workingDirectory: workingDirectory,
            startupCommand: startupCommand,
            interactive: configuration.interactive,
            keepsShellOpen: configuration.keepsShellOpen
        )
        let reconnectCommand = remoteCommand(
            destination: destination,
            workingDirectory: workingDirectory,
            startupCommand: nil,
            interactive: configuration.interactive,
            keepsShellOpen: configuration.keepsShellOpen
        )
        let options = SSHDestination.terminalOptions
        let baseArguments = destination.connectionArguments + options + ["-tt", destination.target, "--"]
        let initialSSH = (["/usr/bin/ssh"] + baseArguments + [initialCommand])
            .map(ShellEscaper.escape)
            .joined(separator: " ")
        let reconnectSSH = (["/usr/bin/ssh"] + baseArguments + [reconnectCommand])
            .map(ShellEscaper.escape)
            .joined(separator: " ")
        let retryScript = remoteReconnectScript(
            initialSSH: initialSSH,
            reconnectSSH: reconnectSSH,
            recoveryToken: configuration.recoveryToken
        )
        return "/bin/sh -c \(ShellEscaper.escape(retryScript))"
    }

    static func remoteReconnectRequiredTitle(recoveryToken: UUID) -> String {
        remoteReconnectRequiredTitlePrefix + recoveryToken.uuidString
    }

    static func remoteReconnectScript(
        initialSSH: String,
        reconnectSSH: String,
        recoveryToken: UUID,
        attemptLimit: Int = remoteReconnectAttemptLimit,
        resetInterval: Int = remoteReconnectResetInterval,
        minimumSessionDuration: Int = remoteReconnectMinimumSessionDuration
    ) -> String {
        let retryMessage =
            "printf '\\r\\nMuxy: SSH connection lost. Reconnecting in %ss (%s/\(attemptLimit))...\\r\\n' "
                + "\"$muxy_delay\" \"$muxy_attempt\" >&2"
        let recoveryTitle = remoteReconnectRequiredTitle(recoveryToken: recoveryToken)
        let reconnectRequired = "printf '\\033]2;\(recoveryTitle)\\007'"
        return [
            "trap 'exit 130' INT",
            "trap 'exit 143' TERM",
            "muxy_attempt=0",
            "muxy_initial=1",
            "muxy_recovering=0",
            "while true; do",
            "muxy_started=$(date +%s)",
            "if [ \"$muxy_initial\" -eq 1 ]; then \(initialSSH); else \(reconnectSSH); fi",
            "muxy_status=$?",
            "muxy_now=$(date +%s)",
            "muxy_elapsed=$((muxy_now - muxy_started))",
            "[ \"$muxy_status\" -eq 255 ] || exit \"$muxy_status\"",
            "if [ \"$muxy_recovering\" -eq 0 ] && [ \"$muxy_elapsed\" -lt \(minimumSessionDuration) ]; then",
            "muxy_attempt=$((\(attemptLimit) + 1))",
            "else",
            "muxy_recovering=1",
            "[ \"$muxy_elapsed\" -lt \(resetInterval) ] || muxy_attempt=0",
            "muxy_attempt=$((muxy_attempt + 1))",
            "fi",
            "muxy_initial=0",
            "if [ \"$muxy_attempt\" -gt \(attemptLimit) ]; then",
            reconnectRequired,
            "printf '\\r\\nMuxy: SSH connection unavailable. Select Retry to try again.\\r\\n' >&2",
            "while :; do sleep 3600; done",
            "else",
            "case \"$muxy_attempt\" in 1) muxy_delay=1 ;; 2) muxy_delay=2 ;; 3) muxy_delay=4 ;; *) muxy_delay=8 ;; esac",
            retryMessage,
            "sleep \"$muxy_delay\"",
            "fi",
            "done",
        ].joined(separator: "\n")
    }

    private static func remoteCommand(
        destination: SSHDestination,
        workingDirectory: String,
        startupCommand: String?,
        interactive: Bool,
        keepsShellOpen: Bool
    ) -> String {
        let inner = remoteLoginShell(
            startupCommand: startupCommand,
            interactive: interactive,
            keepsShellOpen: keepsShellOpen
        )
        let remoteCommand = RemoteCommandBuilder.changeDirectoryPrefix(workingDirectory) + inner
        let command = RemoteCommandBuilder.environmentPrefix(destination.environment) + remoteCommand
        let executableCommand = if let startupCommand, !startupCommand.isEmpty {
            RemoteLoginShellCommand.bootstrap(command)
        } else {
            command
        }
        return remoteCommandReservingTransportFailureStatus(executableCommand)
    }

    static func remoteCommandReservingTransportFailureStatus(_ command: String) -> String {
        let script = [
            "/bin/sh -c \(ShellEscaper.escape(command))",
            "muxy_remote_status=$?",
            "[ \"$muxy_remote_status\" -eq 255 ] && exit 254",
            "exit \"$muxy_remote_status\"",
        ].joined(separator: "\n")
        return "/bin/sh -c \(ShellEscaper.escape(script))"
    }

    private static func remoteLoginShell(
        startupCommand: String?,
        interactive: Bool,
        keepsShellOpen: Bool
    ) -> String {
        let shell = "\"${SHELL:-/bin/sh}\""
        let flags = interactive ? "-l -i" : "-l"
        guard let startupCommand, !startupCommand.isEmpty else {
            return "exec \(shell) \(flags)"
        }
        let assignment = "\(environmentKey)=\(ShellEscaper.escape(startupCommand))"
        return [
            "export \(assignment)",
            RemoteLoginShellCommand.dispatch(
                posixScript: posixScript(keepsShellOpen: keepsShellOpen),
                fishScript: fishScript(keepsShellOpen: keepsShellOpen),
                interactive: interactive
            ),
        ].joined(separator: "; ")
    }

    private static func script(shell: String, keepsShellOpen: Bool) -> String {
        if isFishShell(shell) {
            return fishScript(keepsShellOpen: keepsShellOpen)
        }
        return posixScript(keepsShellOpen: keepsShellOpen)
    }

    private static func startupShellFlags(interactive: Bool) -> String {
        interactive ? "-l -i" : "-l"
    }

    private static func isFishShell(_ shell: String) -> Bool {
        (shell as NSString).lastPathComponent == "fish"
    }

    private static func posixScript(keepsShellOpen: Bool) -> String {
        var segments = [
            "eval \"$\(environmentKey)\"",
            "muxy_status=$?",
            "if [ $muxy_status -ne 0 ]",
            "then exec \"$0\" -l",
        ]
        segments.append(keepsShellOpen ? "else exec \"$0\" -l" : "else exit $muxy_status")
        segments.append("fi")
        return segments.joined(separator: "; ")
    }

    private static func fishScript(keepsShellOpen: Bool) -> String {
        var segments = [
            "eval \"$\(environmentKey)\"",
            "set muxy_status $status",
            "if test $muxy_status -ne 0",
            "exec \"$argv[1]\" -l",
        ]
        segments.append(keepsShellOpen ? "else exec \"$argv[1]\" -l" : "else exit $muxy_status")
        segments.append("end")
        return segments.joined(separator: "; ")
    }
}
