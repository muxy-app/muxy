import Foundation
import MuxySessionProtocol

enum TerminalLaunchCommand {
    static let environmentKey = "MUXY_STARTUP_COMMAND"

    static func shellCommand(
        interactive: Bool,
        keepsShellOpen: Bool = false,
        shell: String = UserShell.path()
    ) -> String {
        let flags = startupShellFlags(interactive: interactive, shell: shell)
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
        let remoteCommand = remoteShellBootstrapCommand(
            environment: destination.environment,
            workingDirectory: workingDirectory,
            startupCommand: startupCommand,
            interactive: interactive,
            keepsShellOpen: keepsShellOpen
        )
        let options = SSHDestination.terminalOptions
        let arguments = destination.connectionArguments + options + ["-tt", destination.target, "--", remoteCommand]
        return (["/usr/bin/ssh"] + arguments.map(ShellEscaper.escape)).joined(separator: " ")
    }

    static func remoteShellBootstrapCommand(
        environment: [String: String],
        workingDirectory: String,
        startupCommand: String?,
        interactive: Bool,
        keepsShellOpen: Bool
    ) -> String {
        let script = remoteShellBootstrapScript(
            environment: environment,
            workingDirectory: workingDirectory,
            startupCommand: startupCommand,
            interactive: interactive,
            keepsShellOpen: keepsShellOpen
        )
        return "exec /bin/sh -c \(ShellEscaper.escape(script))"
    }

    static func remoteShellBootstrapScript(
        environment: [String: String],
        workingDirectory: String,
        startupCommand: String?,
        interactive: Bool,
        keepsShellOpen: Bool
    ) -> String {
        RemoteCommandBuilder.environmentPrefix(environment)
            + RemoteCommandBuilder.changeDirectoryPrefix(workingDirectory)
            + remoteLoginShell(
                startupCommand: startupCommand,
                interactive: interactive,
                keepsShellOpen: keepsShellOpen
            )
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
        let fishFlags = startupShellFlags(interactive: interactive, shell: "fish")
        let posixScriptText = ShellEscaper.escape(posixScript(keepsShellOpen: keepsShellOpen))
        let fishScriptText = ShellEscaper.escape(fishScript(keepsShellOpen: keepsShellOpen))
        let fishBranch = "fish) exec \"$__muxy_shell\" \(fishFlags) -c \(fishScriptText) \"$__muxy_shell\""
        let posixBranch = "*) exec \"$__muxy_shell\" \(posixFlags) -c \(posixScriptText) \"$__muxy_shell\""
        return "case \"$__muxy_shell_name\" in \(fishBranch) ;; \(posixBranch) ;; esac"
    }

    private static func script(shell: String, keepsShellOpen: Bool) -> String {
        if SessionShellIntegration.shellName(shell) == "fish" {
            return fishScript(keepsShellOpen: keepsShellOpen)
        }
        return posixScript(keepsShellOpen: keepsShellOpen)
    }

    private static func startupShellFlags(interactive: Bool, shell: String) -> String {
        if interactive, SessionShellIntegration.shellName(shell) == "fish" {
            return "-l"
        }
        return interactive ? "-l -i" : "-l"
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
