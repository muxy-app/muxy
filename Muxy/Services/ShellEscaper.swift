import Foundation

enum ShellEscaper {
    private static let safeCharacters: Set<Character> = {
        var characters = Set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789")
        characters.formUnion(["-", "_", ".", "/", ":", "@", "%", "+", ","])
        return characters
    }()

    static func escape(_ path: String) -> String {
        guard path.contains(where: { !safeCharacters.contains($0) }) else { return path }
        return "'" + path.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }
}
