import Foundation

struct CodexSessionStore: AgentSessionStore {
    private let homeDirectory: String

    init(homeDirectory: String = NSHomeDirectory()) {
        self.homeDirectory = homeDirectory
    }

    func sessions(inDirectory directory: String) -> [AgentSessionRef] {
        let root = homeDirectory + "/.codex/sessions"
        let fileManager = FileManager.default
        guard let enumerator = fileManager.enumerator(atPath: root) else { return [] }

        var results: [AgentSessionRef] = []
        for case let relative as String in enumerator {
            guard relative.hasSuffix(".jsonl"),
                  (relative as NSString).lastPathComponent.hasPrefix("rollout-")
            else { continue }
            let path = root + "/" + relative
            guard let header = Self.header(atPath: path),
                  let payload = header["payload"] as? [String: Any],
                  (payload["cwd"] as? String) == directory,
                  let sessionID = payload["session_id"] as? String
            else { continue }
            let attributes = try? fileManager.attributesOfItem(atPath: path)
            let updatedAt = (attributes?[.modificationDate] as? Date) ?? Date(timeIntervalSince1970: 0)
            results.append(AgentSessionRef(
                id: sessionID, providerID: "codex", cwd: directory, gitBranch: nil,
                title: nil, preview: nil, updatedAt: updatedAt, pinned: false, archived: false))
        }
        return results.sorted { $0.updatedAt > $1.updatedAt }
    }

    private static func header(atPath path: String) -> [String: Any]? {
        guard let handle = FileManager.default.contents(atPath: path),
              let text = String(data: handle, encoding: .utf8),
              let firstLine = text.split(separator: "\n", maxSplits: 1).first,
              let data = firstLine.data(using: .utf8)
        else { return nil }
        return try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    }
}
