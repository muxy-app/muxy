#if DEBUG
import Foundation

enum DotEnvLoader {
    static func value(for key: String) -> String? {
        guard let url = locateDotEnv() else { return nil }
        guard let contents = try? String(contentsOf: url, encoding: .utf8) else { return nil }
        for line in contents.split(whereSeparator: \.isNewline) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard !trimmed.isEmpty, !trimmed.hasPrefix("#") else { continue }
            guard let separator = trimmed.firstIndex(of: "=") else { continue }
            let name = trimmed[..<separator].trimmingCharacters(in: .whitespaces)
            guard name == key else { continue }
            var raw = trimmed[trimmed.index(after: separator)...].trimmingCharacters(in: .whitespaces)
            if raw.hasPrefix("\"") && raw.hasSuffix("\"") && raw.count >= 2 {
                raw = String(raw.dropFirst().dropLast())
            } else if raw.hasPrefix("'") && raw.hasSuffix("'") && raw.count >= 2 {
                raw = String(raw.dropFirst().dropLast())
            }
            return raw.isEmpty ? nil : raw
        }
        return nil
    }

    private static func locateDotEnv() -> URL? {
        let fileManager = FileManager.default
        var directory = URL(fileURLWithPath: fileManager.currentDirectoryPath, isDirectory: true)
        for _ in 0 ..< 6 {
            let candidate = directory.appendingPathComponent(".env")
            if fileManager.fileExists(atPath: candidate.path) {
                return candidate
            }
            let parent = directory.deletingLastPathComponent()
            if parent.path == directory.path { break }
            directory = parent
        }
        return nil
    }
}
#endif
