import Darwin
import Foundation

enum TerminalLaunchCommand {
    static let environmentKey = "MUXY_STARTUP_COMMAND"

    static func shellCommand(
        interactive: Bool,
        keepsShellOpen: Bool = false,
        shell: String = userShell()
    ) -> String {
        let flags = interactive ? "-l -i" : "-l"
        let escapedShell = ShellEscaper.escape(shell)
        return "\(escapedShell) \(flags) -c '\(script(keepsShellOpen: keepsShellOpen))' \(escapedShell)"
    }

    static func remoteShellCommand(
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
        let options = SSHDestination.terminalOptions
        let arguments = destination.connectionArguments + options + ["-tt", destination.target, "--", command]
        return (["/usr/bin/ssh"] + arguments.map(ShellEscaper.escape)).joined(separator: " ")
    }

    private static func remoteLoginShell(
        startupCommand: String?,
        interactive: Bool,
        keepsShellOpen: Bool
    ) -> String {
        let flags = interactive ? "-l -i" : "-l"
        guard let startupCommand, !startupCommand.isEmpty else {
            return "exec \"${SHELL:-/bin/sh}\" \(flags)"
        }
        let scriptText = ShellEscaper.escape(script(keepsShellOpen: keepsShellOpen))
        let assignment = "\(environmentKey)=\(ShellEscaper.escape(startupCommand))"
        return "export \(assignment); exec \"${SHELL:-/bin/sh}\" \(flags) -c \(scriptText) \"${SHELL:-/bin/sh}\""
    }

    private static func script(keepsShellOpen: Bool) -> String {
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

    private static func userShell() -> String {
        if let shell = ProcessInfo.processInfo.environment["SHELL"], !shell.isEmpty {
            return shell
        }
        guard let pw = getpwuid(getuid()), let shellPtr = pw.pointee.pw_shell else {
            return "/bin/zsh"
        }
        return String(cString: shellPtr)
    }
}
