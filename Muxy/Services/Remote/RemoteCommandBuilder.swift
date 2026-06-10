import Foundation

enum RemoteCommandBuilder {
    static func quoteRemotePath(_ path: String) -> String {
        guard path == "~" || path.hasPrefix("~/") else {
            return ShellEscaper.escape(path)
        }
        guard path != "~" else { return "~" }
        let remainder = String(path.dropFirst(2))
        return "~/" + ShellEscaper.escape(remainder)
    }

    static func changeDirectoryPrefix(_ workingDirectory: String?) -> String {
        guard let workingDirectory, !workingDirectory.isEmpty else { return "" }
        return "cd \(quoteRemotePath(workingDirectory)) && "
    }

    static func isValidEnvironmentKey(_ key: String) -> Bool {
        guard let first = key.first, first.isASCII, first.isLetter || first == "_" else { return false }
        return key.dropFirst().allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "_") }
    }

    static func environmentPrefix(_ environment: [String: String]?) -> String {
        guard let environment, !environment.isEmpty else { return "" }
        let assignments = environment
            .filter { isValidEnvironmentKey($0.key) }
            .sorted { $0.key < $1.key }
            .map { "export \($0.key)=\(ShellEscaper.escape($0.value))" }
        guard !assignments.isEmpty else { return "" }
        return assignments.joined(separator: "; ") + "; "
    }

    static func remoteCommand(
        executable: String,
        arguments: [String],
        workingDirectory: String?,
        environment: [String: String]? = nil
    ) -> String {
        let command = ([executable] + arguments)
            .map(ShellEscaper.escape)
            .joined(separator: " ")
        return environmentPrefix(environment)
            + changeDirectoryPrefix(workingDirectory)
            + command
    }

    static func remoteShellCommand(
        shell: String,
        workingDirectory: String?,
        environment: [String: String]? = nil
    ) -> String {
        environmentPrefix(environment)
            + changeDirectoryPrefix(workingDirectory)
            + "( \(shell) )"
    }
}
