import Foundation

protocol WorktreeService: Sendable {
    func isRepository(_ path: String) async -> Bool
    func hasUncommittedChanges(worktreePath: String) async -> Bool
    func listWorktrees(repoPath: String) async throws -> [WorktreeRecord]
    func addWorktree(repoPath: String, path: String, branch: String, createBranch: Bool) async throws
    func removeWorktree(repoPath: String, path: String, force: Bool) async throws
}
