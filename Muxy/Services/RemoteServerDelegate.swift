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
        case "\u{1B}[A": return KeyEvent(codepoint: 0, keycode: 126, mods: GHOSTTY_MODS_NONE)
        case "\u{1B}[B": return KeyEvent(codepoint: 0, keycode: 125, mods: GHOSTTY_MODS_NONE)
        case "\u{1B}[C": return KeyEvent(codepoint: 0, keycode: 124, mods: GHOSTTY_MODS_NONE)
        case "\u{1B}[D": return KeyEvent(codepoint: 0, keycode: 123, mods: GHOSTTY_MODS_NONE)
        default: return nil
        }
    }

    private static func specialKeyEvent(_ scalar: UInt32) -> KeyEvent? {
        switch scalar {
        case 0x01: return KeyEvent(codepoint: 97, keycode: 0, mods: GHOSTTY_MODS_CTRL)
        case 0x02: return KeyEvent(codepoint: 98, keycode: 11, mods: GHOSTTY_MODS_CTRL)
        case 0x03: return KeyEvent(codepoint: 99, keycode: 8, mods: GHOSTTY_MODS_CTRL)
        case 0x04: return KeyEvent(codepoint: 100, keycode: 2, mods: GHOSTTY_MODS_CTRL)
        case 0x05: return KeyEvent(codepoint: 101, keycode: 14, mods: GHOSTTY_MODS_CTRL)
        case 0x06: return KeyEvent(codepoint: 102, keycode: 3, mods: GHOSTTY_MODS_CTRL)
        case 0x0C: return KeyEvent(codepoint: 108, keycode: 37, mods: GHOSTTY_MODS_CTRL)
        case 0x1A: return KeyEvent(codepoint: 122, keycode: 6, mods: GHOSTTY_MODS_CTRL)
        case 0x09: return KeyEvent(codepoint: 9, keycode: 48, mods: GHOSTTY_MODS_NONE)
        case 0x1B: return KeyEvent(codepoint: 27, keycode: 53, mods: GHOSTTY_MODS_NONE)
        case 0x7F: return KeyEvent(codepoint: 8, keycode: 51, mods: GHOSTTY_MODS_NONE)
        default: return nil
        }
    }

    func getTerminalContent(paneID: UUID) -> TerminalContentDTO? {
        guard let view = TerminalViewRegistry.shared.existingView(for: paneID),
              let surface = view.surface
        else { return nil }

        let size = ghostty_surface_size(surface)

        var topLeft = ghostty_point_s()
        topLeft.tag = GHOSTTY_POINT_VIEWPORT
        topLeft.coord = GHOSTTY_POINT_COORD_TOP_LEFT
        topLeft.x = 0
        topLeft.y = 0

        var bottomRight = ghostty_point_s()
        bottomRight.tag = GHOSTTY_POINT_VIEWPORT
        bottomRight.coord = GHOSTTY_POINT_COORD_BOTTOM_RIGHT
        bottomRight.x = UInt32(size.columns)
        bottomRight.y = UInt32(size.rows)

        var selection = ghostty_selection_s()
        selection.top_left = topLeft
        selection.bottom_right = bottomRight
        selection.rectangle = false

        var text = ghostty_text_s()
        var content = ""
        if ghostty_surface_read_text(surface, selection, &text) {
            if let ptr = text.text, text.text_len > 0 {
                content = String(cString: ptr)
            }
            ghostty_surface_free_text(surface, &text)
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
