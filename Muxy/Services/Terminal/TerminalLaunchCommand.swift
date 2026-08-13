import Foundation

enum TerminalLaunchCommand {
    static let environmentKey = "MUXY_STARTUP_COMMAND"

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
        interactive: Bool,
        keepsShellOpen: Bool
    ) -> String {
        let command: String
        if let startupCommand, !startupCommand.isEmpty {
            let inner = remoteLoginShell(
                startupCommand: startupCommand,
                interactive: interactive,
                keepsShellOpen: keepsShellOpen
            )
            let remoteCommand = RemoteCommandBuilder.changeDirectoryPrefix(workingDirectory) + inner
            let bootstrap = RemoteCommandBuilder.environmentPrefix(destination.environment) + remoteCommand
            command = RemoteLoginShellCommand.bootstrap(bootstrap)
        } else {
            let inner = remoteLoginShell(
                startupCommand: nil,
                interactive: interactive,
                keepsShellOpen: keepsShellOpen
            )
            let remoteCommand = RemoteCommandBuilder.changeDirectoryPrefix(workingDirectory) + inner
            command = RemoteCommandBuilder.environmentPrefix(destination.environment) + remoteCommand
        }
        let options = SSHDestination.terminalOptions
        let arguments = destination.connectionArguments + options + ["-tt", destination.target, "--", command]
        return (["/usr/bin/ssh"] + arguments.map(ShellEscaper.escape)).joined(separator: " ")
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
