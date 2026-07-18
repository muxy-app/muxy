import Darwin
import Foundation

enum UserShell {
    static func path(
        environment: [String: String] = ProcessInfo.processInfo.environment,
        accountShell: () -> String? = accountShell
    ) -> String {
        if let shell = environment["SHELL"], !shell.isEmpty {
            return shell
        }
        return accountShell() ?? "/bin/zsh"
    }

    private static func accountShell() -> String? {
        guard let entry = getpwuid(getuid()), let shell = entry.pointee.pw_shell else { return nil }
        let path = String(cString: shell)
        return path.isEmpty ? nil : path
    }
}
