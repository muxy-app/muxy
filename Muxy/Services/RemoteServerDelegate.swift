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

        if let arrowKey = Self.ansiArrowKey(text) {
            view.sendKeyPress(codepoint: arrowKey.codepoint, keycode: arrowKey.keycode, mods: arrowKey.mods)
            return
        }

        var buffer = ""
        for character in text {
            let scalar = character.unicodeScalars.first?.value ?? 0
            if let keyEvent = Self.specialKeyEvent(scalar) {
                if !buffer.isEmpty {
                    view.sendText(buffer)
                    buffer = ""
                }
                view.sendKeyPress(
                    codepoint: keyEvent.codepoint,
                    keycode: keyEvent.keycode,
                    mods: keyEvent.mods
                )
            } else if character == "\r" || character == "\n" {
                if !buffer.isEmpty {
                    view.sendText(buffer)
                    buffer = ""
                }
                view.sendReturnKey()
            } else {
                buffer.append(character)
            }
        }
        if !buffer.isEmpty {
            view.sendText(buffer)
        }
    }

    private struct KeyEvent {
        let codepoint: UInt32
        let keycode: UInt32
        let mods: ghostty_input_mods_e
    }

    private static func ansiArrowKey(_ text: String) -> KeyEvent? {
        switch text {
        case "\u{1B}[A": KeyEvent(codepoint: 0, keycode: 126, mods: GHOSTTY_MODS_NONE)
        case "\u{1B}[B": KeyEvent(codepoint: 0, keycode: 125, mods: GHOSTTY_MODS_NONE)
        case "\u{1B}[C": KeyEvent(codepoint: 0, keycode: 124, mods: GHOSTTY_MODS_NONE)
        case "\u{1B}[D": KeyEvent(codepoint: 0, keycode: 123, mods: GHOSTTY_MODS_NONE)
        default: nil
        }
    }

    private static func specialKeyEvent(_ scalar: UInt32) -> KeyEvent? {
        switch scalar {
        case 0x01: KeyEvent(codepoint: 97, keycode: 0, mods: GHOSTTY_MODS_CTRL)
        case 0x02: KeyEvent(codepoint: 98, keycode: 11, mods: GHOSTTY_MODS_CTRL)
        case 0x03: KeyEvent(codepoint: 99, keycode: 8, mods: GHOSTTY_MODS_CTRL)
        case 0x04: KeyEvent(codepoint: 100, keycode: 2, mods: GHOSTTY_MODS_CTRL)
        case 0x05: KeyEvent(codepoint: 101, keycode: 14, mods: GHOSTTY_MODS_CTRL)
        case 0x06: KeyEvent(codepoint: 102, keycode: 3, mods: GHOSTTY_MODS_CTRL)
        case 0x0C: KeyEvent(codepoint: 108, keycode: 37, mods: GHOSTTY_MODS_CTRL)
        case 0x1A: KeyEvent(codepoint: 122, keycode: 6, mods: GHOSTTY_MODS_CTRL)
        case 0x09: KeyEvent(codepoint: 9, keycode: 48, mods: GHOSTTY_MODS_NONE)
        case 0x1B: KeyEvent(codepoint: 27, keycode: 53, mods: GHOSTTY_MODS_NONE)
        case 0x7F: KeyEvent(codepoint: 8, keycode: 51, mods: GHOSTTY_MODS_NONE)
        default: nil
        }
    }

    func resizeTerminal(paneID: UUID, cols: UInt32, rows: UInt32) {
        guard let view = TerminalViewRegistry.shared.existingView(for: paneID),
              let surface = view.surface
        else { return }

        let size = ghostty_surface_size(surface)
        guard size.cell_width_px > 0, size.cell_height_px > 0 else { return }

        let w = cols * size.cell_width_px
        let h = rows * size.cell_height_px
        ghostty_surface_set_size(surface, w, h)
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
