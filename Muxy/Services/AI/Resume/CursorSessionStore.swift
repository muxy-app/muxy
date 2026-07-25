import Foundation

struct CursorSessionStore: AgentSessionStore {
    private let homeDirectory: String

    init(homeDirectory: String = NSHomeDirectory()) {
        self.homeDirectory = homeDirectory
    }

    func sessions(inDirectory directory: String) -> [AgentSessionRef] {
        let trimmed = directory.hasPrefix("/") ? String(directory.dropFirst()) : directory
        let slug = trimmed.replacingOccurrences(of: "/", with: "-")
        let folder = homeDirectory + "/.cursor/projects/" + slug + "/agent-transcripts"
        let fileManager = FileManager.default
        guard let chats = try? fileManager.contentsOfDirectory(atPath: folder) else { return [] }

        return chats.compactMap { chatID in
            let path = folder + "/" + chatID + "/" + chatID + ".jsonl"
            guard fileManager.fileExists(atPath: path) else { return nil }
            let attributes = try? fileManager.attributesOfItem(atPath: path)
            let updatedAt = (attributes?[.modificationDate] as? Date) ?? Date(timeIntervalSince1970: 0)
            return AgentSessionRef(
                id: chatID, providerID: "cursor", cwd: directory, gitBranch: nil,
                title: nil, preview: Self.firstUserText(atPath: path),
                updatedAt: updatedAt, pinned: false, archived: false)
        }
        .sorted { $0.updatedAt > $1.updatedAt }
    }

    private static func firstUserText(atPath path: String) -> String? {
        guard let contents = try? String(contentsOfFile: path, encoding: .utf8),
              let firstLine = contents.split(separator: "\n").first,
              let data = firstLine.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let message = object["message"] as? [String: Any],
              let blocks = message["content"] as? [[String: Any]]
        else { return nil }
        let text = blocks.compactMap { $0["text"] as? String }.joined(separator: " ")
        let cleaned = text
            .replacingOccurrences(of: "<user_query>", with: "")
            .replacingOccurrences(of: "</user_query>", with: "")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return String(cleaned.prefix(120))
    }
}
