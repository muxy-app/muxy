import Foundation

struct AgySessionStore: AgentSessionStore {
    private let homeDirectory: String

    init(homeDirectory: String = NSHomeDirectory()) {
        self.homeDirectory = homeDirectory
    }

    func sessions(inDirectory directory: String) -> [AgentSessionRef] {
        let path = homeDirectory + "/.gemini/antigravity-cli/cache/last_conversations.json"
        guard let data = FileManager.default.contents(atPath: path),
              let map = try? JSONSerialization.jsonObject(with: data) as? [String: String]
        else { return [] }
        let candidates = [directory, directory + "/"]
        guard let key = candidates.first(where: { map[$0] != nil }), let id = map[key] else { return [] }
        let attributes = try? FileManager.default.attributesOfItem(atPath: path)
        let updatedAt = (attributes?[.modificationDate] as? Date) ?? Date(timeIntervalSince1970: 0)
        return [AgentSessionRef(
            id: id, providerID: "agy", cwd: directory, gitBranch: nil,
            title: nil, preview: nil, updatedAt: updatedAt, pinned: false, archived: false)]
    }
}
