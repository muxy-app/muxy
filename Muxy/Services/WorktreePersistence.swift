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
        try store(for: projectID).load() ?? []
    }

    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws {
        try store(for: projectID).save(worktrees)
    }

    func removeWorktrees(projectID: UUID) throws {
        try store(for: projectID).remove()
    }

    private func store(for projectID: UUID) -> CodableFileStore<[Worktree]> {
        CodableFileStore(
            fileURL: directory.appendingPathComponent("\(projectID.uuidString).json"),
            options: .pretty
        )
    }
}
