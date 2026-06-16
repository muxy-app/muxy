import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "WorktreeStore")

struct WorktreeCreationRequest {
    let name: String
    let path: String
    let branch: String
    let createBranch: Bool
    let baseBranch: String?
}

@MainActor
@Observable
final class WorktreeStore {
    private(set) var worktrees: [UUID: [Worktree]] = [:]
    private var projectIDByPath: [String: UUID] = [:]
    private let persistence: any WorktreePersisting
    private let listGitWorktrees: @Sendable (String) async throws -> [GitWorktreeRecord]
    private let addGitWorktree: @Sendable (String, String, String, Bool, String?) async throws -> Void

    init(
        persistence: any WorktreePersisting,
        listGitWorktrees: @escaping @Sendable (String) async throws -> [GitWorktreeRecord] = {
            try await GitWorktreeService.shared.listWorktrees(repoPath: $0)
        },
        addGitWorktree: @escaping @Sendable (String, String, String, Bool, String?) async throws -> Void = {
            try await GitWorktreeService.shared.addWorktree(
                repoPath: $0,
                path: $1,
                branch: $2,
                createBranch: $3,
                baseBranch: $4
            )
        },
        projects: [Project] = []
    ) {
        self.persistence = persistence
        self.listGitWorktrees = listGitWorktrees
        self.addGitWorktree = addGitWorktree
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

    func preferred(for projectID: UUID, matching preferredID: UUID?) -> Worktree? {
        let list = list(for: projectID)
        return list.first(where: { $0.id == preferredID })
            ?? list.first(where: { $0.isPrimary })
            ?? list.first
    }

    func add(_ worktree: Worktree, to projectID: UUID) {
        var list = worktrees[projectID] ?? []
        list.append(worktree)
        setWorktrees(sortPrimaryFirst(list), for: projectID)
        save(projectID: projectID)
    }

    func createWorktree(
        project: Project,
        request: WorktreeCreationRequest,
        context: WorkspaceContext = .local
    ) async throws -> Worktree {
        let parentPath = parentDirectory(of: request.path, context: context)
        try await context.fileOps.makeDirectory(at: parentPath)

        try await addWorktreeForContext(project: project, request: request, context: context)
        let worktree = Worktree(
            name: request.name,
            path: request.path,
            branch: request.branch,
            isPrimary: false
        )
        add(worktree, to: project.id)
        return worktree
    }

    private func addWorktreeForContext(
        project: Project,
        request: WorktreeCreationRequest,
        context: WorkspaceContext
    ) async throws {
        guard context.isRemote else {
            try await addGitWorktree(
                project.path,
                request.path,
                request.branch,
                request.createBranch,
                request.baseBranch
            )
            return
        }
        try await GitWorktreeService.shared.addWorktree(
            repoPath: project.path,
            path: request.path,
            branch: request.branch,
            createBranch: request.createBranch,
            baseBranch: request.baseBranch,
            context: context
        )
    }

    private func parentDirectory(of path: String, context: WorkspaceContext) -> String {
        guard context.isRemote else {
            return URL(fileURLWithPath: path).deletingLastPathComponent().path
        }
        guard let slashIndex = path.lastIndex(of: "/") else { return "." }
        let parent = String(path[..<slashIndex])
        return parent.isEmpty ? "/" : parent
    }

    func remove(worktreeID: UUID, from projectID: UUID) {
        guard var list = worktrees[projectID] else { return }
        list.removeAll { $0.id == worktreeID && $0.canBeRemoved }
        setWorktrees(list, for: projectID)
        save(projectID: projectID)
    }

    func refreshFromGit(project: Project, context: WorkspaceContext = .local) async throws -> [Worktree] {
        ensurePrimary(for: project)
        let records = try await listWorktreesForContext(project: project, context: context)
            .filter { !$0.isBare && !$0.isPrunable }
        var list = worktrees[project.id] ?? []
        let projectKey = try await resolvedProjectKey(project: project, context: context)
        let recordKeys = Set(records.map { GitWorktreeService.canonicalPath($0.path, context: context) })

        if let primaryIndex = list.firstIndex(where: \.isPrimary) {
            list[primaryIndex].path = project.path
            list[primaryIndex].name = project.name
        } else {
            list.insert(makePrimary(for: project), at: 0)
        }

        var existingByKey: [String: Worktree] = [:]
        for worktree in list {
            let key = GitWorktreeService.canonicalPath(worktree.path, context: context)
            if let existing = existingByKey[key] {
                if worktree.isPrimary, !existing.isPrimary {
                    existingByKey[key] = worktree
                }
            } else {
                existingByKey[key] = worktree
            }
        }

        for record in records {
            let recordKey = GitWorktreeService.canonicalPath(record.path, context: context)
            if recordKey == projectKey {
                if let primaryIndex = list.firstIndex(where: \.isPrimary) {
                    list[primaryIndex].branch = record.branch
                }
                continue
            }

            if let existing = existingByKey[recordKey],
               let index = list.firstIndex(where: { $0.id == existing.id })
            {
                if list[index].isPrimary {
                    list[index].name = project.name
                    list[index].path = project.path
                } else if record.branch != nil, list[index].name == list[index].branch {
                    list[index].name = defaultName(for: record)
                }
                list[index].branch = record.branch
                continue
            }

            list.append(Worktree(
                name: defaultName(for: record),
                path: record.path,
                branch: record.branch,
                source: .external,
                isPrimary: false
            ))
        }

        let sorted = sortPrimaryFirst(list.filter {
            !$0.isExternallyManaged || recordKeys.contains(GitWorktreeService.canonicalPath($0.path, context: context))
        })
        setWorktrees(sorted, for: project.id)
        save(projectID: project.id)
        return sorted
    }

    private func resolvedProjectKey(project: Project, context: WorkspaceContext) async -> String {
        let fallback = GitWorktreeService.canonicalPath(project.path, context: context)
        guard context.isRemote else { return fallback }
        let resolved = await GitWorktreeService.shared.resolvedRepositoryRoot(
            repoPath: project.path,
            context: context
        )
        return resolved ?? fallback
    }

    private func listWorktreesForContext(
        project: Project,
        context: WorkspaceContext
    ) async throws -> [GitWorktreeRecord] {
        guard context.isRemote else {
            return try await listGitWorktrees(project.path)
        }
        return try await GitWorktreeService.shared.listWorktrees(repoPath: project.path, context: context)
    }

    static func cleanupOnDisk(
        worktree: Worktree,
        repoPath: String,
        context: WorkspaceContext = .local,
        teardownEmit: @Sendable @escaping (WorktreeTeardownOutputLine) -> Void = { _ in }
    ) async throws {
        guard worktree.canBeRemoved else { return }
        if !context.isRemote {
            try await WorktreeTeardownRunner.run(
                sourceProjectPath: repoPath,
                worktree: worktree,
                emit: teardownEmit
            )
        }
        try await GitWorktreeService.shared.removeWorktree(
            repoPath: repoPath,
            path: worktree.path,
            force: true,
            context: context
        )

        try? await context.fileOps.removeItem(at: worktree.path)
        guard !context.isRemote, !worktree.isExternallyManaged else { return }
        removeParentDirectoryIfEmpty(for: worktree.path)
    }

    private static func removeParentDirectoryIfEmpty(for path: String) {
        let parent = URL(fileURLWithPath: path).deletingLastPathComponent()
        let children = (try? FileManager.default.contentsOfDirectory(atPath: parent.path)) ?? []
        guard children.isEmpty else { return }
        try? FileManager.default.removeItem(at: parent)
    }

    static func cleanupOnDisk(
        for project: Project,
        knownWorktrees: [Worktree],
        context: WorkspaceContext = .local
    ) async throws {
        let secondaryWorktrees = knownWorktrees.filter { $0.canBeRemoved && !$0.isExternallyManaged }
        for worktree in secondaryWorktrees {
            try await cleanupOnDisk(worktree: worktree, repoPath: project.path, context: context)
        }

        guard !context.isRemote else { return }
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
            source: .muxy,
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

    private func defaultName(for record: GitWorktreeRecord) -> String {
        if let branch = record.branch?.trimmingCharacters(in: .whitespacesAndNewlines),
           !branch.isEmpty
        {
            return branch
        }
        return URL(fileURLWithPath: record.path).lastPathComponent
    }
}
