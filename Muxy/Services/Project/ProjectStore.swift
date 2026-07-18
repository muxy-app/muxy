import AppKit
import Foundation
import MuxyShared
import os

private let logger = Logger(subsystem: "app.muxy", category: "ProjectStore")

private struct PendingProjectRemoval {
    let project: Project
    let previousRecentlyRemovedProjects: [RecentlyRemovedProject]
    let finalRecentlyRemovedProjects: [RecentlyRemovedProject]
    let logoIDsToRemove: Set<UUID>
}

@MainActor
@Observable
final class ProjectStore {
    static let recentlyRemovedProjectLimit = 10

    private(set) var storedProjects: [Project] = []
    private(set) var recentlyRemovedProjects: [RecentlyRemovedProject] = []
    private let persistence: any ProjectPersisting
    private var pendingRemoval: PendingProjectRemoval?
    private var removalSlotReserved = false
    private var removalSlotWaiters: [CheckedContinuation<Void, Never>] = []
    var onProjectRemoved: ((UUID) -> Void)?
    var onProjectsChanged: (() -> Void)?

    init(persistence: any ProjectPersisting) {
        self.persistence = persistence
        load()
    }

    var projects: [Project] {
        [Project.home] + storedProjects
    }

    private var usedIconColors: Set<String> {
        Set(storedProjects.compactMap(\.iconColor))
    }

    @discardableResult
    func add(_ project: Project) -> Bool {
        if let pendingRemoval {
            guard !projectsMatch(pendingRemoval.project, project) else { return false }
            guard !recentlyRemovedProjects.contains(where: { projectsMatch($0.project, project) }) else {
                return false
            }
        }
        var project = project
        if project.iconColor == nil {
            project.iconColor = ProjectIconColor.randomSwatch(excluding: usedIconColors)?.id
        }
        storedProjects.append(project)
        guard save(notify: false) else {
            storedProjects.removeAll { $0.id == project.id }
            return false
        }
        removeRecentlyRemovedProject(matching: project)
        onProjectsChanged?()
        return true
    }

    @discardableResult
    func remove(id: UUID) -> Bool {
        guard !removalSlotReserved else { return false }
        removalSlotReserved = true
        guard prepareRemoval(id: id) else {
            releaseRemovalSlot()
            return false
        }
        return commitRemoval(id: id)
    }

    func prepareRemovalWhenAvailable(id: UUID) async -> Bool {
        await reserveRemovalSlot()
        guard prepareRemoval(id: id) else {
            releaseRemovalSlot()
            return false
        }
        return true
    }

    private func prepareRemoval(id: UUID) -> Bool {
        guard pendingRemoval == nil else { return false }
        guard id != Project.homeID else { return false }
        guard let project = storedProjects.first(where: { $0.id == id }) else { return false }
        let previous = recentlyRemovedProjects
        guard !project.isRemote else {
            pendingRemoval = PendingProjectRemoval(
                project: project,
                previousRecentlyRemovedProjects: previous,
                finalRecentlyRemovedProjects: previous,
                logoIDsToRemove: []
            )
            return true
        }

        let replacedProjects = previous.filter { projectsMatch($0.project, project) }
        var stagedProjects = previous.filter { !projectsMatch($0.project, project) }
        stagedProjects.insert(RecentlyRemovedProject(project: project, removedAt: Date()), at: 0)
        let finalProjects = Array(stagedProjects.prefix(Self.recentlyRemovedProjectLimit))
        let evictedProjects = stagedProjects.dropFirst(Self.recentlyRemovedProjectLimit)
        let replacedLogoIDs = replacedProjects.filter { $0.id != project.id }.map(\.id)
        let logoIDsToRemove = Set(replacedLogoIDs + evictedProjects.map(\.id))
        guard saveRecentlyRemovedProjects(stagedProjects) else { return false }
        guard save(notify: false) else {
            saveRecentlyRemovedProjects(previous)
            return false
        }
        pendingRemoval = PendingProjectRemoval(
            project: project,
            previousRecentlyRemovedProjects: previous,
            finalRecentlyRemovedProjects: finalProjects,
            logoIDsToRemove: logoIDsToRemove
        )
        return true
    }

    func cancelRemoval(id: UUID) {
        guard let pendingRemoval, pendingRemoval.project.id == id else { return }
        recentlyRemovedProjects = pendingRemoval.previousRecentlyRemovedProjects
        if !pendingRemoval.project.isRemote {
            saveRecentlyRemovedProjects(recentlyRemovedProjects)
        }
        self.pendingRemoval = nil
        releaseRemovalSlot()
    }

    @discardableResult
    func commitRemoval(id: UUID) -> Bool {
        guard let pendingRemoval, pendingRemoval.project.id == id else { return false }
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else {
            cancelRemoval(id: id)
            return false
        }
        let project = storedProjects.remove(at: index)
        guard save(notify: false) else {
            storedProjects.insert(project, at: index)
            cancelRemoval(id: id)
            return false
        }

        recentlyRemovedProjects = pendingRemoval.finalRecentlyRemovedProjects
        let didSaveFinalHistory = project.isRemote || saveRecentlyRemovedProjects(recentlyRemovedProjects)
        if didSaveFinalHistory {
            for logoID in pendingRemoval.logoIDsToRemove {
                ProjectLogoStorage.remove(forProjectID: logoID)
            }
        }
        self.pendingRemoval = nil
        releaseRemovalSlot()
        onProjectsChanged?()
        onProjectRemoved?(id)
        return true
    }

    @discardableResult
    func restoreRecentlyRemovedProject(id: UUID) -> Project? {
        guard pendingRemoval == nil else { return nil }
        guard let index = recentlyRemovedProjects.firstIndex(where: { $0.id == id }) else { return nil }
        let entry = recentlyRemovedProjects[index]
        var project = entry.project
        project.sortOrder = (storedProjects.map(\.sortOrder).max() ?? -1) + 1
        storedProjects.append(project)
        guard save(notify: false) else {
            storedProjects.removeAll { $0.id == project.id }
            return nil
        }
        recentlyRemovedProjects.remove(at: index)
        saveRecentlyRemovedProjects()
        onProjectsChanged?()
        return project
    }

    func rename(id: UUID, to newName: String) {
        guard pendingRemoval?.project.id != id else { return }
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].name = newName
        save()
    }

    func setLogo(id: UUID, to logo: String?) {
        guard pendingRemoval?.project.id != id else { return }
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        if logo == nil {
            ProjectLogoStorage.remove(forProjectID: id)
        }
        storedProjects[index].logo = logo
        save()
    }

    func setLogo(id: UUID, croppedImage: NSImage) {
        guard pendingRemoval?.project.id != id else { return }
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        let logo = ProjectLogoStorage.save(croppedImage: croppedImage, forProjectID: id)
        storedProjects[index].logo = logo
        save()
    }

    func setIcon(id: UUID, to icon: String?) {
        guard pendingRemoval?.project.id != id else { return }
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].icon = icon
        save()
    }

    func setIconColor(id: UUID, to color: String?) {
        guard pendingRemoval?.project.id != id else { return }
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].iconColor = color
        save()
    }

    func setPinned(id: UUID, to pinned: Bool) {
        guard pendingRemoval?.project.id != id else { return }
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].isPinned = pinned
        save()
    }

    func setWorktreesEnabled(id: UUID, to enabled: Bool) {
        guard pendingRemoval?.project.id != id else { return }
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].worktreesEnabled = enabled
        save()
    }

    func setPreferredWorktreeLocation(id: UUID, pathTemplate: String?, parentPath: String?) throws {
        guard pendingRemoval?.project.id != id else { return }
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        let normalizedTemplate = WorktreeLocationResolver.normalizedLocation(pathTemplate)
        if normalizedTemplate != nil {
            _ = try WorktreeLocationResolver.validatedPathTemplate(normalizedTemplate)
        }
        storedProjects[index].preferredWorktreePathTemplate = normalizedTemplate
        storedProjects[index].preferredWorktreeParentPath = WorktreeLocationResolver.normalizedLocation(parentPath)
        save()
    }

    func setPullRequestPrompt(id: UUID, to prompt: String?) {
        guard pendingRemoval?.project.id != id else { return }
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].pullRequestPrompt = RepositoryAIActionPreferences.normalizedPrompt(prompt)
        save()
    }

    func markActive(id: UUID) {
        guard pendingRemoval?.project.id != id else { return }
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].lastActiveAt = Date()
        save(notify: false)
    }

    func persistOrder(_ orderedIDs: [UUID]) {
        guard pendingRemoval == nil else { return }
        let positions = Dictionary(uniqueKeysWithValues: orderedIDs.enumerated().map { ($1, $0) })
        storedProjects.sort { positions[$0.id, default: Int.max] < positions[$1.id, default: Int.max] }
        for index in storedProjects.indices {
            storedProjects[index].sortOrder = index
        }
        save()
    }

    func persistOrder(_ orderedIDs: [UUID], scopedTo scopedIDs: Set<UUID>) {
        guard pendingRemoval == nil else { return }
        let projectsByID = Dictionary(uniqueKeysWithValues: storedProjects.map { ($0.id, $0) })
        var scopedProjects = orderedIDs.compactMap { projectsByID[$0] }
        storedProjects = storedProjects.map { project in
            guard scopedIDs.contains(project.id), !scopedProjects.isEmpty else { return project }
            return scopedProjects.removeFirst()
        }
        for index in storedProjects.indices {
            storedProjects[index].sortOrder = index
        }
        save()
    }

    func reorder(fromOffsets source: IndexSet, toOffset destination: Int) {
        guard pendingRemoval == nil else { return }
        storedProjects.move(fromOffsets: source, toOffset: destination)
        for index in storedProjects.indices {
            storedProjects[index].sortOrder = index
        }
        save()
    }

    @discardableResult
    func save(notify: Bool = true) -> Bool {
        do {
            try persistence.saveProjects(storedProjects)
        } catch {
            logger.error("Failed to save projects: \(error)")
            if notify {
                onProjectsChanged?()
            }
            return false
        }
        if notify {
            onProjectsChanged?()
        }
        return true
    }

    private func load() {
        do {
            storedProjects = try persistence.loadProjects().filter { !$0.isHome }
            storedProjects.sort { $0.sortOrder < $1.sortOrder }
        } catch {
            logger.error("Failed to load projects: \(error)")
        }
        do {
            let loaded = try persistence.loadRecentlyRemovedProjects().sorted { $0.removedAt > $1.removedAt }
            recentlyRemovedProjects = []
            for entry in loaded {
                guard !entry.project.isHome, !entry.project.isRemote else { continue }
                guard !storedProjects.contains(where: { projectsMatch($0, entry.project) }) else { continue }
                guard !recentlyRemovedProjects.contains(where: { projectsMatch($0.project, entry.project) }) else {
                    continue
                }
                recentlyRemovedProjects.append(entry)
                guard recentlyRemovedProjects.count < Self.recentlyRemovedProjectLimit else { break }
            }
        } catch {
            logger.error("Failed to load recently removed projects: \(error)")
        }
    }

    private func removeRecentlyRemovedProject(matching project: Project) {
        let removedProjects = recentlyRemovedProjects.filter { projectsMatch($0.project, project) }
        guard !removedProjects.isEmpty else { return }
        recentlyRemovedProjects.removeAll { projectsMatch($0.project, project) }
        for entry in removedProjects where entry.id != project.id {
            ProjectLogoStorage.remove(forProjectID: entry.id)
        }
        saveRecentlyRemovedProjects()
    }

    private func projectsMatch(_ lhs: Project, _ rhs: Project) -> Bool {
        if lhs.id == rhs.id {
            return true
        }
        guard !lhs.isRemote, !rhs.isRemote else { return false }
        return ProjectPickerPathService.standardizedPath(lhs.path)
            == ProjectPickerPathService.standardizedPath(rhs.path)
    }

    @discardableResult
    private func saveRecentlyRemovedProjects(_ projects: [RecentlyRemovedProject]? = nil) -> Bool {
        do {
            try persistence.saveRecentlyRemovedProjects(projects ?? recentlyRemovedProjects)
        } catch {
            logger.error("Failed to save recently removed projects: \(error)")
            return false
        }
        return true
    }

    private func reserveRemovalSlot() async {
        guard removalSlotReserved else {
            removalSlotReserved = true
            return
        }
        await withCheckedContinuation { continuation in
            removalSlotWaiters.append(continuation)
        }
    }

    private func releaseRemovalSlot() {
        guard !removalSlotWaiters.isEmpty else {
            removalSlotReserved = false
            return
        }
        removalSlotWaiters.removeFirst().resume()
    }
}
