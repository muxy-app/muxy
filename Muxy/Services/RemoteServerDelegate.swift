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
    weak var server: MuxyRemoteServer?

    init(appState: AppState, projectStore: ProjectStore, worktreeStore: WorktreeStore) {
        self.appState = appState
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
        PaneOwnershipStore.shared.onOwnershipChanged = { [weak self] paneID, owner in
            TerminalViewRegistry.shared.existingView(for: paneID)?.remoteOwnershipDidChange()
            self?.broadcastOwnership(paneID: paneID, owner: owner)
        }
    }

    private func broadcastOwnership(paneID: UUID, owner: PaneOwnerDTO) {
        let dto = PaneOwnershipEventDTO(paneID: paneID, owner: owner)
        server?.broadcast(MuxyEvent(event: .paneOwnershipChanged, data: .paneOwnership(dto)))
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

    func sendTerminalInput(paneID: UUID, text: String, clientID: UUID) {
        guard let view = TerminalViewRegistry.shared.existingView(for: paneID) else {
            logger.warning("No terminal view for pane \(paneID)")
            return
        }

        guard PaneOwnershipStore.shared.isOwnedBy(clientID: clientID, paneID: paneID) else {
            return
        }

        view.sendRemoteText(text)
    }

    func scrollTerminal(paneID: UUID, deltaX: Double, deltaY: Double, precise: Bool, clientID: UUID) {
        guard let view = TerminalViewRegistry.shared.existingView(for: paneID),
              let surface = view.surface
        else { return }

        guard PaneOwnershipStore.shared.isOwnedBy(clientID: clientID, paneID: paneID) else {
            return
        }

        let mods: ghostty_input_scroll_mods_t = precise ? 1 : 0
        ghostty_surface_mouse_scroll(surface, deltaX, deltaY, mods)
    }

    func resizeTerminal(paneID: UUID, cols: UInt32, rows: UInt32, clientID: UUID) {
        guard PaneOwnershipStore.shared.isOwnedBy(clientID: clientID, paneID: paneID) else {
            return
        }
        applyPTYSize(paneID: paneID, cols: cols, rows: rows)
    }

    private func applyPTYSize(paneID: UUID, cols: UInt32, rows: UInt32) {
        guard let view = TerminalViewRegistry.shared.existingView(for: paneID),
              let surface = view.surface
        else { return }

        let size = ghostty_surface_size(surface)
        guard size.cell_width_px > 0, size.cell_height_px > 0 else { return }

        let w = cols * size.cell_width_px
        let h = rows * size.cell_height_px
        ghostty_surface_set_size(surface, w, h)
    }

    func registerDevice(clientID: UUID, name: String) {
        PaneOwnershipStore.shared.registerDevice(clientID: clientID, name: name)
    }

    func takeOverPane(paneID: UUID, clientID: UUID, cols: UInt32, rows: UInt32) {
        PaneOwnershipStore.shared.assign(paneID: paneID, to: clientID)
        applyPTYSize(paneID: paneID, cols: cols, rows: rows)
    }

    func releasePane(paneID: UUID, clientID: UUID) {
        guard PaneOwnershipStore.shared.isOwnedBy(clientID: clientID, paneID: paneID) else {
            return
        }
        PaneOwnershipStore.shared.releaseToMac(paneID: paneID)
    }

    func clientDisconnected(clientID: UUID) {
        PaneOwnershipStore.shared.releaseAll(clientID: clientID)
    }

    func getPaneOwner(paneID: UUID) -> PaneOwnerDTO? {
        PaneOwnershipStore.shared.owner(for: paneID)
    }

    func getTerminalContent(paneID: UUID) -> TerminalCellsDTO? {
        guard let view = TerminalViewRegistry.shared.existingView(for: paneID),
              let surface = view.surface
        else { return nil }

        var out = ghostty_cells_s()
        guard ghostty_surface_read_cells(surface, &out) else { return nil }
        defer { ghostty_surface_free_cells(surface, &out) }

        let total = Int(out.cells_len)
        var cells: [TerminalCellDTO] = []
        cells.reserveCapacity(total)
        if let ptr = out.cells {
            for i in 0 ..< total {
                let cell = ptr[i]
                cells.append(TerminalCellDTO(
                    codepoint: cell.codepoint,
                    fg: cell.fg_rgb,
                    bg: cell.bg_rgb,
                    flags: cell.flags
                ))
            }
        }

        return TerminalCellsDTO(
            paneID: paneID,
            cols: out.cols,
            rows: out.rows,
            cursorX: out.cursor_x,
            cursorY: out.cursor_y,
            cursorVisible: out.cursor_visible,
            defaultFg: out.default_fg,
            defaultBg: out.default_bg,
            cells: cells
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

    func getProjectLogo(projectID: UUID) -> ProjectLogoDTO? {
        guard let project = projectStore.projects.first(where: { $0.id == projectID }),
              let logo = project.logo
        else { return nil }
        let path = ProjectLogoStorage.logoPath(for: logo)
        guard let data = FileManager.default.contents(atPath: path) else { return nil }
        return ProjectLogoDTO(projectID: projectID, pngData: data.base64EncodedString())
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
