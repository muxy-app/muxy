import Foundation

enum ShellEscaper {
    private static let metaCharacters: Set<Character> = [
        " ", "(", ")", "'", "\"", "\\", "&", "|", ";", "$", "`", "!",
    ]

    static func escape(_ path: String) -> String {
        guard path.contains(where: metaCharacters.contains) else { return path }
        return "'" + path.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }
}
