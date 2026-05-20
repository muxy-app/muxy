enum GhosttyConfigFile {
    static func value(for key: String, in content: String) -> String? {
        let lines = content.components(separatedBy: .newlines)
        guard let index = lineIndex(for: key, in: lines) else { return nil }
        let trimmed = lines[index].trimmingCharacters(in: .whitespaces)
        let afterKey = trimmed.dropFirst(key.count).trimmingCharacters(in: .whitespaces)
        return afterKey.dropFirst().trimmingCharacters(in: .whitespaces)
    }

    static func settingValue(_ value: String, for key: String, in content: String) -> String {
        let entry = "\(key) = \(value)"
        var lines = content.components(separatedBy: "\n")

        if let index = lineIndex(for: key, in: lines) {
            lines[index] = entry
        } else {
            lines.insert(entry, at: 0)
        }

        return lines.joined(separator: "\n")
    }

    static func removingValue(for key: String, in content: String) -> String {
        var lines = content.components(separatedBy: "\n")
        guard let index = lineIndex(for: key, in: lines) else { return content }
        lines.remove(at: index)
        return lines.joined(separator: "\n")
    }

    private static func lineIndex(for key: String, in lines: [String]) -> Int? {
        for (i, line) in lines.enumerated() {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard trimmed.hasPrefix(key) else { continue }
            let afterKey = trimmed.dropFirst(key.count).trimmingCharacters(in: .whitespaces)
            guard afterKey.hasPrefix("=") else { continue }
            return i
        }
        return nil
    }
}
