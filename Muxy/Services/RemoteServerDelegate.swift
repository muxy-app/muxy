import Foundation
import GhosttyKit
import MuxyServer
import MuxyShared
import os

private let logger = Logger(subsystem: "app.muxy", category: "RemoteServerDelegate")

@MainActor
final class RemoteServerDelegate: MuxyRemoteServerDelegate {
    private let appState: AppState
    private let projectStore: ProjectStore
    private let worktreeStore: WorktreeStore
    private let gitService = GitRepositoryService()

    init(appState: AppState, projectStore: ProjectStore, worktreeStore: WorktreeStore) {
        self.appState = appState
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
    }

    func listProjects() -> [ProjectDTO] {
        projectStore.projects.map { $0.toDTO() }
    }

    func selectProject(_ projectID: UUID) {
        guard let project = projectStore.projects.first(where: { $0.id == projectID }) else { return }
        let worktreeList = worktreeStore.list(for: projectID)
        guard let worktree = worktreeList.first(where: \.isPrimary) ?? worktreeList.first else { return }
        appState.selectProject(project, worktree: worktree)
    }

    func listWorktrees(projectID: UUID) -> [WorktreeDTO] {
        worktreeStore.list(for: projectID).map { $0.toDTO() }
    }

    func selectWorktree(projectID: UUID, worktreeID: UUID) {
        guard let worktree = worktreeStore.worktree(projectID: projectID, worktreeID: worktreeID) else { return }
        appState.selectWorktree(projectID: projectID, worktree: worktree)
    }

    func getWorkspace(projectID: UUID) -> WorkspaceDTO? {
        guard let key = appState.activeWorktreeKey(for: projectID),
              let root = appState.workspaceRoots[key]
        else { return nil }

        return WorkspaceDTO(
            projectID: projectID,
            worktreeID: key.worktreeID,
            focusedAreaID: appState.focusedAreaID[key],
            root: root.toDTO()
        )
    }

    func createTab(projectID: UUID, areaID: UUID?, kind: TabKindDTO) -> TabDTO? {
        switch kind {
        case .terminal:
            appState.dispatch(.createTab(projectID: projectID, areaID: areaID))
        case .vcs:
            appState.dispatch(.createVCSTab(projectID: projectID, areaID: areaID))
        case .editor:
            appState.dispatch(.createTab(projectID: projectID, areaID: areaID))
        }

        guard let area = appState.focusedArea(for: projectID),
              let tab = area.activeTab
        else { return nil }

        return tab.toDTO()
    }

    func closeTab(projectID: UUID, areaID: UUID, tabID: UUID) {
        appState.dispatch(.closeTab(projectID: projectID, areaID: areaID, tabID: tabID))
    }

    func selectTab(projectID: UUID, areaID: UUID, tabID: UUID) {
        appState.dispatch(.selectTab(projectID: projectID, areaID: areaID, tabID: tabID))
    }

    func splitArea(projectID: UUID, areaID: UUID, direction: SplitDirectionDTO, position: SplitPositionDTO) {
        let dir: SplitDirection = direction == .horizontal ? .horizontal : .vertical
        let pos: SplitPosition = position == .first ? .first : .second
        appState.dispatch(.splitArea(.init(
            projectID: projectID,
            areaID: areaID,
            direction: dir,
            position: pos
        )))
    }

    func closeArea(projectID: UUID, areaID: UUID) {
        appState.dispatch(.closeArea(projectID: projectID, areaID: areaID))
    }

    func focusArea(projectID: UUID, areaID: UUID) {
        appState.dispatch(.focusArea(projectID: projectID, areaID: areaID))
    }

    func sendTerminalInput(paneID: UUID, text: String) {
        guard let view = TerminalViewRegistry.shared.existingView(for: paneID) else {
            logger.warning("No terminal view for pane \(paneID)")
            return
        }
        view.sendText(text)
    }

    func getTerminalContent(paneID: UUID) -> TerminalContentDTO? {
        guard let view = TerminalViewRegistry.shared.existingView(for: paneID),
              let surface = view.surface
        else { return nil }

        let size = ghostty_surface_size(surface)
        var content = ""
        if ghostty_surface_has_selection(surface) {
            var text = ghostty_text_s()
            if ghostty_surface_read_selection(surface, &text) {
                if let ptr = text.text, text.text_len > 0 {
                    content = String(cString: ptr)
                }
                ghostty_surface_free_text(surface, &text)
            }
        }

        return TerminalContentDTO(
            paneID: paneID,
            content: content,
            cols: UInt32(size.columns),
            rows: UInt32(size.rows)
        )
    }

    func getVCSStatus(projectID: UUID) async -> VCSStatusDTO? {
        guard let project = projectStore.projects.first(where: { $0.id == projectID }) else { return nil }
        let repoPath = resolveWorktreePath(projectID: projectID) ?? project.path

        do {
            let branch = try await gitService.currentBranch(repoPath: repoPath)
            let aheadBehind = await gitService.aheadBehind(repoPath: repoPath, branch: branch)

            return VCSStatusDTO(
                branch: branch,
                aheadCount: aheadBehind.ahead,
                behindCount: aheadBehind.behind,
                stagedFiles: [],
                changedFiles: []
            )
        } catch {
            logger.error("Failed to get VCS status: \(error)")
            return nil
        }
    }

    func vcsCommit(projectID: UUID, message: String, stageAll: Bool) async throws {
        guard let project = projectStore.projects.first(where: { $0.id == projectID }) else { return }
        let repoPath = resolveWorktreePath(projectID: projectID) ?? project.path

        if stageAll {
            try await gitService.stageAll(repoPath: repoPath)
        }
        _ = try await gitService.commit(repoPath: repoPath, message: message)
    }

    func vcsPush(projectID: UUID) async throws {
        guard let project = projectStore.projects.first(where: { $0.id == projectID }) else { return }
        let repoPath = resolveWorktreePath(projectID: projectID) ?? project.path
        try await gitService.push(repoPath: repoPath)
    }

    func vcsPull(projectID: UUID) async throws {
        guard let project = projectStore.projects.first(where: { $0.id == projectID }) else { return }
        let repoPath = resolveWorktreePath(projectID: projectID) ?? project.path
        try await gitService.pull(repoPath: repoPath)
    }

    func listNotifications() -> [NotificationDTO] {
        NotificationStore.shared.notifications.map { $0.toDTO() }
    }

    func markNotificationRead(_ notificationID: UUID) {
        NotificationStore.shared.markAsRead(notificationID)
    }

    private func resolveWorktreePath(projectID: UUID) -> String? {
        guard let worktreeID = appState.activeWorktreeID[projectID],
              let worktree = worktreeStore.worktree(projectID: projectID, worktreeID: worktreeID)
        else { return nil }
        return worktree.path
    }
}
