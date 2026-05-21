import Darwin
import Foundation

enum TerminalLaunchCommand {
    static let environmentKey = "MUXY_STARTUP_COMMAND"

    static func shellCommand(interactive: Bool, shell: String = userShell()) -> String {
        let flags = interactive ? "-l -i" : "-l"
        let escapedShell = ShellEscaper.escape(shell)
        return "\(escapedShell) \(flags) -c '\(script)' \(escapedShell)"
    }

    private static var script: String {
        [
            "eval \"$\(environmentKey)\"",
            "muxy_status=$?",
            "if [ $muxy_status -ne 0 ]",
            "then exec \"$0\" -l",
            "else exit $muxy_status",
            "fi",
        ].joined(separator: "; ")
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
