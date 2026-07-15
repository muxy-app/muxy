import Foundation
import Testing

@testable import Muxy
@testable import MuxyShared

@Suite("ProjectStore")
@MainActor
struct ProjectStoreTests {
    @Test("add assigns a random icon color not used by other projects")
    func addAssignsUnusedIconColor() {
        var existing = Project(name: "Repo", path: "/tmp/repo")
        existing.iconColor = "blue"
        let persistence = ProjectPersistenceStub(initial: [existing])
        let store = ProjectStore(persistence: persistence)

        let project = Project(name: "New", path: "/tmp/new")
        store.add(project)

        let stored = store.storedProjects.first { $0.id == project.id }
        #expect(stored?.iconColor != nil)
        #expect(stored?.iconColor != "blue")
        #expect(ProjectIconColor.swatch(for: stored?.iconColor) != nil)
        #expect(persistence.projects.first { $0.id == project.id }?.iconColor == stored?.iconColor)
    }

    @Test("add keeps an explicitly set icon color")
    func addKeepsExplicitIconColor() {
        let persistence = ProjectPersistenceStub(initial: [])
        let store = ProjectStore(persistence: persistence)

        var project = Project(name: "New", path: "/tmp/new")
        project.iconColor = "red"
        store.add(project)

        #expect(store.storedProjects.first { $0.id == project.id }?.iconColor == "red")
    }

    @Test("add still assigns a palette color when every color is in use")
    func addAssignsColorWhenPaletteExhausted() {
        let existing = ProjectIconColor.palette.map { swatch in
            var project = Project(name: swatch.id, path: "/tmp/\(swatch.id)")
            project.iconColor = swatch.id
            return project
        }
        let persistence = ProjectPersistenceStub(initial: existing)
        let store = ProjectStore(persistence: persistence)

        let project = Project(name: "New", path: "/tmp/new")
        store.add(project)

        let stored = store.storedProjects.first { $0.id == project.id }
        #expect(ProjectIconColor.swatch(for: stored?.iconColor) != nil)
    }

    @Test("setPreferredWorktreeParentPath persists normalized path")
    func setPreferredWorktreeParentPath() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.setPreferredWorktreeParentPath(id: project.id, to: " ~/worktrees ")

        let stored = store.storedProjects.first { $0.id == project.id }
        #expect(stored?.preferredWorktreeParentPath == NSString(string: "~/worktrees").expandingTildeInPath)
        #expect(persistence.projects.first?.preferredWorktreeParentPath == NSString(string: "~/worktrees").expandingTildeInPath)
    }

    @Test("setPreferredWorktreeParentPath clears empty path")
    func clearPreferredWorktreeParentPath() {
        var project = Project(name: "Repo", path: "/tmp/repo")
        project.preferredWorktreeParentPath = "/tmp/worktrees"
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.setPreferredWorktreeParentPath(id: project.id, to: " ")

        let stored = store.storedProjects.first { $0.id == project.id }
        #expect(stored?.preferredWorktreeParentPath == nil)
        #expect(persistence.projects.first?.preferredWorktreeParentPath == nil)
    }

    @Test("setPullRequestPrompt persists and clears the project override")
    func setPullRequestPrompt() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.setPullRequestPrompt(id: project.id, to: "Keep the summary concise")

        #expect(store.storedProjects.first?.pullRequestPrompt == "Keep the summary concise")
        #expect(persistence.projects.first?.pullRequestPrompt == "Keep the summary concise")

        store.setPullRequestPrompt(id: project.id, to: " \n ")

        #expect(store.storedProjects.first?.pullRequestPrompt == nil)
        #expect(persistence.projects.first?.pullRequestPrompt == nil)
    }

    @Test("setWorktreesEnabled persists the new value")
    func setWorktreesEnabled() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.setWorktreesEnabled(id: project.id, to: true)

        #expect(store.storedProjects.first { $0.id == project.id }?.worktreesEnabled == true)
        #expect(persistence.projects.first?.worktreesEnabled == true)
    }

    @Test("setPinned persists the new value")
    func setPinned() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.setPinned(id: project.id, to: true)

        #expect(store.storedProjects.first { $0.id == project.id }?.isPinned == true)
        #expect(persistence.projects.first?.isPinned == true)
    }

    @Test("setPinned ignores the Home project")
    func setPinnedIgnoresHome() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.setPinned(id: Project.homeID, to: true)

        #expect(store.storedProjects.allSatisfy { !$0.isPinned })
    }

    @Test("projects always exposes Home at the front without persisting it")
    func projectsSynthesizesHome() {
        let existing = Project(name: "Repo", path: "/tmp/repo")
        let persistence = ProjectPersistenceStub(initial: [existing])
        let store = ProjectStore(persistence: persistence)

        #expect(store.projects.first?.isHome == true)
        #expect(store.projects.count == 2)
        #expect(store.storedProjects.contains(where: { $0.isHome }) == false)
        #expect(persistence.projects.contains(where: { $0.isHome }) == false)
    }

    @Test("load drops any persisted Home record")
    func loadDropsPersistedHome() {
        let persistence = ProjectPersistenceStub(initial: [Project.home, Project(name: "Repo", path: "/tmp/repo")])
        let store = ProjectStore(persistence: persistence)

        #expect(store.storedProjects.contains(where: { $0.isHome }) == false)
        #expect(store.projects.filter(\.isHome).count == 1)
    }

    @Test("remove never deletes the Home project")
    func removeIgnoresHome() {
        let persistence = ProjectPersistenceStub(initial: [])
        let store = ProjectStore(persistence: persistence)

        store.remove(id: Project.homeID)

        #expect(store.projects.contains { $0.isHome })
    }

    @Test("markActive stamps lastActiveAt and persists")
    func markActiveStampsTimestamp() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        #expect(store.storedProjects.first?.lastActiveAt == nil)

        store.markActive(id: project.id)

        #expect(store.storedProjects.first?.lastActiveAt != nil)
        #expect(persistence.projects.first?.lastActiveAt != nil)
    }

    @Test("markActive ignores unknown ids")
    func markActiveIgnoresUnknown() {
        let persistence = ProjectPersistenceStub(initial: [Project(name: "Repo", path: "/tmp/repo")])
        let store = ProjectStore(persistence: persistence)

        store.markActive(id: UUID())

        #expect(store.storedProjects.first?.lastActiveAt == nil)
    }

    @Test("persistOrder rewrites sortOrder to match the given order")
    func persistOrderRewritesSortOrder() {
        let first = Project(name: "A", path: "/tmp/a", sortOrder: 0)
        let second = Project(name: "B", path: "/tmp/b", sortOrder: 1)
        let third = Project(name: "C", path: "/tmp/c", sortOrder: 2)
        let persistence = ProjectPersistenceStub(initial: [first, second, third])
        let store = ProjectStore(persistence: persistence)

        store.persistOrder([third.id, first.id, second.id])

        #expect(store.storedProjects.map(\.id) == [third.id, first.id, second.id])
        #expect(store.storedProjects.map(\.sortOrder) == [0, 1, 2])
        #expect(persistence.projects.map(\.id) == [third.id, first.id, second.id])
    }

    @Test("remove archives the complete local project snapshot")
    func removeArchivesCompleteProject() throws {
        var project = Project(name: "Custom Repo", path: "/tmp/repo", sortOrder: 4)
        project.icon = "shippingbox.fill"
        project.logo = "logo.png"
        project.iconColor = "purple"
        project.preferredWorktreeParentPath = "/tmp/worktrees"
        project.pullRequestPrompt = "Keep it short"
        project.worktreesEnabled = true
        project.isPinned = true
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.remove(id: project.id)

        let archived = try #require(store.recentlyRemovedProjects.first)
        #expect(archived.project == project)
        #expect(persistence.recentlyRemovedProjects == store.recentlyRemovedProjects)
        #expect(store.storedProjects.isEmpty)
    }

    @Test("recently removed projects are newest first and bounded")
    func recentlyRemovedProjectsAreBounded() {
        let persistence = ProjectPersistenceStub(initial: [])
        let store = ProjectStore(persistence: persistence)

        for index in 0 ..< ProjectStore.recentlyRemovedProjectLimit + 2 {
            let project = Project(name: "Repo \(index)", path: "/tmp/repo-\(index)")
            store.add(project)
            store.remove(id: project.id)
        }

        #expect(store.recentlyRemovedProjects.count == ProjectStore.recentlyRemovedProjectLimit)
        #expect(store.recentlyRemovedProjects.first?.project.name == "Repo 11")
        #expect(store.recentlyRemovedProjects.last?.project.name == "Repo 2")
        #expect(persistence.recentlyRemovedProjects == store.recentlyRemovedProjects)
    }

    @Test("load removes duplicate and active paths from recent history")
    func loadSanitizesRecentlyRemovedProjects() {
        let active = Project(name: "Active", path: "/tmp/active")
        let oldDuplicate = Project(name: "Old", path: "/tmp/duplicate/.")
        let newDuplicate = Project(name: "New", path: "/tmp/duplicate")
        let history = [
            RecentlyRemovedProject(project: oldDuplicate, removedAt: Date(timeIntervalSince1970: 1)),
            RecentlyRemovedProject(project: active, removedAt: Date(timeIntervalSince1970: 3)),
            RecentlyRemovedProject(project: newDuplicate, removedAt: Date(timeIntervalSince1970: 2)),
        ]
        let persistence = ProjectPersistenceStub(initial: [active], recentlyRemovedProjects: history)
        let store = ProjectStore(persistence: persistence)

        #expect(store.recentlyRemovedProjects.map(\.project.name) == ["New"])
    }

    @Test("remote projects and unknown ids are not archived")
    func removeExcludesRemoteAndUnknownProjects() {
        let remote = Project(
            name: "Remote",
            path: "/srv/repo",
            remoteDeviceID: UUID()
        )
        let persistence = ProjectPersistenceStub(initial: [remote])
        let store = ProjectStore(persistence: persistence)

        store.remove(id: UUID())
        store.remove(id: remote.id)

        #expect(store.recentlyRemovedProjects.isEmpty)
        #expect(persistence.recentlyRemovedProjects.isEmpty)
    }

    @Test("remote paths do not hide matching local recent projects")
    func remotePathsDoNotMatchLocalHistory() {
        let local = Project(name: "Local", path: "/srv/repo")
        let remote = Project(
            name: "Remote",
            path: "/srv/repo",
            remoteDeviceID: UUID()
        )
        let history = [RecentlyRemovedProject(project: local, removedAt: Date())]
        let persistence = ProjectPersistenceStub(initial: [remote], recentlyRemovedProjects: history)
        let store = ProjectStore(persistence: persistence)

        store.add(Project(
            name: "Another Remote",
            path: "/srv/repo",
            remoteDeviceID: UUID()
        ))

        #expect(store.recentlyRemovedProjects.first?.id == local.id)
    }

    @Test("restore preserves metadata and removes the history entry")
    func restorePreservesMetadata() throws {
        var project = Project(name: "Custom", path: "/tmp/custom", sortOrder: 0)
        project.icon = "star.fill"
        project.logo = "stored-logo.png"
        project.iconColor = nil
        project.worktreesEnabled = true
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)
        store.remove(id: project.id)

        let restored = try #require(store.restoreRecentlyRemovedProject(id: project.id))

        #expect(restored.id == project.id)
        #expect(restored.name == project.name)
        #expect(restored.icon == project.icon)
        #expect(restored.logo == project.logo)
        #expect(restored.iconColor == nil)
        #expect(restored.worktreesEnabled)
        #expect(store.recentlyRemovedProjects.isEmpty)
        #expect(persistence.recentlyRemovedProjects.isEmpty)
    }

    @Test("adding an equivalent path clears stale recent history")
    func addClearsEquivalentRecentlyRemovedProject() {
        let removed = Project(name: "Old", path: "/tmp/repo/.")
        let history = [RecentlyRemovedProject(project: removed, removedAt: Date())]
        let persistence = ProjectPersistenceStub(initial: [], recentlyRemovedProjects: history)
        let store = ProjectStore(persistence: persistence)

        store.add(Project(name: "New", path: "/tmp/repo"))

        #expect(store.recentlyRemovedProjects.isEmpty)
        #expect(persistence.recentlyRemovedProjects.isEmpty)
    }

    @Test("failed add preserves matching recent history")
    func failedAddPreservesRecentlyRemovedProject() {
        let removed = Project(name: "Old", path: "/tmp/repo")
        let history = [RecentlyRemovedProject(project: removed, removedAt: Date())]
        let persistence = ProjectPersistenceStub(initial: [], recentlyRemovedProjects: history)
        let store = ProjectStore(persistence: persistence)
        persistence.projectSaveError = ProjectPersistenceTestError.saveFailed

        let didAdd = store.add(Project(name: "New", path: "/tmp/repo"))

        #expect(!didAdd)
        #expect(store.storedProjects.isEmpty)
        #expect(store.recentlyRemovedProjects == history)
        #expect(persistence.recentlyRemovedProjects == history)
    }

    @Test("failed restore preserves recent history")
    func failedRestorePreservesRecentlyRemovedProject() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let history = [RecentlyRemovedProject(project: project, removedAt: Date())]
        let persistence = ProjectPersistenceStub(initial: [], recentlyRemovedProjects: history)
        let store = ProjectStore(persistence: persistence)
        persistence.projectSaveError = ProjectPersistenceTestError.saveFailed

        let restored = store.restoreRecentlyRemovedProject(id: project.id)

        #expect(restored == nil)
        #expect(store.storedProjects.isEmpty)
        #expect(store.recentlyRemovedProjects == history)
        #expect(persistence.projects.isEmpty)
        #expect(persistence.recentlyRemovedProjects == history)
    }

    @Test("failed recent history save keeps project active")
    func failedArchiveSaveKeepsProjectActive() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)
        persistence.recentlyRemovedSaveError = ProjectPersistenceTestError.saveFailed

        let didRemove = store.remove(id: project.id)

        #expect(!didRemove)
        #expect(store.storedProjects.first?.id == project.id)
        #expect(store.recentlyRemovedProjects.isEmpty)
        #expect(persistence.projects.first?.id == project.id)
        #expect(persistence.recentlyRemovedProjects.isEmpty)
    }

    @Test("failed active project verification preserves a full recent history")
    func failedActiveProjectVerificationPreservesFullHistory() {
        let project = Project(name: "Active", path: "/tmp/active")
        let history = (0 ..< ProjectStore.recentlyRemovedProjectLimit).map { index in
            RecentlyRemovedProject(
                project: Project(name: "Recent \(index)", path: "/tmp/recent-\(index)"),
                removedAt: Date(timeIntervalSince1970: TimeInterval(index))
            )
        }
        let persistence = ProjectPersistenceStub(initial: [project], recentlyRemovedProjects: history)
        let store = ProjectStore(persistence: persistence)
        persistence.projectSaveError = ProjectPersistenceTestError.saveFailed

        let didRemove = store.remove(id: project.id)
        let expectedHistory = Array(history.reversed())

        #expect(!didRemove)
        #expect(store.storedProjects.first?.id == project.id)
        #expect(store.recentlyRemovedProjects == expectedHistory)
        #expect(persistence.projects.first?.id == project.id)
        #expect(persistence.recentlyRemovedProjects == expectedHistory)
    }

    @Test("file persistence restores recently removed projects after relaunch")
    func filePersistenceRestoresHistory() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("ProjectStoreTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let projectsURL = directory.appendingPathComponent("projects.json")
        let recentURL = directory.appendingPathComponent("recently-removed-projects.json")
        let persistence = FileProjectPersistence(fileURL: projectsURL, recentlyRemovedFileURL: recentURL)
        let project = Project(name: "Repo", path: "/tmp/repo")
        let store = ProjectStore(persistence: persistence)
        store.add(project)
        store.remove(id: project.id)

        let reloaded = ProjectStore(
            persistence: FileProjectPersistence(fileURL: projectsURL, recentlyRemovedFileURL: recentURL)
        )

        #expect(reloaded.recentlyRemovedProjects.first?.project.id == project.id)
        #expect(reloaded.recentlyRemovedProjects.first?.project.name == project.name)
    }
}

private final class ProjectPersistenceStub: ProjectPersisting {
    var projects: [Project]
    var recentlyRemovedProjects: [RecentlyRemovedProject]
    var projectSaveError: Error?
    var recentlyRemovedSaveError: Error?

    init(initial: [Project], recentlyRemovedProjects: [RecentlyRemovedProject] = []) {
        projects = initial
        self.recentlyRemovedProjects = recentlyRemovedProjects
    }

    func loadProjects() throws -> [Project] {
        projects
    }

    func saveProjects(_ projects: [Project]) throws {
        if let projectSaveError { throw projectSaveError }
        self.projects = projects
    }

    func loadRecentlyRemovedProjects() throws -> [RecentlyRemovedProject] {
        recentlyRemovedProjects
    }

    func saveRecentlyRemovedProjects(_ projects: [RecentlyRemovedProject]) throws {
        if let recentlyRemovedSaveError { throw recentlyRemovedSaveError }
        recentlyRemovedProjects = projects
    }
}

private enum ProjectPersistenceTestError: Error {
    case saveFailed
}
