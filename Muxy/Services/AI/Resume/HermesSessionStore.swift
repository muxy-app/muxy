import Foundation

struct HermesSessionStore: AgentSessionStore {
    private let homeDirectory: String

    init(homeDirectory: String = NSHomeDirectory()) {
        self.homeDirectory = homeDirectory
    }

    func sessions(inDirectory directory: String) -> [AgentSessionRef] {
        let path = homeDirectory + "/.hermes/state.db"
        guard FileManager.default.fileExists(atPath: path) else { return [] }
        let query = """
        SELECT id, title, cwd, git_branch, started_at, pinned, archived \
        FROM sessions WHERE cwd = ? ORDER BY started_at DESC
        """
        return SQLiteReader.rows(databasePath: path, query: query, parameters: [directory]).compactMap { row in
            guard let id = row["id"] else { return nil }
            let seconds = Double(row["started_at"] ?? "0") ?? 0
            return AgentSessionRef(
                id: id, providerID: "hermes", cwd: directory, gitBranch: row["git_branch"],
                title: row["title"], preview: row["title"],
                updatedAt: Date(timeIntervalSince1970: seconds),
                pinned: row["pinned"] == "1", archived: row["archived"] == "1")
        }
    }
}
