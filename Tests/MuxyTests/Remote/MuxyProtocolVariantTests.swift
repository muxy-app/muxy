import Foundation
import MuxyShared
import Testing

@Suite("MuxyProtocol variants")
struct MuxyProtocolVariantTests {
    private let projectID = UUID(uuidString: "00000000-0000-0000-0000-000000000001")!
    private let worktreeID = UUID(uuidString: "00000000-0000-0000-0000-000000000002")!
    private let areaID = UUID(uuidString: "00000000-0000-0000-0000-000000000003")!
    private let tabID = UUID(uuidString: "00000000-0000-0000-0000-000000000004")!
    private let paneID = UUID(uuidString: "00000000-0000-0000-0000-000000000005")!
    private let deviceID = UUID(uuidString: "00000000-0000-0000-0000-000000000006")!

    @Test("params encode and decode every protocol case", arguments: MuxyProtocolVariantTests.paramSamples())
    func paramsRoundTrip(sample: ProtocolSample<MuxyParams>) throws {
        let decoded = try roundTrip(sample.value, as: MuxyParams.self)
        #expect(typeName(decoded) == sample.caseName)
    }

    @Test("results encode and decode every protocol case", arguments: MuxyProtocolVariantTests.resultSamples())
    func resultsRoundTrip(sample: ProtocolSample<MuxyResult>) throws {
        let decoded = try roundTrip(sample.value, as: MuxyResult.self)
        #expect(typeName(decoded) == sample.caseName)
    }

    @Test("event data encodes and decodes every protocol case", arguments: MuxyProtocolVariantTests.eventSamples())
    func eventsRoundTrip(sample: ProtocolSample<MuxyEventData>) throws {
        let decoded = try roundTrip(sample.value, as: MuxyEventData.self)
        #expect(typeName(decoded) == sample.caseName)
    }

    @Test("unknown result and event data types reject decoding")
    func unknownProtocolTypesFail() {
        #expect(throws: DecodingError.self) {
            _ = try JSONDecoder().decode(MuxyResult.self, from: Data(#"{"type":"missing","value":{}}"#.utf8))
        }
        #expect(throws: DecodingError.self) {
            _ = try JSONDecoder().decode(MuxyEventData.self, from: Data(#"{"type":"missing","value":{}}"#.utf8))
        }
    }

    @Test("pane owner display name uses local and remote names")
    func paneOwnerDisplayName() {
        #expect(PaneOwnerDTO.mac(deviceName: "Mac").displayName == "Mac")
        #expect(PaneOwnerDTO.remote(deviceID: deviceID, deviceName: "iPad").displayName == "iPad")
    }

    @Test("terminal cell flags remain stable bit masks")
    func terminalCellFlags() {
        #expect(TerminalCellFlag.bold == 1)
        #expect(TerminalCellFlag.italic == 2)
        #expect(TerminalCellFlag.faint == 4)
        #expect(TerminalCellFlag.blink == 8)
        #expect(TerminalCellFlag.inverse == 16)
        #expect(TerminalCellFlag.invisible == 32)
        #expect(TerminalCellFlag.strike == 64)
        #expect(TerminalCellFlag.underline == 128)
        #expect(TerminalCellFlag.overline == 256)
        #expect(TerminalCellFlag.wide == 512)
        #expect(TerminalCellFlag.spacer == 1024)
    }

    @Test("VCS DTO defaults preserve backwards compatible values")
    func vcsDefaults() {
        let file = GitFileDTO(path: "Sources/App.swift", status: .modified)
        let pr = VCSPullRequestDTO(url: "https://example.test/pull/7", number: 7, state: "OPEN", isDraft: false, baseBranch: "main")

        #expect(file.id == "Sources/App.swift")
        #expect(file.isUntracked == false)
        #expect(pr.mergeable == nil)
        #expect(pr.mergeStateStatus == "UNKNOWN")
        #expect(pr.checks.status == "none")
        #expect(pr.checks.total == 0)
    }

    private static func paramSamples() -> [ProtocolSample<MuxyParams>] {
        let ids = IDs()
        return [
            ProtocolSample(.selectProject(SelectProjectParams(projectID: ids.projectID)), caseName: ".selectProject"),
            ProtocolSample(.listWorktrees(ListWorktreesParams(projectID: ids.projectID)), caseName: ".listWorktrees"),
            ProtocolSample(.selectWorktree(SelectWorktreeParams(projectID: ids.projectID, worktreeID: ids.worktreeID)), caseName: ".selectWorktree"),
            ProtocolSample(.getWorkspace(GetWorkspaceParams(projectID: ids.projectID)), caseName: ".getWorkspace"),
            ProtocolSample(.createTab(CreateTabParams(projectID: ids.projectID, areaID: ids.areaID, kind: .editor)), caseName: ".createTab"),
            ProtocolSample(.closeTab(CloseTabParams(projectID: ids.projectID, areaID: ids.areaID, tabID: ids.tabID)), caseName: ".closeTab"),
            ProtocolSample(.selectTab(SelectTabParams(projectID: ids.projectID, areaID: ids.areaID, tabID: ids.tabID)), caseName: ".selectTab"),
            ProtocolSample(.splitArea(SplitAreaParams(projectID: ids.projectID, areaID: ids.areaID, direction: .horizontal, position: .second)), caseName: ".splitArea"),
            ProtocolSample(.closeArea(CloseAreaParams(projectID: ids.projectID, areaID: ids.areaID)), caseName: ".closeArea"),
            ProtocolSample(.focusArea(FocusAreaParams(projectID: ids.projectID, areaID: ids.areaID)), caseName: ".focusArea"),
            ProtocolSample(.terminalInput(TerminalInputParams(paneID: ids.paneID, bytes: Data([1, 2, 3]))), caseName: ".terminalInput"),
            ProtocolSample(.terminalResize(TerminalResizeParams(paneID: ids.paneID, cols: 120, rows: 40)), caseName: ".terminalResize"),
            ProtocolSample(.terminalScroll(TerminalScrollParams(paneID: ids.paneID, deltaX: 1.5, deltaY: -3.25, precise: true)), caseName: ".terminalScroll"),
            ProtocolSample(.getTerminalContent(GetTerminalContentParams(paneID: ids.paneID)), caseName: ".getTerminalContent"),
            ProtocolSample(.registerDevice(RegisterDeviceParams(deviceName: "iPhone")), caseName: ".registerDevice"),
            ProtocolSample(.pairDevice(PairDeviceParams(deviceID: ids.deviceID, deviceName: "iPhone", token: "token")), caseName: ".pairDevice"),
            ProtocolSample(.authenticateDevice(AuthenticateDeviceParams(deviceID: ids.deviceID, deviceName: "iPhone", token: "token")), caseName: ".authenticateDevice"),
            ProtocolSample(.takeOverPane(TakeOverPaneParams(paneID: ids.paneID, cols: 80, rows: 24)), caseName: ".takeOverPane"),
            ProtocolSample(.releasePane(ReleasePaneParams(paneID: ids.paneID)), caseName: ".releasePane"),
            ProtocolSample(.getVCSStatus(GetVCSStatusParams(projectID: ids.projectID)), caseName: ".getVCSStatus"),
            ProtocolSample(.vcsRefresh(VCSRefreshParams(projectID: ids.projectID)), caseName: ".vcsRefresh"),
            ProtocolSample(.vcsCommit(VCSCommitParams(projectID: ids.projectID, message: "Commit", stageAll: true)), caseName: ".vcsCommit"),
            ProtocolSample(.vcsPush(VCSPushParams(projectID: ids.projectID)), caseName: ".vcsPush"),
            ProtocolSample(.vcsPull(VCSPullParams(projectID: ids.projectID)), caseName: ".vcsPull"),
            ProtocolSample(.vcsStageFiles(VCSStageFilesParams(projectID: ids.projectID, paths: ["a.swift"])), caseName: ".vcsStageFiles"),
            ProtocolSample(.vcsUnstageFiles(VCSUnstageFilesParams(projectID: ids.projectID, paths: ["a.swift"])), caseName: ".vcsUnstageFiles"),
            ProtocolSample(.vcsDiscardFiles(VCSDiscardFilesParams(projectID: ids.projectID, paths: ["a.swift"], untrackedPaths: ["b.swift"])), caseName: ".vcsDiscardFiles"),
            ProtocolSample(.vcsListBranches(VCSListBranchesParams(projectID: ids.projectID)), caseName: ".vcsListBranches"),
            ProtocolSample(.vcsSwitchBranch(VCSSwitchBranchParams(projectID: ids.projectID, branch: "main")), caseName: ".vcsSwitchBranch"),
            ProtocolSample(.vcsCreateBranch(VCSCreateBranchParams(projectID: ids.projectID, name: "feature")), caseName: ".vcsCreateBranch"),
            ProtocolSample(.vcsCreatePR(VCSCreatePRParams(projectID: ids.projectID, title: "Title", body: "Body", baseBranch: "main", draft: false)), caseName: ".vcsCreatePR"),
            ProtocolSample(.vcsMergePullRequest(VCSMergePullRequestParams(projectID: ids.projectID, number: 3, method: .squash, deleteBranch: true)), caseName: ".vcsMergePullRequest"),
            ProtocolSample(.vcsAddWorktree(VCSAddWorktreeParams(projectID: ids.projectID, name: "Feature", branch: "feature", createBranch: true, baseBranch: "main")), caseName: ".vcsAddWorktree"),
            ProtocolSample(.vcsRemoveWorktree(VCSRemoveWorktreeParams(projectID: ids.projectID, worktreeID: ids.worktreeID)), caseName: ".vcsRemoveWorktree"),
            ProtocolSample(.vcsGetDiff(VCSGetDiffParams(projectID: ids.projectID, filePath: "a.swift", forceFull: true)), caseName: ".vcsGetDiff"),
            ProtocolSample(.getProjectLogo(GetProjectLogoParams(projectID: ids.projectID)), caseName: ".getProjectLogo"),
            ProtocolSample(.markNotificationRead(MarkNotificationReadParams(notificationID: ids.notificationID)), caseName: ".markNotificationRead"),
            ProtocolSample(.subscribe(SubscribeParams(events: [.workspaceChanged, .themeChanged])), caseName: ".subscribe"),
            ProtocolSample(.unsubscribe(UnsubscribeParams(events: [.terminalOutput])), caseName: ".unsubscribe"),
        ]
    }

    private static func resultSamples() -> [ProtocolSample<MuxyResult>] {
        let ids = IDs()
        return [
            ProtocolSample(.projects([project(ids)]), caseName: ".projects"),
            ProtocolSample(.worktrees([WorktreeDTO(id: ids.worktreeID, name: "Main", path: "/repo", branch: "main", isPrimary: true, createdAt: Date(timeIntervalSince1970: 3))]), caseName: ".worktrees"),
            ProtocolSample(.workspace(workspace(ids)), caseName: ".workspace"),
            ProtocolSample(.tab(tab(ids)), caseName: ".tab"),
            ProtocolSample(.terminalContent(TerminalContentDTO(paneID: ids.paneID, content: "hello", cols: 80, rows: 24)), caseName: ".terminalContent"),
            ProtocolSample(.terminalCells(terminalCells(ids)), caseName: ".terminalCells"),
            ProtocolSample(.deviceInfo(DeviceInfoDTO(clientID: ids.deviceID, deviceName: "iPad", themeFg: 1, themeBg: 2, themePalette: [1, 2])), caseName: ".deviceInfo"),
            ProtocolSample(.pairing(PairingResultDTO(clientID: ids.deviceID, deviceName: "iPad", themeFg: 1, themeBg: 2, themePalette: [3])), caseName: ".pairing"),
            ProtocolSample(.paneOwner(.mac(deviceName: "Mac")), caseName: ".paneOwner"),
            ProtocolSample(.vcsStatus(vcsStatus), caseName: ".vcsStatus"),
            ProtocolSample(.vcsBranches(VCSBranchesDTO(current: "main", locals: ["main"], defaultBranch: "main")), caseName: ".vcsBranches"),
            ProtocolSample(.vcsPRCreated(VCSCreatePRResultDTO(url: "https://example.test/pull/1", number: 1)), caseName: ".vcsPRCreated"),
            ProtocolSample(.vcsDiff(diff), caseName: ".vcsDiff"),
            ProtocolSample(.projectLogo(ProjectLogoDTO(projectID: ids.projectID, pngData: "base64")), caseName: ".projectLogo"),
            ProtocolSample(.notifications([notification(ids)]), caseName: ".notifications"),
            ProtocolSample(.ok, caseName: ".ok"),
        ]
    }

    private static func eventSamples() -> [ProtocolSample<MuxyEventData>] {
        let ids = IDs()
        return [
            ProtocolSample(.workspace(workspace(ids)), caseName: ".workspace"),
            ProtocolSample(.terminalOutput(TerminalOutputEventDTO(paneID: ids.paneID, bytes: Data([65]))), caseName: ".terminalOutput"),
            ProtocolSample(.terminalSnapshot(TerminalOutputEventDTO(paneID: ids.paneID, bytes: Data([66]))), caseName: ".terminalSnapshot"),
            ProtocolSample(.notification(notification(ids)), caseName: ".notification"),
            ProtocolSample(.projects([project(ids)]), caseName: ".projects"),
            ProtocolSample(.paneOwnership(PaneOwnershipEventDTO(paneID: ids.paneID, owner: .remote(deviceID: ids.deviceID, deviceName: "iPad"))), caseName: ".paneOwnership"),
            ProtocolSample(.deviceTheme(DeviceThemeEventDTO(fg: 1, bg: 2, palette: [1, 2, 3])), caseName: ".deviceTheme"),
        ]
    }

    private static func project(_ ids: IDs) -> ProjectDTO {
        ProjectDTO(id: ids.projectID, name: "Muxy", path: "/repo", sortOrder: 1, createdAt: Date(timeIntervalSince1970: 1), icon: "terminal", logo: nil, iconColor: "blue", preferredWorktreeParentPath: "/worktrees")
    }

    private static func workspace(_ ids: IDs) -> WorkspaceDTO {
        WorkspaceDTO(projectID: ids.projectID, worktreeID: ids.worktreeID, focusedAreaID: ids.areaID, root: .tabArea(tabArea(ids)))
    }

    private static func tabArea(_ ids: IDs) -> TabAreaDTO {
        TabAreaDTO(id: ids.areaID, projectPath: "/repo", tabs: [tab(ids)], activeTabID: ids.tabID)
    }

    private static func tab(_ ids: IDs) -> TabDTO {
        TabDTO(id: ids.tabID, kind: .terminal, title: "Shell", isPinned: false, paneID: ids.paneID)
    }

    private static func terminalCells(_ ids: IDs) -> TerminalCellsDTO {
        TerminalCellsDTO(paneID: ids.paneID, cols: 1, rows: 1, cursorX: 0, cursorY: 0, cursorVisible: true, defaultFg: 0xFFFFFF, defaultBg: 0, cells: [
            TerminalCellDTO(codepoint: 65, fg: 0xFFFFFF, bg: 0, flags: TerminalCellFlag.bold),
        ], altScreen: true, cursorKeys: true, bracketedPaste: true, focusEvent: true, mouseEvent: 1, mouseFormat: 2)
    }

    private static var vcsStatus: VCSStatusDTO {
        VCSStatusDTO(branch: "main", aheadCount: 1, behindCount: 2, hasUpstream: true, stagedFiles: [
            GitFileDTO(path: "a.swift", status: .added),
        ], changedFiles: [
            GitFileDTO(path: "b.swift", status: .untracked, isUntracked: true),
        ], defaultBranch: "main", pullRequest: VCSPullRequestDTO(url: "https://example.test/pull/1", number: 1, state: "OPEN", isDraft: true, baseBranch: "main", mergeable: true, mergeStateStatus: "CLEAN", checks: VCSPRChecksDTO(status: "passing", passing: 1, failing: 0, pending: 0, total: 1)))
    }

    private static var diff: VCSDiffDTO {
        VCSDiffDTO(filePath: "a.swift", rows: [
            VCSDiffRowDTO(kind: .addition, oldLineNumber: nil, newLineNumber: 1, oldText: nil, newText: "let a = 1", text: "+let a = 1"),
        ], additions: 1, deletions: 0, truncated: false, isBinary: false)
    }

    private static func notification(_ ids: IDs) -> NotificationDTO {
        NotificationDTO(id: ids.notificationID, paneID: ids.paneID, projectID: ids.projectID, worktreeID: ids.worktreeID, areaID: ids.areaID, tabID: ids.tabID, source: .aiProvider("codex"), title: "Done", body: "Task complete", timestamp: Date(timeIntervalSince1970: 2), isRead: false)
    }

    private func roundTrip<T: Codable>(_ value: T, as type: T.Type) throws -> T {
        try JSONDecoder().decode(type, from: JSONEncoder().encode(value))
    }

    private func typeName(_ value: Any) -> String {
        let name = String(describing: value).split(separator: "(").first.map(String.init) ?? ""
        return ".\(name)"
    }
}

struct ProtocolSample<T: Sendable>: CustomTestStringConvertible, Sendable {
    let value: T
    let caseName: String

    var testDescription: String { caseName }

    init(_ value: T, caseName: String) {
        self.value = value
        self.caseName = caseName
    }
}

private struct IDs {
    let projectID = UUID(uuidString: "00000000-0000-0000-0000-000000000001")!
    let worktreeID = UUID(uuidString: "00000000-0000-0000-0000-000000000002")!
    let areaID = UUID(uuidString: "00000000-0000-0000-0000-000000000003")!
    let tabID = UUID(uuidString: "00000000-0000-0000-0000-000000000004")!
    let paneID = UUID(uuidString: "00000000-0000-0000-0000-000000000005")!
    let deviceID = UUID(uuidString: "00000000-0000-0000-0000-000000000006")!
    let notificationID = UUID(uuidString: "00000000-0000-0000-0000-000000000007")!
}
