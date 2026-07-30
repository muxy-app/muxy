import Foundation
import MuxyShared
import Testing

@testable import MuxyServer

@MainActor
private final class MockDelegate: MuxyRemoteServerDelegate {
    var listProjectsCalled = 0
    var selectProjectCalls: [UUID] = []
    var terminalInputCalls: [(paneID: UUID, bytes: Data, clientID: UUID)] = []
    var takeOverCalls: [(paneID: UUID, clientID: UUID, cols: UInt32, rows: UInt32)] = []
    var releasePaneCalls: [(paneID: UUID, clientID: UUID)] = []
    var setClientThemeCalls: [(theme: ClientThemeDTO?, clientID: UUID)] = []
    var registerDeviceCalls: [(clientID: UUID, name: String)] = []
    var clientDisconnectedCalls: [UUID] = []
    var markNotificationReadCalls: [UUID] = []
    var vcsPushCalls: [UUID] = []
    var vcsPullCalls: [UUID] = []
    var vcsStageFilesCalls: [(projectID: UUID, paths: [String])] = []
    var vcsUnstageFilesCalls: [(projectID: UUID, paths: [String])] = []
    var vcsDiscardFilesCalls: [(projectID: UUID, paths: [String], untrackedPaths: [String])] = []
    var vcsListBranchesCalls: [UUID] = []
    var vcsSwitchBranchCalls: [(projectID: UUID, branch: String)] = []
    var vcsCreateBranchCalls: [(projectID: UUID, name: String)] = []
    var vcsCreatePRCalls: [(projectID: UUID, title: String, body: String, baseBranch: String?, draft: Bool)] = []
    var vcsMergePullRequestCalls: [(projectID: UUID, number: Int, method: VCSMergeMethodDTO, deleteBranch: Bool)] = []
    var vcsAddWorktreeCalls: [(projectID: UUID, name: String, branch: String, createBranch: Bool, baseBranch: String?)] = []
    var vcsRemoveWorktreeCalls: [(projectID: UUID, worktreeID: UUID)] = []

    var stubProjects: [ProjectDTO] = []
    var stubWorkspaces: [WorkspaceInfoDTO] = []
    var listProjectsByWorkspaceCalls: [UUID] = []
    var stubProjectsByWorkspace: [UUID: [ProjectDTO]] = [:]
    var stubWorkspace: WorkspaceDTO?
    var stubTab: TabDTO?
    var stubTerminalContent: TerminalCellsDTO?
    var vcsCommitError: Error?
    var vcsRefreshCalls: [UUID] = []
    var stubVCSStatus: VCSStatusDTO?
    var stubVCSBranches = VCSBranchesDTO(current: "main", locals: ["main"], defaultBranch: "main")
    var stubCreatePRResult = VCSCreatePRResultDTO(url: "https://example.com", number: 42)
    var stubAddedWorktree = WorktreeDTO(id: UUID(), name: "wt", path: "/tmp/wt", isPrimary: false, createdAt: Date())

    func listProjects() -> [ProjectDTO] {
        listProjectsCalled += 1
        return stubProjects
    }

    func listWorkspaces() -> [WorkspaceInfoDTO] {
        stubWorkspaces
    }

    func listProjectsByWorkspace(workspaceID: UUID) -> [ProjectDTO] {
        listProjectsByWorkspaceCalls.append(workspaceID)
        return stubProjectsByWorkspace[workspaceID] ?? []
    }

    func selectProject(_ projectID: UUID) {
        selectProjectCalls.append(projectID)
    }

    func listWorktrees(projectID _: UUID) -> [WorktreeDTO] { [] }
    func selectWorktree(projectID _: UUID, worktreeID _: UUID) {}
    func getWorkspace(projectID _: UUID) -> WorkspaceDTO? { stubWorkspace }
    func createTab(projectID _: UUID, areaID _: UUID?, kind _: TabKindDTO) -> TabDTO? { stubTab }
    func closeTab(projectID _: UUID, areaID _: UUID, tabID _: UUID) {}
    func selectTab(projectID _: UUID, areaID _: UUID, tabID _: UUID) {}
    func splitArea(projectID _: UUID, areaID _: UUID, direction _: SplitDirectionDTO, position _: SplitPositionDTO) {}
    func closeArea(projectID _: UUID, areaID _: UUID) {}
    func focusArea(projectID _: UUID, areaID _: UUID) {}

    func sendTerminalInput(paneID: UUID, bytes: Data, clientID: UUID) {
        terminalInputCalls.append((paneID, bytes, clientID))
    }

    func resizeTerminal(paneID _: UUID, cols _: UInt32, rows _: UInt32, clientID _: UUID) {}
    func scrollTerminal(paneID _: UUID, deltaX _: Double, deltaY _: Double, precise _: Bool, clientID _: UUID) {}
    func getTerminalContent(paneID _: UUID) -> TerminalCellsDTO? { stubTerminalContent }

    func takeOverPane(paneID: UUID, clientID: UUID, cols: UInt32, rows: UInt32) {
        takeOverCalls.append((paneID, clientID, cols, rows))
    }

    func releasePane(paneID: UUID, clientID: UUID) {
        releasePaneCalls.append((paneID, clientID))
    }

    func setClientTheme(_ theme: ClientThemeDTO?, clientID: UUID) {
        setClientThemeCalls.append((theme, clientID))
    }

    func registerDevice(clientID: UUID, name: String) {
        registerDeviceCalls.append((clientID, name))
    }

    func authenticateDevice(deviceID _: UUID, token _: String, name: String) -> DeviceAuthDecision {
        .approved(deviceName: name)
    }

    func requestPairing(deviceID _: UUID, token _: String, name: String) async -> DeviceAuthDecision {
        .approved(deviceName: name)
    }

    func getDeviceTheme() -> DeviceThemeEventDTO? { nil }

    func clientDisconnected(clientID: UUID) {
        clientDisconnectedCalls.append(clientID)
    }

    func getPaneOwner(paneID _: UUID) -> PaneOwnerDTO? { nil }
    func getVCSStatus(projectID _: UUID) async -> VCSStatusDTO? { nil }

    func vcsRefresh(projectID: UUID) async -> VCSStatusDTO? {
        vcsRefreshCalls.append(projectID)
        return stubVCSStatus
    }

    func vcsCommit(projectID _: UUID, message _: String, stageAll _: Bool) async throws {
        if let vcsCommitError { throw vcsCommitError }
    }

    func vcsPush(projectID: UUID) async throws {
        vcsPushCalls.append(projectID)
    }

    func vcsPull(projectID: UUID) async throws {
        vcsPullCalls.append(projectID)
    }

    func vcsStageFiles(projectID: UUID, paths: [String]) async throws {
        vcsStageFilesCalls.append((projectID, paths))
    }

    func vcsUnstageFiles(projectID: UUID, paths: [String]) async throws {
        vcsUnstageFilesCalls.append((projectID, paths))
    }

    func vcsDiscardFiles(projectID: UUID, paths: [String], untrackedPaths: [String]) async throws {
        vcsDiscardFilesCalls.append((projectID, paths, untrackedPaths))
    }

    func vcsListBranches(projectID: UUID) async throws -> VCSBranchesDTO {
        vcsListBranchesCalls.append(projectID)
        return stubVCSBranches
    }

    func vcsSwitchBranch(projectID: UUID, branch: String) async throws {
        vcsSwitchBranchCalls.append((projectID, branch))
    }

    func vcsCreateBranch(projectID: UUID, name: String) async throws {
        vcsCreateBranchCalls.append((projectID, name))
    }

    func vcsCreatePR(
        projectID: UUID,
        title: String,
        body: String,
        baseBranch: String?,
        draft: Bool
    ) async throws -> VCSCreatePRResultDTO {
        vcsCreatePRCalls.append((projectID, title, body, baseBranch, draft))
        return stubCreatePRResult
    }

    func vcsMergePullRequest(
        projectID: UUID,
        number: Int,
        method: VCSMergeMethodDTO,
        deleteBranch: Bool
    ) async throws {
        vcsMergePullRequestCalls.append((projectID, number, method, deleteBranch))
    }

    func vcsAddWorktree(
        projectID: UUID,
        name: String,
        branch: String,
        createBranch: Bool,
        baseBranch: String?
    ) async throws -> WorktreeDTO {
        vcsAddWorktreeCalls.append((projectID, name, branch, createBranch, baseBranch))
        return stubAddedWorktree
    }

    func vcsRemoveWorktree(projectID: UUID, worktreeID: UUID) async throws {
        vcsRemoveWorktreeCalls.append((projectID, worktreeID))
    }

    func vcsGetDiff(projectID _: UUID, filePath: String, forceFull _: Bool) async throws -> VCSDiffDTO {
        VCSDiffDTO(filePath: filePath, rows: [], additions: 0, deletions: 0, truncated: false, isBinary: false)
    }

    var filesListCalls: [(projectID: UUID, path: String)] = []
    var filesReadCalls: [(projectID: UUID, path: String, encoding: FileEncodingDTO)] = []
    var filesStatCalls: [(projectID: UUID, path: String)] = []
    var filesWriteCalls: [(projectID: UUID, path: String, contents: String, encoding: FileEncodingDTO)] = []
    var filesMkdirCalls: [(projectID: UUID, path: String)] = []
    var filesRenameCalls: [(projectID: UUID, path: String, newName: String)] = []
    var filesMoveCalls: [(projectID: UUID, paths: [String], into: String)] = []
    var filesDeleteCalls: [(projectID: UUID, paths: [String])] = []

    var stubFileEntries: [FileEntryDTO] = []
    var stubFileContent = FileContentDTO(path: "a.txt", content: "hello", size: 5, encoding: .utf8)
    var stubFileStat = FileStatDTO(name: "a.txt", path: "a.txt", isDirectory: false, size: 5)
    var stubFileMovedPaths: [String] = []
    var filesError: Error?

    func filesList(projectID: UUID, path: String) async throws -> [FileEntryDTO] {
        filesListCalls.append((projectID, path))
        if let filesError { throw filesError }
        return stubFileEntries
    }

    func filesRead(projectID: UUID, path: String, encoding: FileEncodingDTO) async throws -> FileContentDTO {
        filesReadCalls.append((projectID, path, encoding))
        if let filesError { throw filesError }
        return stubFileContent
    }

    func filesStat(projectID: UUID, path: String) async throws -> FileStatDTO {
        filesStatCalls.append((projectID, path))
        if let filesError { throw filesError }
        return stubFileStat
    }

    func filesWrite(projectID: UUID, path: String, contents: String, encoding: FileEncodingDTO) async throws -> String {
        filesWriteCalls.append((projectID, path, contents, encoding))
        if let filesError { throw filesError }
        return path
    }

    func filesMkdir(projectID: UUID, path: String) async throws -> String {
        filesMkdirCalls.append((projectID, path))
        if let filesError { throw filesError }
        return path
    }

    func filesRename(projectID: UUID, path: String, newName: String) async throws -> String {
        filesRenameCalls.append((projectID, path, newName))
        if let filesError { throw filesError }
        return newName
    }

    func filesMove(projectID: UUID, paths: [String], into destination: String) async throws -> [String] {
        filesMoveCalls.append((projectID, paths, destination))
        if let filesError { throw filesError }
        return stubFileMovedPaths
    }

    func filesDelete(projectID: UUID, paths: [String]) async throws {
        filesDeleteCalls.append((projectID, paths))
        if let filesError { throw filesError }
    }

    func getProjectLogo(projectID _: UUID) -> ProjectLogoDTO? { nil }
    func listNotifications() -> [NotificationDTO] { [] }

    func markNotificationRead(_ notificationID: UUID) {
        markNotificationReadCalls.append(notificationID)
    }

    var extensionRequestCalls: [(extension: String, action: String, payload: MuxyJSON, clientID: UUID)] = []
    var stubExtensionResult: Result<MuxyJSON, MuxyError> = .success(.null)

    func extensionRequest(extension: String, action: String, payload: MuxyJSON, clientID: UUID) async -> Result<MuxyJSON, MuxyError> {
        extensionRequestCalls.append((`extension`, action, payload, clientID))
        return stubExtensionResult
    }
}

@Suite("MuxyRemoteServer routing")
@MainActor
struct MuxyRemoteServerRoutingTests {
    private func makeServer() -> (MuxyRemoteServer, MockDelegate) {
        let server = MuxyRemoteServer()
        let delegate = MockDelegate()
        server.delegate = delegate
        return (server, delegate)
    }

    private func authedClient(on server: MuxyRemoteServer) -> UUID {
        let id = UUID()
        server._testingMarkAuthenticated(id)
        return id
    }

    @Test("listProjects routes to delegate and returns projects")
    func listProjectsRoutes() async {
        let (server, delegate) = makeServer()
        let project = ProjectDTO(
            id: UUID(),
            name: "Muxy",
            path: "/tmp/muxy",
            sortOrder: 0,
            createdAt: Date(timeIntervalSince1970: 0),
            icon: nil,
            logo: nil
        )
        delegate.stubProjects = [project]

        let response = await server.processRequest(
            MuxyRequest(id: "1", method: .listProjects),
            clientID: authedClient(on: server)
        )

        #expect(delegate.listProjectsCalled == 1)
        guard case let .projects(projects) = response.result else {
            Issue.record("expected projects result")
            return
        }
        #expect(projects.count == 1)
        #expect(projects.first?.id == project.id)
        #expect(response.error == nil)
    }

    @Test("listWorkspaces routes to delegate and returns workspaces")
    func listWorkspacesRoutes() async {
        let (server, delegate) = makeServer()
        let workspace = WorkspaceInfoDTO(
            id: WorkspaceInfoDTO.defaultLocalID,
            name: "Local",
            kind: .local,
            isDefault: true,
            projectCount: 2
        )
        delegate.stubWorkspaces = [workspace]

        let response = await server.processRequest(
            MuxyRequest(id: "ws-1", method: .listWorkspaces),
            clientID: authedClient(on: server)
        )

        guard case let .workspaces(workspaces) = response.result else {
            Issue.record("expected workspaces result")
            return
        }
        #expect(workspaces.count == 1)
        #expect(workspaces.first?.id == WorkspaceInfoDTO.defaultLocalID)
        #expect(workspaces.first?.isDefault == true)
        #expect(workspaces.first?.projectCount == 2)
        #expect(response.error == nil)
    }

    @Test("listProjectsByWorkspace forwards workspaceID and returns its projects")
    func listProjectsByWorkspaceRoutes() async {
        let (server, delegate) = makeServer()
        let workspaceID = UUID()
        let project = ProjectDTO(
            id: UUID(),
            name: "Muxy",
            path: "/tmp/muxy",
            sortOrder: 0,
            createdAt: Date(timeIntervalSince1970: 0),
            workspaceID: workspaceID,
            workspaceName: "Frontend",
            workspaceKind: .local
        )
        delegate.stubProjectsByWorkspace = [workspaceID: [project]]

        let response = await server.processRequest(
            MuxyRequest(
                id: "ws-2",
                method: .listProjectsByWorkspace,
                params: .listProjectsByWorkspace(ListProjectsByWorkspaceParams(workspaceID: workspaceID))
            ),
            clientID: authedClient(on: server)
        )

        #expect(delegate.listProjectsByWorkspaceCalls == [workspaceID])
        guard case let .projects(projects) = response.result else {
            Issue.record("expected projects result")
            return
        }
        #expect(projects.count == 1)
        #expect(projects.first?.workspaceID == workspaceID)
        #expect(response.error == nil)
    }

    @Test("workspace methods require authentication")
    func workspaceMethodsRequireAuth() async {
        let (server, delegate) = makeServer()

        let listResponse = await server.processRequest(
            MuxyRequest(id: "ws-auth-1", method: .listWorkspaces),
            clientID: UUID()
        )
        let byWorkspaceResponse = await server.processRequest(
            MuxyRequest(
                id: "ws-auth-2",
                method: .listProjectsByWorkspace,
                params: .listProjectsByWorkspace(ListProjectsByWorkspaceParams(workspaceID: UUID()))
            ),
            clientID: UUID()
        )

        #expect(listResponse.error?.code == 401)
        #expect(byWorkspaceResponse.error?.code == 401)
        #expect(delegate.listProjectsByWorkspaceCalls.isEmpty)
    }

    @Test("listProjectsByWorkspace rejects missing params as invalidParams")
    func listProjectsByWorkspaceInvalidParams() async {
        let (server, delegate) = makeServer()

        let response = await server.processRequest(
            MuxyRequest(id: "ws-3", method: .listProjectsByWorkspace, params: nil),
            clientID: authedClient(on: server)
        )

        #expect(delegate.listProjectsByWorkspaceCalls.isEmpty)
        #expect(response.error?.code == 400)
        #expect(response.result == nil)
    }

    @Test("selectProject forwards projectID")
    func selectProjectRoutes() async {
        let (server, delegate) = makeServer()
        let projectID = UUID()

        let response = await server.processRequest(
            MuxyRequest(
                id: "2",
                method: .selectProject,
                params: .selectProject(SelectProjectParams(projectID: projectID))
            ),
            clientID: authedClient(on: server)
        )

        #expect(delegate.selectProjectCalls == [projectID])
        guard case .ok = response.result else {
            Issue.record("expected ok")
            return
        }
    }

    @Test("selectProject rejects wrong params as invalidParams")
    func selectProjectInvalidParams() async {
        let (server, delegate) = makeServer()

        let response = await server.processRequest(
            MuxyRequest(id: "3", method: .selectProject, params: nil),
            clientID: authedClient(on: server)
        )

        #expect(delegate.selectProjectCalls.isEmpty)
        #expect(response.error?.code == 400)
        #expect(response.result == nil)
    }

    @Test("terminalInput threads clientID from connection into delegate")
    func terminalInputCarriesClientID() async {
        let (server, delegate) = makeServer()
        let clientID = authedClient(on: server)
        let paneID = UUID()

        _ = await server.processRequest(
            MuxyRequest(
                id: "4",
                method: .terminalInput,
                params: .terminalInput(TerminalInputParams(paneID: paneID, bytes: Data("hello".utf8)))
            ),
            clientID: clientID
        )

        #expect(delegate.terminalInputCalls.count == 1)
        #expect(delegate.terminalInputCalls.first?.paneID == paneID)
        #expect(delegate.terminalInputCalls.first?.bytes == Data("hello".utf8))
        #expect(delegate.terminalInputCalls.first?.clientID == clientID)
    }

    @Test("takeOverPane threads clientID and sizes through")
    func takeOverPaneRoutes() async {
        let (server, delegate) = makeServer()
        let clientID = authedClient(on: server)
        let paneID = UUID()

        _ = await server.processRequest(
            MuxyRequest(
                id: "5",
                method: .takeOverPane,
                params: .takeOverPane(TakeOverPaneParams(paneID: paneID, cols: 80, rows: 24))
            ),
            clientID: clientID
        )

        #expect(delegate.takeOverCalls.count == 1)
        let call = delegate.takeOverCalls.first
        #expect(call?.paneID == paneID)
        #expect(call?.clientID == clientID)
        #expect(call?.cols == 80)
        #expect(call?.rows == 24)
    }

    @Test("setClientTheme threads theme and clientID through")
    func setClientThemeRoutes() async {
        let (server, delegate) = makeServer()
        let clientID = authedClient(on: server)
        let theme = ClientThemeDTO(fg: 0xD4D4D4, bg: 0x141414, palette: Array(repeating: 0x242424, count: 16))

        let response = await server.processRequest(
            MuxyRequest(
                id: "set-theme",
                method: .setClientTheme,
                params: .setClientTheme(SetClientThemeParams(theme: theme))
            ),
            clientID: clientID
        )

        #expect(delegate.setClientThemeCalls.count == 1)
        #expect(delegate.setClientThemeCalls.first?.clientID == clientID)
        #expect(delegate.setClientThemeCalls.first?.theme == theme)
        guard case .ok = response.result else {
            Issue.record("expected ok result")
            return
        }
    }

    @Test("setClientTheme requires authentication")
    func setClientThemeRequiresAuth() async {
        let (server, delegate) = makeServer()

        let response = await server.processRequest(
            MuxyRequest(
                id: "set-theme-unauth",
                method: .setClientTheme,
                params: .setClientTheme(SetClientThemeParams(theme: nil))
            ),
            clientID: UUID()
        )

        #expect(delegate.setClientThemeCalls.isEmpty)
        #expect(response.error?.code == 401)
    }

    @Test("authenticateDevice stashes the client theme on approval")
    func authenticateDeviceStashesTheme() async {
        let (server, delegate) = makeServer()
        let theme = ClientThemeDTO(fg: 1, bg: 2, palette: Array(repeating: 3, count: 16))

        _ = await server.processRequest(
            MuxyRequest(
                id: "auth-theme",
                method: .authenticateDevice,
                params: .authenticateDevice(
                    AuthenticateDeviceParams(deviceID: UUID(), deviceName: "iPhone", token: "token", theme: theme)
                )
            ),
            clientID: UUID()
        )

        #expect(delegate.setClientThemeCalls.first?.theme == theme)
    }

    @Test("registerDevice returns device info with clientID")
    func registerDeviceResponse() async {
        let (server, delegate) = makeServer()
        let clientID = authedClient(on: server)

        let response = await server.processRequest(
            MuxyRequest(
                id: "6",
                method: .registerDevice,
                params: .registerDevice(RegisterDeviceParams(deviceName: "iPhone"))
            ),
            clientID: clientID
        )

        #expect(delegate.registerDeviceCalls.first?.clientID == clientID)
        #expect(delegate.registerDeviceCalls.first?.name == "iPhone")
        guard case let .deviceInfo(info) = response.result else {
            Issue.record("expected deviceInfo result")
            return
        }
        #expect(info.clientID == clientID)
        #expect(info.deviceName == "iPhone")
    }

    @Test("getWorkspace returns notFound when delegate has no workspace")
    func getWorkspaceNotFound() async {
        let (server, delegate) = makeServer()

        let response = await server.processRequest(
            MuxyRequest(
                id: "7",
                method: .getWorkspace,
                params: .getWorkspace(GetWorkspaceParams(projectID: UUID()))
            ),
            clientID: authedClient(on: server)
        )

        #expect(delegate.stubWorkspace == nil)
        #expect(response.error?.code == 404)
    }

    @Test("vcsCommit surfaces delegate error as 500 response")
    func vcsCommitErrorResponse() async {
        struct Boom: Error, LocalizedError {
            var errorDescription: String? { "boom" }
        }

        let (server, delegate) = makeServer()
        delegate.vcsCommitError = Boom()

        let response = await server.processRequest(
            MuxyRequest(
                id: "8",
                method: .vcsCommit,
                params: .vcsCommit(VCSCommitParams(projectID: UUID(), message: "msg", stageAll: true))
            ),
            clientID: authedClient(on: server)
        )

        #expect(response.error?.code == 500)
        #expect(response.error?.message == "boom")
    }

    @Test("vcs stage routes pass through payload")
    func vcsStageRoutes() async {
        let (server, delegate) = makeServer()
        let projectID = UUID()

        let stageResponse = await server.processRequest(
            MuxyRequest(
                id: "8a",
                method: .vcsStageFiles,
                params: .vcsStageFiles(VCSStageFilesParams(projectID: projectID, paths: ["a.swift", "b.swift"]))
            ),
            clientID: authedClient(on: server)
        )
        let unstageResponse = await server.processRequest(
            MuxyRequest(
                id: "8b",
                method: .vcsUnstageFiles,
                params: .vcsUnstageFiles(VCSUnstageFilesParams(projectID: projectID, paths: ["a.swift"]))
            ),
            clientID: authedClient(on: server)
        )
        let discardResponse = await server.processRequest(
            MuxyRequest(
                id: "8c",
                method: .vcsDiscardFiles,
                params: .vcsDiscardFiles(VCSDiscardFilesParams(projectID: projectID, paths: ["a.swift"], untrackedPaths: ["tmp.txt"]))
            ),
            clientID: authedClient(on: server)
        )

        #expect(delegate.vcsStageFilesCalls.first?.projectID == projectID)
        #expect(delegate.vcsStageFilesCalls.first?.paths == ["a.swift", "b.swift"])
        #expect(delegate.vcsUnstageFilesCalls.first?.projectID == projectID)
        #expect(delegate.vcsUnstageFilesCalls.first?.paths == ["a.swift"])
        #expect(delegate.vcsDiscardFilesCalls.first?.projectID == projectID)
        #expect(delegate.vcsDiscardFilesCalls.first?.paths == ["a.swift"])
        #expect(delegate.vcsDiscardFilesCalls.first?.untrackedPaths == ["tmp.txt"])
        #expect(stageResponse.error == nil)
        #expect(unstageResponse.error == nil)
        #expect(discardResponse.error == nil)
    }

    @Test("vcs push and pull routes call delegate")
    func vcsPushPullRoutes() async {
        let (server, delegate) = makeServer()
        let projectID = UUID()
        let clientID = authedClient(on: server)

        let pushResponse = await server.processRequest(
            MuxyRequest(id: "8d", method: .vcsPush, params: .vcsPush(VCSPushParams(projectID: projectID))),
            clientID: clientID
        )
        let pullResponse = await server.processRequest(
            MuxyRequest(id: "8e", method: .vcsPull, params: .vcsPull(VCSPullParams(projectID: projectID))),
            clientID: clientID
        )

        #expect(delegate.vcsPushCalls == [projectID])
        #expect(delegate.vcsPullCalls == [projectID])
        #expect(pushResponse.error == nil)
        #expect(pullResponse.error == nil)
    }

    @Test("vcsRefresh routes and returns delegate status")
    func vcsRefreshRoutes() async {
        let (server, delegate) = makeServer()
        let projectID = UUID()
        delegate.stubVCSStatus = VCSStatusDTO(
            branch: "feature/x",
            aheadCount: 1,
            behindCount: 0,
            hasUpstream: true,
            stagedFiles: [],
            changedFiles: [],
            defaultBranch: "main",
            pullRequest: nil
        )

        let response = await server.processRequest(
            MuxyRequest(id: "8r", method: .vcsRefresh, params: .vcsRefresh(VCSRefreshParams(projectID: projectID))),
            clientID: authedClient(on: server)
        )

        #expect(delegate.vcsRefreshCalls == [projectID])
        guard case let .vcsStatus(status) = response.result else {
            Issue.record("expected vcsStatus result")
            return
        }
        #expect(status.branch == "feature/x")
        #expect(status.aheadCount == 1)
    }

    @Test("vcs branch routes return and forward data")
    func vcsBranchRoutes() async {
        let (server, delegate) = makeServer()
        let projectID = UUID()
        let clientID = authedClient(on: server)

        let listResponse = await server.processRequest(
            MuxyRequest(id: "8f", method: .vcsListBranches, params: .vcsListBranches(VCSListBranchesParams(projectID: projectID))),
            clientID: clientID
        )
        _ = await server.processRequest(
            MuxyRequest(id: "8g", method: .vcsSwitchBranch, params: .vcsSwitchBranch(VCSSwitchBranchParams(projectID: projectID, branch: "feature/a"))),
            clientID: clientID
        )
        _ = await server.processRequest(
            MuxyRequest(id: "8h", method: .vcsCreateBranch, params: .vcsCreateBranch(VCSCreateBranchParams(projectID: projectID, name: "feature/b"))),
            clientID: clientID
        )

        #expect(delegate.vcsListBranchesCalls == [projectID])
        #expect(delegate.vcsSwitchBranchCalls.first?.projectID == projectID)
        #expect(delegate.vcsSwitchBranchCalls.first?.branch == "feature/a")
        #expect(delegate.vcsCreateBranchCalls.first?.projectID == projectID)
        #expect(delegate.vcsCreateBranchCalls.first?.name == "feature/b")
        guard case let .vcsBranches(branches) = listResponse.result else {
            Issue.record("expected vcsBranches result")
            return
        }
        #expect(branches.current == delegate.stubVCSBranches.current)
    }

    @Test("vcs create PR route returns delegate payload")
    func vcsCreatePRRoute() async {
        let (server, delegate) = makeServer()
        let projectID = UUID()

        let response = await server.processRequest(
            MuxyRequest(
                id: "8i",
                method: .vcsCreatePR,
                params: .vcsCreatePR(VCSCreatePRParams(
                    projectID: projectID,
                    title: "Add feature",
                    body: "Body",
                    baseBranch: "main",
                    draft: true
                ))
            ),
            clientID: authedClient(on: server)
        )

        #expect(delegate.vcsCreatePRCalls.first?.projectID == projectID)
        #expect(delegate.vcsCreatePRCalls.first?.title == "Add feature")
        #expect(delegate.vcsCreatePRCalls.first?.body == "Body")
        #expect(delegate.vcsCreatePRCalls.first?.baseBranch == "main")
        #expect(delegate.vcsCreatePRCalls.first?.draft == true)
        guard case let .vcsPRCreated(result) = response.result else {
            Issue.record("expected vcsPRCreated result")
            return
        }
        #expect(result.url == delegate.stubCreatePRResult.url)
        #expect(result.number == delegate.stubCreatePRResult.number)
    }

    @Test("vcs merge pull request route forwards params")
    func vcsMergePullRequestRoute() async {
        let (server, delegate) = makeServer()
        let projectID = UUID()

        let response = await server.processRequest(
            MuxyRequest(
                id: "8m",
                method: .vcsMergePullRequest,
                params: .vcsMergePullRequest(VCSMergePullRequestParams(
                    projectID: projectID,
                    number: 42,
                    method: .squash,
                    deleteBranch: true
                ))
            ),
            clientID: authedClient(on: server)
        )

        #expect(delegate.vcsMergePullRequestCalls.first?.projectID == projectID)
        #expect(delegate.vcsMergePullRequestCalls.first?.number == 42)
        #expect(delegate.vcsMergePullRequestCalls.first?.method == .squash)
        #expect(delegate.vcsMergePullRequestCalls.first?.deleteBranch == true)
        guard case .ok = response.result else {
            Issue.record("expected ok result")
            return
        }
    }

    @Test("vcs worktree routes forward input and output")
    func vcsWorktreeRoutes() async {
        let (server, delegate) = makeServer()
        let projectID = UUID()
        let worktreeID = UUID()
        let clientID = authedClient(on: server)

        let addResponse = await server.processRequest(
            MuxyRequest(
                id: "8j",
                method: .vcsAddWorktree,
                params: .vcsAddWorktree(VCSAddWorktreeParams(
                    projectID: projectID,
                    name: "wt",
                    branch: "feature",
                    createBranch: true,
                    baseBranch: "main"
                ))
            ),
            clientID: clientID
        )
        let removeResponse = await server.processRequest(
            MuxyRequest(
                id: "8k",
                method: .vcsRemoveWorktree,
                params: .vcsRemoveWorktree(VCSRemoveWorktreeParams(projectID: projectID, worktreeID: worktreeID))
            ),
            clientID: clientID
        )

        #expect(delegate.vcsAddWorktreeCalls.first?.projectID == projectID)
        #expect(delegate.vcsAddWorktreeCalls.first?.name == "wt")
        #expect(delegate.vcsAddWorktreeCalls.first?.branch == "feature")
        #expect(delegate.vcsAddWorktreeCalls.first?.createBranch == true)
        #expect(delegate.vcsAddWorktreeCalls.first?.baseBranch == "main")
        #expect(delegate.vcsRemoveWorktreeCalls.first?.projectID == projectID)
        #expect(delegate.vcsRemoveWorktreeCalls.first?.worktreeID == worktreeID)
        guard case let .worktrees(worktrees) = addResponse.result else {
            Issue.record("expected worktrees result")
            return
        }
        #expect(worktrees.first?.id == delegate.stubAddedWorktree.id)
        #expect(removeResponse.error == nil)
    }

    @Test("files read routes return their delegate payloads")
    func filesReadRoutes() async {
        let (server, delegate) = makeServer()
        let projectID = UUID()
        let clientID = authedClient(on: server)
        delegate.stubFileEntries = [
            FileEntryDTO(name: "src", path: "src", isDirectory: true, isIgnored: false),
        ]

        let listResponse = await server.processRequest(
            MuxyRequest(id: "f1", method: .filesList, params: .filesList(FilesListParams(projectID: projectID, path: "."))),
            clientID: clientID
        )
        let readResponse = await server.processRequest(
            MuxyRequest(
                id: "f2",
                method: .filesRead,
                params: .filesRead(FilesReadParams(projectID: projectID, path: "a.txt", encoding: .base64))
            ),
            clientID: clientID
        )
        let statResponse = await server.processRequest(
            MuxyRequest(id: "f3", method: .filesStat, params: .filesStat(FilesStatParams(projectID: projectID, path: "a.txt"))),
            clientID: clientID
        )

        #expect(delegate.filesListCalls.first?.projectID == projectID)
        #expect(delegate.filesListCalls.first?.path == ".")
        #expect(delegate.filesReadCalls.first?.encoding == .base64)
        #expect(delegate.filesStatCalls.first?.path == "a.txt")

        guard case let .files(entries) = listResponse.result,
              case let .fileContent(content) = readResponse.result,
              case let .fileStat(stat) = statResponse.result
        else {
            Issue.record("expected files, fileContent, and fileStat results")
            return
        }
        #expect(entries.first?.name == "src")
        #expect(content.content == "hello")
        #expect(stat.name == "a.txt")
    }

    @Test("files write routes forward payloads and return paths")
    func filesWriteRoutes() async {
        let (server, delegate) = makeServer()
        let projectID = UUID()
        let clientID = authedClient(on: server)
        delegate.stubFileMovedPaths = ["archive/a.txt", "archive/b.txt"]

        let writeResponse = await server.processRequest(
            MuxyRequest(
                id: "f4",
                method: .filesWrite,
                params: .filesWrite(FilesWriteParams(
                    projectID: projectID,
                    path: "notes/todo.md",
                    contents: "# Todo",
                    encoding: .utf8
                ))
            ),
            clientID: clientID
        )
        let mkdirResponse = await server.processRequest(
            MuxyRequest(id: "f5", method: .filesMkdir, params: .filesMkdir(FilesMkdirParams(projectID: projectID, path: "notes"))),
            clientID: clientID
        )
        let renameResponse = await server.processRequest(
            MuxyRequest(
                id: "f6",
                method: .filesRename,
                params: .filesRename(FilesRenameParams(projectID: projectID, path: "todo.md", newName: "done.md"))
            ),
            clientID: clientID
        )
        let moveResponse = await server.processRequest(
            MuxyRequest(
                id: "f7",
                method: .filesMove,
                params: .filesMove(FilesMoveParams(projectID: projectID, paths: ["a.txt", "b.txt"], into: "archive"))
            ),
            clientID: clientID
        )
        let deleteResponse = await server.processRequest(
            MuxyRequest(id: "f8", method: .filesDelete, params: .filesDelete(FilesDeleteParams(projectID: projectID, paths: ["old.log"]))),
            clientID: clientID
        )

        #expect(delegate.filesWriteCalls.first?.contents == "# Todo")
        #expect(delegate.filesWriteCalls.first?.encoding == .utf8)
        #expect(delegate.filesMkdirCalls.first?.path == "notes")
        #expect(delegate.filesRenameCalls.first?.newName == "done.md")
        #expect(delegate.filesMoveCalls.first?.paths == ["a.txt", "b.txt"])
        #expect(delegate.filesMoveCalls.first?.into == "archive")
        #expect(delegate.filesDeleteCalls.first?.paths == ["old.log"])

        guard case let .filePaths(written) = writeResponse.result,
              case let .filePaths(created) = mkdirResponse.result,
              case let .filePaths(renamed) = renameResponse.result,
              case let .filePaths(moved) = moveResponse.result,
              case .ok = deleteResponse.result
        else {
            Issue.record("expected filePaths results and ok for delete")
            return
        }
        #expect(written == ["notes/todo.md"])
        #expect(created == ["notes"])
        #expect(renamed == ["done.md"])
        #expect(moved == ["archive/a.txt", "archive/b.txt"])
    }

    @Test("files methods pass a thrown MuxyError through with its own code")
    func filesErrorPassthrough() async {
        let (server, delegate) = makeServer()
        delegate.filesError = MuxyError(code: 403, message: "path '../x' escapes the workspace root")

        let response = await server.processRequest(
            MuxyRequest(id: "f9", method: .filesList, params: .filesList(FilesListParams(projectID: UUID(), path: "../x"))),
            clientID: authedClient(on: server)
        )

        #expect(response.error?.code == 403)
        #expect(response.error?.message == "path '../x' escapes the workspace root")
    }

    @Test("files methods wrap a plain error as 500")
    func filesErrorWrapped() async {
        struct Boom: Error, LocalizedError {
            var errorDescription: String? { "boom" }
        }

        let (server, delegate) = makeServer()
        delegate.filesError = Boom()

        let response = await server.processRequest(
            MuxyRequest(id: "f10", method: .filesRead, params: .filesRead(FilesReadParams(projectID: UUID(), path: "a", encoding: .utf8))),
            clientID: authedClient(on: server)
        )

        #expect(response.error?.code == 500)
        #expect(response.error?.message == "boom")
    }

    @Test("files methods require authentication")
    func filesRequireAuth() async {
        let (server, delegate) = makeServer()

        let response = await server.processRequest(
            MuxyRequest(id: "f11", method: .filesList, params: .filesList(FilesListParams(projectID: UUID(), path: "."))),
            clientID: UUID()
        )

        #expect(delegate.filesListCalls.isEmpty)
        #expect(response.error?.code == 401)
    }

    @Test("files methods reject missing params as invalidParams")
    func filesInvalidParams() async {
        let (server, delegate) = makeServer()

        let response = await server.processRequest(
            MuxyRequest(id: "f12", method: .filesWrite, params: nil),
            clientID: authedClient(on: server)
        )

        #expect(delegate.filesWriteCalls.isEmpty)
        #expect(response.error?.code == 400)
    }

    @Test("subscribe and unsubscribe return ok")
    func subscribeOk() async {
        let (server, delegate) = makeServer()
        _ = delegate

        let subResponse = await server.processRequest(
            MuxyRequest(
                id: "9",
                method: .subscribe,
                params: .subscribe(SubscribeParams(events: [.workspaceChanged]))
            ),
            clientID: authedClient(on: server)
        )
        let unsubResponse = await server.processRequest(
            MuxyRequest(
                id: "10",
                method: .unsubscribe,
                params: .unsubscribe(UnsubscribeParams(events: [.workspaceChanged]))
            ),
            clientID: authedClient(on: server)
        )

        guard case .ok = subResponse.result, case .ok = unsubResponse.result else {
            Issue.record("expected ok for both subscribe and unsubscribe")
            return
        }
    }

    @Test("extensionRequest forwards params and returns delegate payload")
    func extensionRequestRoutes() async {
        let (server, delegate) = makeServer()
        delegate.stubExtensionResult = .success(.object(["pong": .number(42)]))

        let response = await server.processRequest(
            MuxyRequest(
                id: "12",
                method: .extensionRequest,
                params: .extensionRequest(ExtensionRequestParams(
                    extension: "weather",
                    action: "forecast",
                    payload: .object(["city": .string("Berlin")])
                ))
            ),
            clientID: authedClient(on: server)
        )

        #expect(delegate.extensionRequestCalls.first?.extension == "weather")
        #expect(delegate.extensionRequestCalls.first?.action == "forecast")
        guard case let .extensionResult(result) = response.result else {
            Issue.record("expected extensionResult result")
            return
        }
        #expect(result.payload == .object(["pong": .number(42)]))
    }

    @Test("extensionRequest surfaces delegate failure as the mapped error")
    func extensionRequestFailure() async {
        let (server, delegate) = makeServer()
        delegate.stubExtensionResult = .failure(.forbidden)

        let response = await server.processRequest(
            MuxyRequest(
                id: "13",
                method: .extensionRequest,
                params: .extensionRequest(ExtensionRequestParams(extension: "x", action: "y", payload: .null))
            ),
            clientID: authedClient(on: server)
        )

        #expect(response.result == nil)
        #expect(response.error?.code == 403)
    }

    @Test("extensionRequest requires authentication")
    func extensionRequestRequiresAuth() async {
        let (server, delegate) = makeServer()

        let response = await server.processRequest(
            MuxyRequest(
                id: "14",
                method: .extensionRequest,
                params: .extensionRequest(ExtensionRequestParams(extension: "x", action: "y", payload: .null))
            ),
            clientID: UUID()
        )

        #expect(delegate.extensionRequestCalls.isEmpty)
        #expect(response.error?.code == 401)
    }

    @Test("missing delegate returns internal error")
    func missingDelegateErrors() async {
        let server = MuxyRemoteServer()

        let response = await server.processRequest(
            MuxyRequest(id: "11", method: .listProjects),
            clientID: authedClient(on: server)
        )

        #expect(response.error?.code == 500)
    }
}
