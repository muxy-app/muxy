import Foundation

enum TerminalProcessDisplayName {
    private static let shellNames: Set<String> = ["bash", "fish", "sh", "zsh"]

    static func resolve(foregroundName: String?, activeCommand: String?) -> String? {
        guard let foregroundName else {
            return commandExecutableName(activeCommand)
        }
        guard shellNames.contains(foregroundName.lowercased()) else {
            return foregroundName
        }
        return commandExecutableName(activeCommand) ?? foregroundName
    }

    static func commandExecutableName(_ command: String?) -> String? {
        guard let command else { return nil }
        let words = command.split(whereSeparator: \.isWhitespace)
        guard let executable = words.first(where: { !$0.contains("=") }) else { return nil }
        let name = (String(executable) as NSString).lastPathComponent
        return name.isEmpty ? nil : name
    }
}
