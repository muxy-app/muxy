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
        NotificationCenter.default.addObserver(
            forName: .themeDidChange,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.broadcastTheme()
            }
        }
    }

    private func broadcastOwnership(paneID: UUID, owner: PaneOwnerDTO) {
        let dto = PaneOwnershipEventDTO(paneID: paneID, owner: owner)
        server?.broadcast(MuxyEvent(event: .paneOwnershipChanged, data: .paneOwnership(dto)))
    }

    private func broadcastTheme() {
        guard let theme = ThemeService.shared.currentThemeColors() else { return }
        let dto = DeviceThemeEventDTO(fg: theme.fg, bg: theme.bg)
        server?.broadcast(MuxyEvent(event: .themeChanged, data: .deviceTheme(dto)))
    }

    func listProjects() -> [ProjectDTO] {
        projectStore.projects.map { $0.toDTO() }
    }

    func selectProject(_ projectID: UUID) {
        guard let project = projectStore.projects.first(where: { $0.id == projectID }) else { return }
        if appState.activeProjectID == projectID { return }
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
        case .diffViewer:
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

    func authenticateDevice(deviceID: UUID, token: String, name: String) -> DeviceAuthDecision {
        guard ApprovedDevicesStore.shared.devices.contains(where: { $0.id == deviceID }) else {
            return .unknown
        }
        guard let device = ApprovedDevicesStore.shared.validate(deviceID: deviceID, token: token) else {
            return .denied
        }
        if device.name != name {
            ApprovedDevicesStore.shared.rename(deviceID: deviceID, to: name)
        }
        ApprovedDevicesStore.shared.touch(deviceID: deviceID)
        return .approved(deviceName: name)
    }

    func requestPairing(deviceID: UUID, token: String, name: String) async -> DeviceAuthDecision {
        if ApprovedDevicesStore.shared.devices.contains(where: { $0.id == deviceID }) {
            return .denied
        }
        let approved = await PairingRequestCoordinator.shared.requestApproval(
            deviceID: deviceID,
            deviceName: name,
            token: token
        )
        guard approved else { return .denied }
        return .approved(deviceName: name)
    }

    func getDeviceTheme() -> (fg: UInt32, bg: UInt32)? {
        ThemeService.shared.currentThemeColors()
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
            let files = try await gitService.changedFiles(repoPath: repoPath)
            let defaultBranch = await gitService.defaultBranch(repoPath: repoPath)

            var pullRequest: VCSPullRequestDTO?
            if let headSha = await gitService.headSha(repoPath: repoPath),
               let info = await gitService.cachedPullRequestInfo(
                   repoPath: repoPath,
                   branch: branch,
                   headSha: headSha,
                   forceFresh: false
               )
            {
                pullRequest = VCSPullRequestDTO(
                    url: info.url,
                    number: info.number,
                    state: info.state.rawValue,
                    isDraft: info.isDraft,
                    baseBranch: info.baseBranch
                )
            }

            return VCSStatusDTO(
                branch: branch,
                aheadCount: aheadBehind.ahead,
                behindCount: aheadBehind.behind,
                hasUpstream: aheadBehind.hasUpstream,
                stagedFiles: files.filter(\.isStaged).map { Self.toFileDTO($0, staged: true) },
                changedFiles: files.filter(\.isUnstaged).map { Self.toFileDTO($0, staged: false) },
                defaultBranch: defaultBranch,
                pullRequest: pullRequest
            )
        } catch {
            logger.error("Failed to get VCS status: \(error)")
            return nil
        }
    }

    func vcsCommit(projectID: UUID, message: String, stageAll: Bool) async throws {
        let repoPath = try repoPath(projectID: projectID)
        if stageAll {
            try await gitService.stageAll(repoPath: repoPath)
        }
        _ = try await gitService.commit(repoPath: repoPath, message: message)
        notifyRepoDidChange(repoPath: repoPath)
    }

    func vcsPush(projectID: UUID) async throws {
        let repoPath = try repoPath(projectID: projectID)
        do {
            try await gitService.push(repoPath: repoPath)
        } catch GitRepositoryService.GitError.noUpstreamBranch {
            let branch = try await gitService.currentBranch(repoPath: repoPath)
            try await gitService.pushSetUpstream(repoPath: repoPath, branch: branch)
        }
        notifyRepoDidChange(repoPath: repoPath)
    }

    func vcsPull(projectID: UUID) async throws {
        let repoPath = try repoPath(projectID: projectID)
        try await gitService.pull(repoPath: repoPath)
        notifyRepoDidChange(repoPath: repoPath)
    }

    func vcsStageFiles(projectID: UUID, paths: [String]) async throws {
        let repoPath = try repoPath(projectID: projectID)
        try await gitService.stageFiles(repoPath: repoPath, paths: paths)
        notifyRepoDidChange(repoPath: repoPath)
    }

    func vcsUnstageFiles(projectID: UUID, paths: [String]) async throws {
        let repoPath = try repoPath(projectID: projectID)
        try await gitService.unstageFiles(repoPath: repoPath, paths: paths)
        notifyRepoDidChange(repoPath: repoPath)
    }

    func vcsDiscardFiles(projectID: UUID, paths: [String], untrackedPaths: [String]) async throws {
        let repoPath = try repoPath(projectID: projectID)
        try await gitService.discardFiles(
            repoPath: repoPath,
            paths: paths,
            untrackedPaths: untrackedPaths
        )
        notifyRepoDidChange(repoPath: repoPath)
    }

    func vcsListBranches(projectID: UUID) async throws -> VCSBranchesDTO {
        let repoPath = try repoPath(projectID: projectID)
        let current = try await gitService.currentBranch(repoPath: repoPath)
        let locals = try await gitService.listBranches(repoPath: repoPath)
        let defaultBranch = await gitService.defaultBranch(repoPath: repoPath)
        return VCSBranchesDTO(current: current, locals: locals, defaultBranch: defaultBranch)
    }

    func vcsSwitchBranch(projectID: UUID, branch: String) async throws {
        let repoPath = try repoPath(projectID: projectID)
        try await gitService.switchBranch(repoPath: repoPath, branch: branch)
        notifyRepoDidChange(repoPath: repoPath)
    }

    func vcsCreateBranch(projectID: UUID, name: String) async throws {
        let repoPath = try repoPath(projectID: projectID)
        try await gitService.createAndSwitchBranch(repoPath: repoPath, name: name)
        notifyRepoDidChange(repoPath: repoPath)
    }

    func vcsCreatePR(
        projectID: UUID,
        title: String,
        body: String,
        baseBranch: String?,
        draft: Bool
    ) async throws -> VCSCreatePRResultDTO {
        let repoPath = try repoPath(projectID: projectID)
        let branch = try await gitService.currentBranch(repoPath: repoPath)

        let hasRemote = await gitService.hasRemoteBranch(repoPath: repoPath, branch: branch)
        if !hasRemote {
            try await gitService.pushSetUpstream(repoPath: repoPath, branch: branch)
        }

        let trimmedBase = baseBranch?.trimmingCharacters(in: .whitespacesAndNewlines)
        let resolvedBase: String = if let trimmedBase, !trimmedBase.isEmpty {
            trimmedBase
        } else {
            await gitService.defaultBranch(repoPath: repoPath) ?? "main"
        }

        let info = try await gitService.createPullRequest(
            repoPath: repoPath,
            branch: branch,
            baseBranch: resolvedBase,
            title: title,
            body: body,
            draft: draft
        )
        notifyRepoDidChange(repoPath: repoPath)
        return VCSCreatePRResultDTO(url: info.url, number: info.number)
    }

    func vcsAddWorktree(
        projectID: UUID,
        name: String,
        branch: String,
        createBranch: Bool
    ) async throws -> WorktreeDTO {
        guard let project = projectStore.projects.first(where: { $0.id == projectID }) else {
            throw RemoteVCSError.projectNotFound
        }
        let trimmedName = name.trimmingCharacters(in: .whitespaces)
        guard !trimmedName.isEmpty else {
            throw RemoteVCSError.invalidInput("Worktree name is required.")
        }
        let trimmedBranch = branch.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedBranch.isEmpty else {
            throw RemoteVCSError.invalidInput("Branch name is required.")
        }
        let slug = Self.worktreeSlug(from: trimmedName)
        let worktreeDirectory = MuxyFileStorage
            .worktreeDirectory(forProjectID: project.id, name: slug)
            .path(percentEncoded: false)

        if FileManager.default.fileExists(atPath: worktreeDirectory) {
            throw RemoteVCSError.invalidInput("A worktree with this name already exists on disk.")
        }

        try await GitWorktreeService.shared.addWorktree(
            repoPath: project.path,
            path: worktreeDirectory,
            branch: trimmedBranch,
            createBranch: createBranch
        )

        let worktree = Worktree(
            name: trimmedName,
            path: worktreeDirectory,
            branch: trimmedBranch,
            ownsBranch: createBranch,
            isPrimary: false
        )
        worktreeStore.add(worktree, to: project.id)
        return worktree.toDTO()
    }

    func vcsRemoveWorktree(projectID: UUID, worktreeID: UUID) async throws {
        guard let project = projectStore.projects.first(where: { $0.id == projectID }) else {
            throw RemoteVCSError.projectNotFound
        }
        guard let worktree = worktreeStore.worktree(projectID: projectID, worktreeID: worktreeID) else {
            throw RemoteVCSError.worktreeNotFound
        }
        guard worktree.canBeRemoved else {
            throw RemoteVCSError.invalidInput(
                worktree.isPrimary
                    ? "The primary worktree cannot be removed."
                    : "This worktree is managed outside Muxy and cannot be removed here."
            )
        }

        await WorktreeStore.cleanupOnDisk(worktree: worktree, repoPath: project.path)
        worktreeStore.remove(worktreeID: worktreeID, from: projectID)
    }

    private func repoPath(projectID: UUID) throws -> String {
        guard let project = projectStore.projects.first(where: { $0.id == projectID }) else {
            throw RemoteVCSError.projectNotFound
        }
        return resolveWorktreePath(projectID: projectID) ?? project.path
    }

    private func notifyRepoDidChange(repoPath: String) {
        NotificationCenter.default.post(
            name: .vcsRepoDidChange,
            object: nil,
            userInfo: ["repoPath": repoPath]
        )
    }

    private static func toFileDTO(_ file: GitStatusFile, staged: Bool) -> GitFileDTO {
        let statusChar = staged ? file.xStatus : file.yStatus
        let isUntracked = file.xStatus == "?" && file.yStatus == "?"
        let status: GitFileStatusDTO = if isUntracked {
            .untracked
        } else {
            switch statusChar {
            case "A": .added
            case "M": .modified
            case "D": .deleted
            case "R": .renamed
            case "C": .copied
            case "U": .unmerged
            case "?": .untracked
            default: .modified
            }
        }
        return GitFileDTO(path: file.path, status: status, isUntracked: isUntracked)
    }

    private static func worktreeSlug(from name: String) -> String {
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "._-"))
        let scalars = name.unicodeScalars.map { allowed.contains($0) ? Character($0) : "-" }
        let collapsed = String(scalars)
            .split(separator: "-", omittingEmptySubsequences: true)
            .joined(separator: "-")
        return collapsed.isEmpty ? UUID().uuidString : collapsed
    }

    enum RemoteVCSError: LocalizedError {
        case projectNotFound
        case worktreeNotFound
        case invalidInput(String)

        var errorDescription: String? {
            switch self {
            case .projectNotFound: "Project not found."
            case .worktreeNotFound: "Worktree not found."
            case let .invalidInput(message): message
            }
        }
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
