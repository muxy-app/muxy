import Foundation

struct ClaudeSessionStore: AgentSessionStore {
    private let homeDirectory: String

    init(homeDirectory: String = NSHomeDirectory()) {
        self.homeDirectory = homeDirectory
    }

    func sessions(inDirectory directory: String) -> [AgentSessionRef] {
        let slug = directory.replacingOccurrences(of: "/", with: "-")
        let folder = homeDirectory + "/.claude/projects/" + slug
        let fileManager = FileManager.default
        guard let entries = try? fileManager.contentsOfDirectory(atPath: folder) else { return [] }

        return entries.compactMap { entry in
            guard entry.hasSuffix(".jsonl") else { return nil }
            let path = folder + "/" + entry
            let id = String(entry.dropLast(".jsonl".count))
            let attributes = try? fileManager.attributesOfItem(atPath: path)
            let updatedAt = (attributes?[.modificationDate] as? Date) ?? Date(timeIntervalSince1970: 0)
            return AgentSessionRef(
                id: id,
                providerID: "claude",
                cwd: directory,
                gitBranch: nil,
                title: nil,
                preview: Self.firstUserMessage(atPath: path),
                updatedAt: updatedAt,
                pinned: false,
                archived: false
            )
        }
        .sorted { $0.updatedAt > $1.updatedAt }
    }

    private static func firstUserMessage(atPath path: String) -> String? {
        guard let handle = try? FileHandle(forReadingFrom: URL(fileURLWithPath: path)) else { return nil }
        defer { try? handle.close() }
        guard let chunk = try? handle.read(upToCount: 65536),
              let contents = String(data: chunk, encoding: .utf8)
        else { return nil }
        for line in contents.split(separator: "\n") {
            guard let data = line.data(using: .utf8),
                  let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let message = object["message"] as? [String: Any],
                  (message["role"] as? String) == "user"
            else { continue }
            let text = Self.plainText(from: message["content"]) ?? ""
            return String(text.prefix(120))
        }
        return nil
    }

    private static func plainText(from content: Any?) -> String? {
        if let string = content as? String {
            return string
        }
        if let blocks = content as? [[String: Any]] {
            return blocks.compactMap { $0["text"] as? String }.joined(separator: " ")
        }
        return nil
    }
}
