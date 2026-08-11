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
            command = "exec /bin/sh -c \(ShellEscaper.escape(bootstrap))"
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
            "__muxy_shell=${SHELL:-/bin/sh}",
            "__muxy_shell_name=${__muxy_shell##*/}",
            remoteLoginShellCase(
                interactive: interactive,
                keepsShellOpen: keepsShellOpen
            ),
        ].joined(separator: "; ")
    }

    private static func remoteLoginShellCase(
        interactive: Bool,
        keepsShellOpen: Bool
    ) -> String {
        let posixFlags = interactive ? "-l -i" : "-l"
        let fishFlags = startupShellFlags(interactive: interactive)
        let posixScript = ShellEscaper.escape(posixScript(keepsShellOpen: keepsShellOpen))
        let fishScript = ShellEscaper.escape(fishScript(keepsShellOpen: keepsShellOpen))
        let fishBranch = "fish) exec \"$__muxy_shell\" \(fishFlags) -c \(fishScript) \"$__muxy_shell\""
        let posixBranch = "*) exec \"$__muxy_shell\" \(posixFlags) -c \(posixScript) \"$__muxy_shell\""
        return "case \"$__muxy_shell_name\" in \(fishBranch) ;; \(posixBranch) ;; esac"
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
