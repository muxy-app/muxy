import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "WorktreeStore")

@MainActor
@Observable
final class WorktreeStore {
    private(set) var worktrees: [UUID: [Worktree]] = [:]
    private var projectIDByPath: [String: UUID] = [:]
    private let persistence: any WorktreePersisting

    init(persistence: any WorktreePersisting, projects: [Project] = []) {
        self.persistence = persistence
        guard !projects.isEmpty else { return }
        loadAll(projects: projects)
    }

    func loadAll(projects: [Project]) {
        for project in projects {
            do {
                var loaded = try persistence.loadWorktrees(projectID: project.id)
                if !loaded.contains(where: \.isPrimary) {
                    loaded.insert(makePrimary(for: project), at: 0)
                    try? persistence.saveWorktrees(loaded, projectID: project.id)
                }
                setWorktrees(sortPrimaryFirst(loaded), for: project.id)
            } catch {
                logger.error("Failed to load worktrees for project \(project.id): \(error)")
                setWorktrees([makePrimary(for: project)], for: project.id)
                save(projectID: project.id)
            }
        }
    }

    func ensurePrimary(for project: Project) {
        var list = worktrees[project.id] ?? []
        if list.contains(where: \.isPrimary) { return }
        list.insert(makePrimary(for: project), at: 0)
        setWorktrees(sortPrimaryFirst(list), for: project.id)
        save(projectID: project.id)
    }

    func list(for projectID: UUID) -> [Worktree] {
        worktrees[projectID] ?? []
    }

    func projectID(forWorktreePath path: String) -> UUID? {
        projectIDByPath[path]
    }

    func primary(for projectID: UUID) -> Worktree? {
        list(for: projectID).first(where: { $0.isPrimary })
    }

    func worktree(projectID: UUID, worktreeID: UUID) -> Worktree? {
        list(for: projectID).first(where: { $0.id == worktreeID })
    }

    func add(_ worktree: Worktree, to projectID: UUID) {
        var list = worktrees[projectID] ?? []
        list.append(worktree)
        setWorktrees(sortPrimaryFirst(list), for: projectID)
        save(projectID: projectID)
    }

    func remove(worktreeID: UUID, from projectID: UUID) {
        guard var list = worktrees[projectID] else { return }
        list.removeAll { $0.id == worktreeID && !$0.isPrimary }
        setWorktrees(list, for: projectID)
        save(projectID: projectID)
    }

    static func cleanupOnDisk(
        worktree: Worktree,
        repoPath: String
    ) async {
        guard !worktree.isPrimary else { return }
        do {
            try await GitWorktreeService.shared.removeWorktree(
                repoPath: repoPath,
                path: worktree.path,
                force: true
            )
        } catch {
            logger.error("Failed to remove git worktree at \(worktree.path): \(error)")
        }

        if worktree.ownsBranch,
           let branch = worktree.branch?.trimmingCharacters(in: .whitespacesAndNewlines),
           !branch.isEmpty
        {
            do {
                try await GitWorktreeService.shared.deleteBranch(repoPath: repoPath, branch: branch)
            } catch {
                logger.error("Failed to delete branch \(branch) for worktree \(worktree.path): \(error)")
            }
        }

        try? FileManager.default.removeItem(atPath: worktree.path)
        removeParentDirectoryIfEmpty(for: worktree.path)
    }

    static func cleanupOnDisk(for project: Project, knownWorktrees: [Worktree]) async {
        let secondaryWorktrees = knownWorktrees.filter { !$0.isPrimary }
        for worktree in secondaryWorktrees {
            await cleanupOnDisk(worktree: worktree, repoPath: project.path)
        }

        let root = MuxyFileStorage.worktreeRoot(forProjectID: project.id)
        guard FileManager.default.fileExists(atPath: root.path) else { return }
        let children = (try? FileManager.default.contentsOfDirectory(atPath: root.path)) ?? []
        for child in children {
            let childPath = root.appendingPathComponent(child).path
            try? await GitWorktreeService.shared.removeWorktree(
                repoPath: project.path,
                path: childPath,
                force: true
            )
            try? FileManager.default.removeItem(atPath: childPath)
        }
        try? FileManager.default.removeItem(at: root)
    }

    private static func removeParentDirectoryIfEmpty(for path: String) {
        let parent = URL(fileURLWithPath: path).deletingLastPathComponent()
        let children = (try? FileManager.default.contentsOfDirectory(atPath: parent.path)) ?? []
        guard children.isEmpty else { return }
        try? FileManager.default.removeItem(at: parent)
    }

    func rename(worktreeID: UUID, in projectID: UUID, to newName: String) {
        guard var list = worktrees[projectID],
              let index = list.firstIndex(where: { $0.id == worktreeID })
        else { return }
        list[index].name = newName
        setWorktrees(list, for: projectID)
        save(projectID: projectID)
    }

    func updateBranch(worktreeID: UUID, in projectID: UUID, branch: String?) {
        guard var list = worktrees[projectID],
              let index = list.firstIndex(where: { $0.id == worktreeID })
        else { return }
        list[index].branch = branch
        setWorktrees(list, for: projectID)
        save(projectID: projectID)
    }

    func removeProject(_ projectID: UUID) {
        if let existing = worktrees[projectID] {
            for worktree in existing where projectIDByPath[worktree.path] == projectID {
                projectIDByPath.removeValue(forKey: worktree.path)
            }
        }
        worktrees.removeValue(forKey: projectID)
        do {
            try persistence.removeWorktrees(projectID: projectID)
        } catch {
            logger.error("Failed to remove worktrees file for project \(projectID): \(error)")
        }
    }

    private func setWorktrees(_ list: [Worktree], for projectID: UUID) {
        if let previous = worktrees[projectID] {
            for worktree in previous where projectIDByPath[worktree.path] == projectID {
                projectIDByPath.removeValue(forKey: worktree.path)
            }
        }
        for worktree in list {
            projectIDByPath[worktree.path] = projectID
        }
        worktrees[projectID] = list
    }

    private func makePrimary(for project: Project) -> Worktree {
        Worktree(
            name: project.name,
            path: project.path,
            branch: nil,
            isPrimary: true
        )
    }

    private func sortPrimaryFirst(_ list: [Worktree]) -> [Worktree] {
        let primary = list.filter(\.isPrimary)
        let others = list.filter { !$0.isPrimary }.sorted { $0.createdAt < $1.createdAt }
        return primary + others
    }

    private func save(projectID: UUID) {
        guard let list = worktrees[projectID] else { return }
        do {
            try persistence.saveWorktrees(list, projectID: projectID)
        } catch {
            logger.error("Failed to save worktrees for project \(projectID): \(error)")
        }
    }
}
