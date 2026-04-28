import Foundation
import MuxyShared

@MainActor
final class DemoBackend {
    static let seededDeviceName = "Demo Mac"
    static let seededDeviceHost = "192.168.1.42"
    static let seededDevicePort: UInt16 = 4865

    static let theme = ConnectionManager.DeviceTheme(
        fg: 0xC9C2D9,
        bg: 0x19171F,
        palette: [
            0x141219, 0xEC4899, 0x34D399, 0xE0AF68,
            0xC370D3, 0x6366F1, 0x22D3EE, 0xA9B1D6,
            0x2E2B34, 0xF472B6, 0x6EE7B7, 0xFBBF24,
            0xD99BE5, 0x818CF8, 0x67E8F9, 0xC9C2D9,
        ]
    )

    static let myClientID = UUID(uuidString: "00000000-0000-0000-0000-00000000C11D")!

    weak var owner: ConnectionManager?

    private(set) var savedDevices: [ConnectionManager.SavedDevice]
    private(set) var projects: [ProjectDTO]
    private var workspaces: [UUID: WorkspaceDTO]
    private var worktreesByProject: [UUID: [WorktreeDTO]]
    private var statusByProject: [UUID: VCSStatusDTO]
    private var branchesByProject: [UUID: VCSBranchesDTO]

    init() {
        let seedDevice = ConnectionManager.SavedDevice(
            name: Self.seededDeviceName,
            host: Self.seededDeviceHost,
            port: Self.seededDevicePort
        )
        savedDevices = [seedDevice]

        let muxyID = UUID(uuidString: "11111111-1111-1111-1111-111111111111")!
        let webID = UUID(uuidString: "22222222-2222-2222-2222-222222222222")!
        let now = Date()

        projects = [
            ProjectDTO(
                id: muxyID,
                name: "muxy",
                path: "/Users/demo/Projects/muxy",
                sortOrder: 0,
                createdAt: now,
                icon: "terminal",
                logo: nil,
                iconColor: "blue"
            ),
            ProjectDTO(
                id: webID,
                name: "web-app",
                path: "/Users/demo/Projects/web-app",
                sortOrder: 1,
                createdAt: now,
                icon: "globe",
                logo: nil,
                iconColor: "green"
            ),
        ]

        let muxyMainWT = WorktreeDTO(
            id: UUID(uuidString: "AAAA0001-0000-0000-0000-000000000001")!,
            name: "main",
            path: "/Users/demo/Projects/muxy",
            branch: "main",
            isPrimary: true,
            createdAt: now
        )
        let muxyFeatureWT = WorktreeDTO(
            id: UUID(uuidString: "AAAA0001-0000-0000-0000-000000000002")!,
            name: "feature-search",
            path: "/Users/demo/Projects/muxy-worktrees/feature-search",
            branch: "feature/search",
            isPrimary: false,
            createdAt: now
        )
        let webMainWT = WorktreeDTO(
            id: UUID(uuidString: "BBBB0001-0000-0000-0000-000000000001")!,
            name: "main",
            path: "/Users/demo/Projects/web-app",
            branch: "main",
            isPrimary: true,
            createdAt: now
        )

        worktreesByProject = [
            muxyID: [muxyMainWT, muxyFeatureWT],
            webID: [webMainWT],
        ]

        workspaces = [
            muxyID: Self.makeWorkspace(projectID: muxyID, worktreeID: muxyMainWT.id, projectPath: "/Users/demo/Projects/muxy"),
            webID: Self.makeWorkspace(projectID: webID, worktreeID: webMainWT.id, projectPath: "/Users/demo/Projects/web-app"),
        ]

        statusByProject = [
            muxyID: VCSStatusDTO(
                branch: "main",
                aheadCount: 1,
                behindCount: 1,
                hasUpstream: true,
                stagedFiles: [
                    GitFileDTO(path: "MuxyMobile/SettingsSheet.swift", status: .modified),
                ],
                changedFiles: [
                    GitFileDTO(path: "MuxyMobile/ConnectionManager.swift", status: .modified),
                    GitFileDTO(path: "README.md", status: .modified),
                    GitFileDTO(path: "docs/demo.md", status: .untracked, isUntracked: true),
                ],
                defaultBranch: "main",
                pullRequest: nil
            ),
            webID: VCSStatusDTO(
                branch: "main",
                aheadCount: 0,
                behindCount: 0,
                hasUpstream: true,
                stagedFiles: [],
                changedFiles: [],
                defaultBranch: "main",
                pullRequest: nil
            ),
        ]

        branchesByProject = [
            muxyID: VCSBranchesDTO(
                current: "main",
                locals: ["main", "feature/search", "fix/scrolling"],
                defaultBranch: "main"
            ),
            webID: VCSBranchesDTO(
                current: "main",
                locals: ["main"],
                defaultBranch: "main"
            ),
        ]
    }

    private static func makeWorkspace(projectID: UUID, worktreeID: UUID, projectPath: String) -> WorkspaceDTO {
        let areaID = UUID()
        let tab1 = TabDTO(id: UUID(), kind: .terminal, title: "zsh", isPinned: false, paneID: UUID())
        let tab2 = TabDTO(id: UUID(), kind: .terminal, title: "server", isPinned: false, paneID: UUID())
        let area = TabAreaDTO(id: areaID, projectPath: projectPath, tabs: [tab1, tab2], activeTabID: tab1.id)
        return WorkspaceDTO(projectID: projectID, worktreeID: worktreeID, focusedAreaID: areaID, root: .tabArea(area))
    }

    func addDevice(name: String, host: String, port: UInt16) {
        savedDevices.removeAll { $0.host == host && $0.port == port }
        savedDevices.insert(ConnectionManager.SavedDevice(name: name, host: host, port: port), at: 0)
    }

    func removeDevice(_ device: ConnectionManager.SavedDevice) {
        savedDevices.removeAll { $0.id == device.id }
    }

    func worktrees(for projectID: UUID) -> [WorktreeDTO] {
        worktreesByProject[projectID] ?? []
    }

    func simulatedDelay(for method: MuxyMethod) -> Duration {
        switch method {
        case .vcsPush,
             .vcsPull,
             .vcsCommit,
             .vcsCreatePR,
             .vcsAddWorktree,
             .vcsRemoveWorktree,
             .vcsSwitchBranch:
            .milliseconds(700)
        case .vcsCreateBranch,
             .vcsStageFiles,
             .vcsUnstageFiles,
             .vcsDiscardFiles:
            .milliseconds(250)
        default:
            .zero
        }
    }

    func handle(_ method: MuxyMethod, params: MuxyParams?) -> MuxyResponse {
        let id = UUID().uuidString
        switch method {
        case .listProjects:
            return MuxyResponse(id: id, result: .projects(projects))

        case .selectProject:
            guard case let .selectProject(p) = params, workspaces[p.projectID] != nil else {
                return MuxyResponse(id: id, error: .notFound)
            }
            return MuxyResponse(id: id, result: .ok)

        case .listWorktrees:
            guard case let .listWorktrees(p) = params else {
                return MuxyResponse(id: id, error: .invalidParams)
            }
            return MuxyResponse(id: id, result: .worktrees(worktrees(for: p.projectID)))

        case .getWorkspace:
            guard case let .getWorkspace(p) = params, let ws = workspaces[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            return MuxyResponse(id: id, result: .workspace(ws))

        case .takeOverPane:
            guard case let .takeOverPane(p) = params else {
                return MuxyResponse(id: id, error: .invalidParams)
            }
            owner?.paneOwners[p.paneID] = .remote(deviceID: Self.myClientID, deviceName: "iPhone (Demo)")
            scheduleTerminalGreeting(paneID: p.paneID)
            return MuxyResponse(id: id, result: .ok)

        case .releasePane:
            guard case let .releasePane(p) = params else {
                return MuxyResponse(id: id, error: .invalidParams)
            }
            owner?.paneOwners[p.paneID] = .mac(deviceName: Self.seededDeviceName)
            return MuxyResponse(id: id, result: .ok)

        case .terminalResize,
             .terminalScroll:
            return MuxyResponse(id: id, result: .ok)

        case .createTab:
            guard case let .createTab(p) = params, let ws = workspaces[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            workspaces[p.projectID] = appendTab(to: ws)
            return MuxyResponse(id: id, result: .ok)

        case .closeTab:
            guard case let .closeTab(p) = params, let ws = workspaces[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            if let updated = removeTab(from: ws, areaID: p.areaID, tabID: p.tabID) {
                workspaces[p.projectID] = updated
            }
            return MuxyResponse(id: id, result: .ok)

        case .selectTab:
            guard case let .selectTab(p) = params, let ws = workspaces[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            if let updated = selectTab(in: ws, areaID: p.areaID, tabID: p.tabID) {
                workspaces[p.projectID] = updated
            }
            return MuxyResponse(id: id, result: .ok)

        case .getVCSStatus:
            guard case let .getVCSStatus(p) = params, let status = statusByProject[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            return MuxyResponse(id: id, result: .vcsStatus(status))

        case .vcsListBranches:
            guard case let .vcsListBranches(p) = params, let branches = branchesByProject[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            return MuxyResponse(id: id, result: .vcsBranches(branches))

        case .vcsSwitchBranch:
            guard case let .vcsSwitchBranch(p) = params, let current = branchesByProject[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            branchesByProject[p.projectID] = VCSBranchesDTO(
                current: p.branch,
                locals: current.locals,
                defaultBranch: current.defaultBranch
            )
            if var status = statusByProject[p.projectID] {
                status = VCSStatusDTO(
                    branch: p.branch,
                    aheadCount: 0,
                    behindCount: 0,
                    hasUpstream: status.hasUpstream,
                    stagedFiles: status.stagedFiles,
                    changedFiles: status.changedFiles,
                    defaultBranch: status.defaultBranch,
                    pullRequest: nil
                )
                statusByProject[p.projectID] = status
            }
            return MuxyResponse(id: id, result: .ok)

        case .vcsCreateBranch:
            guard case let .vcsCreateBranch(p) = params, let current = branchesByProject[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            var locals = current.locals
            if !locals.contains(p.name) { locals.append(p.name) }
            branchesByProject[p.projectID] = VCSBranchesDTO(
                current: p.name,
                locals: locals,
                defaultBranch: current.defaultBranch
            )
            return MuxyResponse(id: id, result: .ok)

        case .vcsStageFiles:
            guard case let .vcsStageFiles(p) = params, var status = statusByProject[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            let moving = status.changedFiles.filter { p.paths.contains($0.path) }
            let staged = status.stagedFiles + moving
            let remaining = status.changedFiles.filter { !p.paths.contains($0.path) }
            status = VCSStatusDTO(
                branch: status.branch, aheadCount: status.aheadCount, behindCount: status.behindCount,
                hasUpstream: status.hasUpstream, stagedFiles: staged, changedFiles: remaining,
                defaultBranch: status.defaultBranch, pullRequest: status.pullRequest
            )
            statusByProject[p.projectID] = status
            return MuxyResponse(id: id, result: .ok)

        case .vcsUnstageFiles:
            guard case let .vcsUnstageFiles(p) = params, var status = statusByProject[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            let moving = status.stagedFiles.filter { p.paths.contains($0.path) }
            let unstaged = status.stagedFiles.filter { !p.paths.contains($0.path) }
            let changed = status.changedFiles + moving
            status = VCSStatusDTO(
                branch: status.branch, aheadCount: status.aheadCount, behindCount: status.behindCount,
                hasUpstream: status.hasUpstream, stagedFiles: unstaged, changedFiles: changed,
                defaultBranch: status.defaultBranch, pullRequest: status.pullRequest
            )
            statusByProject[p.projectID] = status
            return MuxyResponse(id: id, result: .ok)

        case .vcsDiscardFiles:
            guard case let .vcsDiscardFiles(p) = params, var status = statusByProject[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            let kept = status.changedFiles.filter { !p.paths.contains($0.path) && !p.untrackedPaths.contains($0.path) }
            status = VCSStatusDTO(
                branch: status.branch, aheadCount: status.aheadCount, behindCount: status.behindCount,
                hasUpstream: status.hasUpstream, stagedFiles: status.stagedFiles, changedFiles: kept,
                defaultBranch: status.defaultBranch, pullRequest: status.pullRequest
            )
            statusByProject[p.projectID] = status
            return MuxyResponse(id: id, result: .ok)

        case .vcsCommit:
            guard case let .vcsCommit(p) = params, var status = statusByProject[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            let stagedAll = p.stageAll ? status.stagedFiles + status.changedFiles : status.stagedFiles
            let changed = p.stageAll ? [] : status.changedFiles
            let _ = stagedAll
            status = VCSStatusDTO(
                branch: status.branch, aheadCount: status.aheadCount + 1, behindCount: status.behindCount,
                hasUpstream: status.hasUpstream, stagedFiles: [], changedFiles: changed,
                defaultBranch: status.defaultBranch, pullRequest: status.pullRequest
            )
            statusByProject[p.projectID] = status
            return MuxyResponse(id: id, result: .ok)

        case .vcsPush:
            guard case let .vcsPush(p) = params, var status = statusByProject[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            status = VCSStatusDTO(
                branch: status.branch, aheadCount: 0, behindCount: status.behindCount,
                hasUpstream: true, stagedFiles: status.stagedFiles, changedFiles: status.changedFiles,
                defaultBranch: status.defaultBranch, pullRequest: status.pullRequest
            )
            statusByProject[p.projectID] = status
            return MuxyResponse(id: id, result: .ok)

        case .vcsPull:
            guard case let .vcsPull(p) = params, var status = statusByProject[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            status = VCSStatusDTO(
                branch: status.branch, aheadCount: status.aheadCount, behindCount: 0,
                hasUpstream: status.hasUpstream, stagedFiles: status.stagedFiles, changedFiles: status.changedFiles,
                defaultBranch: status.defaultBranch, pullRequest: status.pullRequest
            )
            statusByProject[p.projectID] = status
            return MuxyResponse(id: id, result: .ok)

        case .vcsCreatePR:
            guard case let .vcsCreatePR(p) = params, var status = statusByProject[p.projectID] else {
                return MuxyResponse(id: id, error: .notFound)
            }
            let pr = VCSPullRequestDTO(
                url: "https://github.com/muxy-app/demo/pull/42",
                number: 42,
                state: "open",
                isDraft: p.draft,
                baseBranch: p.baseBranch ?? "main"
            )
            status = VCSStatusDTO(
                branch: status.branch, aheadCount: status.aheadCount, behindCount: status.behindCount,
                hasUpstream: status.hasUpstream, stagedFiles: status.stagedFiles, changedFiles: status.changedFiles,
                defaultBranch: status.defaultBranch, pullRequest: pr
            )
            statusByProject[p.projectID] = status
            return MuxyResponse(id: id, result: .vcsPRCreated(VCSCreatePRResultDTO(url: pr.url, number: pr.number)))

        case .vcsAddWorktree:
            guard case let .vcsAddWorktree(p) = params else {
                return MuxyResponse(id: id, error: .invalidParams)
            }
            let wt = WorktreeDTO(
                id: UUID(),
                name: p.name,
                path: "/Users/demo/Projects/\(p.name)",
                branch: p.branch,
                isPrimary: false,
                createdAt: Date()
            )
            worktreesByProject[p.projectID, default: []].append(wt)
            if p.createBranch, var branches = branchesByProject[p.projectID], !branches.locals.contains(p.branch) {
                branches = VCSBranchesDTO(
                    current: branches.current,
                    locals: branches.locals + [p.branch],
                    defaultBranch: branches.defaultBranch
                )
                branchesByProject[p.projectID] = branches
            }
            return MuxyResponse(id: id, result: .ok)

        case .vcsRemoveWorktree:
            guard case let .vcsRemoveWorktree(p) = params else {
                return MuxyResponse(id: id, error: .invalidParams)
            }
            worktreesByProject[p.projectID]?.removeAll { $0.id == p.worktreeID }
            return MuxyResponse(id: id, result: .ok)

        case .selectWorktree:
            return MuxyResponse(id: id, result: .ok)

        case .getProjectLogo:
            return MuxyResponse(id: id, error: .notFound)

        case .listNotifications:
            return MuxyResponse(id: id, result: .notifications([]))

        case .markNotificationRead,
             .subscribe,
             .unsubscribe:
            return MuxyResponse(id: id, result: .ok)

        case .splitArea,
             .closeArea,
             .focusArea:
            return MuxyResponse(id: id, result: .ok)

        case .getTerminalContent:
            return MuxyResponse(id: id, error: .notFound)

        case .registerDevice,
             .pairDevice,
             .authenticateDevice,
             .terminalInput:
            return MuxyResponse(id: id, error: .invalidParams)
        }
    }

    func handleTerminalInput(paneID: UUID, bytes: Data) {
        guard let owner else { return }
        guard let handler = owner.terminalByteHandler(for: paneID) else { return }

        let containsEnter = bytes.contains(0x0D) || bytes.contains(0x0A)
        if containsEnter {
            let echo = stripControlBytes(bytes)
            var response = Data()
            response.append(echo)
            response.append(contentsOf: [0x0D, 0x0A])
            response.append(Self.demoNoticeBytes)
            response.append(Self.promptBytes)
            handler(response)
            return
        }
        handler(bytes)
    }

    private func scheduleTerminalGreeting(paneID: UUID) {
        Task { @MainActor [weak owner] in
            try? await Task.sleep(for: .milliseconds(150))
            guard let handler = owner?.terminalByteHandler(for: paneID) else { return }
            handler(Self.greetingBytes)
        }
    }

    private func stripControlBytes(_ bytes: Data) -> Data {
        Data(bytes.filter { $0 != 0x0D && $0 != 0x0A })
    }

    private func appendTab(to ws: WorkspaceDTO) -> WorkspaceDTO {
        let updatedRoot = appendTab(in: ws.root, focusedAreaID: ws.focusedAreaID)
        return WorkspaceDTO(
            projectID: ws.projectID,
            worktreeID: ws.worktreeID,
            focusedAreaID: ws.focusedAreaID,
            root: updatedRoot
        )
    }

    private func appendTab(in node: SplitNodeDTO, focusedAreaID: UUID?) -> SplitNodeDTO {
        switch node {
        case let .tabArea(area):
            guard focusedAreaID == nil || focusedAreaID == area.id else { return node }
            let newTab = TabDTO(id: UUID(), kind: .terminal, title: "zsh", isPinned: false, paneID: UUID())
            let updated = TabAreaDTO(
                id: area.id,
                projectPath: area.projectPath,
                tabs: area.tabs + [newTab],
                activeTabID: newTab.id
            )
            return .tabArea(updated)
        case let .split(branch):
            return .split(SplitBranchDTO(
                id: branch.id,
                direction: branch.direction,
                ratio: branch.ratio,
                first: appendTab(in: branch.first, focusedAreaID: focusedAreaID),
                second: appendTab(in: branch.second, focusedAreaID: focusedAreaID)
            ))
        }
    }

    private func removeTab(from ws: WorkspaceDTO, areaID: UUID, tabID: UUID) -> WorkspaceDTO? {
        guard let updated = removeTab(in: ws.root, areaID: areaID, tabID: tabID) else { return nil }
        return WorkspaceDTO(
            projectID: ws.projectID,
            worktreeID: ws.worktreeID,
            focusedAreaID: ws.focusedAreaID,
            root: updated
        )
    }

    private func removeTab(in node: SplitNodeDTO, areaID: UUID, tabID: UUID) -> SplitNodeDTO? {
        switch node {
        case let .tabArea(area):
            guard area.id == areaID else { return node }
            let remaining = area.tabs.filter { $0.id != tabID }
            let active = area.activeTabID == tabID ? remaining.first?.id : area.activeTabID
            return .tabArea(TabAreaDTO(
                id: area.id,
                projectPath: area.projectPath,
                tabs: remaining,
                activeTabID: active
            ))
        case let .split(branch):
            return .split(SplitBranchDTO(
                id: branch.id,
                direction: branch.direction,
                ratio: branch.ratio,
                first: removeTab(in: branch.first, areaID: areaID, tabID: tabID) ?? branch.first,
                second: removeTab(in: branch.second, areaID: areaID, tabID: tabID) ?? branch.second
            ))
        }
    }

    private func selectTab(in ws: WorkspaceDTO, areaID: UUID, tabID: UUID) -> WorkspaceDTO? {
        let updated = selectTab(in: ws.root, areaID: areaID, tabID: tabID)
        return WorkspaceDTO(
            projectID: ws.projectID,
            worktreeID: ws.worktreeID,
            focusedAreaID: ws.focusedAreaID,
            root: updated
        )
    }

    private func selectTab(in node: SplitNodeDTO, areaID: UUID, tabID: UUID) -> SplitNodeDTO {
        switch node {
        case let .tabArea(area):
            guard area.id == areaID else { return node }
            return .tabArea(TabAreaDTO(
                id: area.id,
                projectPath: area.projectPath,
                tabs: area.tabs,
                activeTabID: tabID
            ))
        case let .split(branch):
            return .split(SplitBranchDTO(
                id: branch.id,
                direction: branch.direction,
                ratio: branch.ratio,
                first: selectTab(in: branch.first, areaID: areaID, tabID: tabID),
                second: selectTab(in: branch.second, areaID: areaID, tabID: tabID)
            ))
        }
    }

    private static let greetingBytes: Data = {
        let text = "\u{1B}[1;32mDemo Mode\u{1B}[0m — this terminal is simulated.\r\n" +
            "Type any command and press Enter to see the demo response.\r\n" +
            "demo@muxy ~ % "
        return Data(text.utf8)
    }()

    private static let demoNoticeBytes: Data = {
        let text = "\u{1B}[33m[Demo Mode]\u{1B}[0m Commands are not executed in demo mode.\r\n"
        return Data(text.utf8)
    }()

    private static let promptBytes = Data("demo@muxy ~ % ".utf8)
}
