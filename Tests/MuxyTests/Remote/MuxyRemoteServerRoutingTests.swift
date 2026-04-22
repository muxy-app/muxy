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
    var vcsAddWorktreeCalls: [(projectID: UUID, name: String, branch: String, createBranch: Bool)] = []
    var vcsRemoveWorktreeCalls: [(projectID: UUID, worktreeID: UUID)] = []

    var stubProjects: [ProjectDTO] = []
    var stubWorkspace: WorkspaceDTO?
    var stubTab: TabDTO?
    var stubTerminalContent: TerminalCellsDTO?
    var vcsCommitError: Error?
    var stubVCSBranches = VCSBranchesDTO(current: "main", locals: ["main"], defaultBranch: "main")
    var stubCreatePRResult = VCSCreatePRResultDTO(url: "https://example.com", number: 42)
    var stubAddedWorktree = WorktreeDTO(id: UUID(), name: "wt", path: "/tmp/wt", isPrimary: false, createdAt: Date())

    func listProjects() -> [ProjectDTO] {
        listProjectsCalled += 1
        return stubProjects
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

    func registerDevice(clientID: UUID, name: String) {
        registerDeviceCalls.append((clientID, name))
    }

    func authenticateDevice(deviceID _: UUID, token _: String, name: String) -> DeviceAuthDecision {
        .approved(deviceName: name)
    }

    func requestPairing(deviceID _: UUID, token _: String, name: String) async -> DeviceAuthDecision {
        .approved(deviceName: name)
    }

    func getDeviceTheme() -> (fg: UInt32, bg: UInt32)? { nil }

    func clientDisconnected(clientID: UUID) {
        clientDisconnectedCalls.append(clientID)
    }

    func getPaneOwner(paneID _: UUID) -> PaneOwnerDTO? { nil }
    func getVCSStatus(projectID _: UUID) async -> VCSStatusDTO? { nil }

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

    func vcsAddWorktree(
        projectID: UUID,
        name: String,
        branch: String,
        createBranch: Bool
    ) async throws -> WorktreeDTO {
        vcsAddWorktreeCalls.append((projectID, name, branch, createBranch))
        return stubAddedWorktree
    }

    func vcsRemoveWorktree(projectID: UUID, worktreeID: UUID) async throws {
        vcsRemoveWorktreeCalls.append((projectID, worktreeID))
    }
    func getProjectLogo(projectID _: UUID) -> ProjectLogoDTO? { nil }
    func listNotifications() -> [NotificationDTO] { [] }

    func markNotificationRead(_ notificationID: UUID) {
        markNotificationReadCalls.append(notificationID)
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
                    branch: "main",
                    createBranch: false
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
        #expect(delegate.vcsAddWorktreeCalls.first?.branch == "main")
        #expect(delegate.vcsAddWorktreeCalls.first?.createBranch == false)
        #expect(delegate.vcsRemoveWorktreeCalls.first?.projectID == projectID)
        #expect(delegate.vcsRemoveWorktreeCalls.first?.worktreeID == worktreeID)
        guard case let .worktrees(worktrees) = addResponse.result else {
            Issue.record("expected worktrees result")
            return
        }
        #expect(worktrees.first?.id == delegate.stubAddedWorktree.id)
        #expect(removeResponse.error == nil)
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
