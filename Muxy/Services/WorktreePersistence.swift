import Foundation

protocol WorktreePersisting {
    func loadWorktrees(projectID: UUID) throws -> [Worktree]
    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws
    func removeWorktrees(projectID: UUID) throws
}

final class FileWorktreePersistence: WorktreePersisting {
    private let directory: URL

    init(directory: URL = MuxyFileStorage.appSupportDirectory().appendingPathComponent("worktrees", isDirectory: true)) {
        self.directory = directory
        try? FileManager.default.createDirectory(
            at: directory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
    }

    func loadWorktrees(projectID: UUID) throws -> [Worktree] {
        let url = fileURL(for: projectID)
        guard FileManager.default.fileExists(atPath: url.path) else { return [] }
        let data = try Data(contentsOf: url)
        return try JSONDecoder().decode([Worktree].self, from: data)
    }

    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws {
        let encoder = JSONEncoder()
        encoder.outputFormatting = .prettyPrinted
        let data = try encoder.encode(worktrees)
        try data.write(to: fileURL(for: projectID), options: .atomic)
    }

    func removeWorktrees(projectID: UUID) throws {
        let url = fileURL(for: projectID)
        guard FileManager.default.fileExists(atPath: url.path) else { return }
        try FileManager.default.removeItem(at: url)
    }

    private func fileURL(for projectID: UUID) -> URL {
        directory.appendingPathComponent("\(projectID.uuidString).json")
    }
}
