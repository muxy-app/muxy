import Foundation
import MuxyShared
import Testing

@testable import MuxyServer

@MainActor
private final class MockDelegate: MuxyRemoteServerDelegate {
    var listProjectsCalled = 0
    var selectProjectCalls: [UUID] = []
    var terminalInputCalls: [(paneID: UUID, text: String, clientID: UUID)] = []
    var takeOverCalls: [(paneID: UUID, clientID: UUID, cols: UInt32, rows: UInt32)] = []
    var releasePaneCalls: [(paneID: UUID, clientID: UUID)] = []
    var registerDeviceCalls: [(clientID: UUID, name: String)] = []
    var clientDisconnectedCalls: [UUID] = []
    var markNotificationReadCalls: [UUID] = []

    var stubProjects: [ProjectDTO] = []
    var stubWorkspace: WorkspaceDTO?
    var stubTab: TabDTO?
    var stubTerminalContent: TerminalCellsDTO?
    var vcsCommitError: Error?

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

    func sendTerminalInput(paneID: UUID, text: String, clientID: UUID) {
        terminalInputCalls.append((paneID, text, clientID))
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

    func clientDisconnected(clientID: UUID) {
        clientDisconnectedCalls.append(clientID)
    }

    func getPaneOwner(paneID _: UUID) -> PaneOwnerDTO? { nil }
    func getVCSStatus(projectID _: UUID) async -> VCSStatusDTO? { nil }

    func vcsCommit(projectID _: UUID, message _: String, stageAll _: Bool) async throws {
        if let vcsCommitError { throw vcsCommitError }
    }

    func vcsPush(projectID _: UUID) async throws {}
    func vcsPull(projectID _: UUID) async throws {}
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
            clientID: UUID()
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
            clientID: UUID()
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
            clientID: UUID()
        )

        #expect(delegate.selectProjectCalls.isEmpty)
        #expect(response.error?.code == 400)
        #expect(response.result == nil)
    }

    @Test("terminalInput threads clientID from connection into delegate")
    func terminalInputCarriesClientID() async {
        let (server, delegate) = makeServer()
        let clientID = UUID()
        let paneID = UUID()

        _ = await server.processRequest(
            MuxyRequest(
                id: "4",
                method: .terminalInput,
                params: .terminalInput(TerminalInputParams(paneID: paneID, text: "hello"))
            ),
            clientID: clientID
        )

        #expect(delegate.terminalInputCalls.count == 1)
        #expect(delegate.terminalInputCalls.first?.paneID == paneID)
        #expect(delegate.terminalInputCalls.first?.text == "hello")
        #expect(delegate.terminalInputCalls.first?.clientID == clientID)
    }

    @Test("takeOverPane threads clientID and sizes through")
    func takeOverPaneRoutes() async {
        let (server, delegate) = makeServer()
        let clientID = UUID()
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
        let clientID = UUID()

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
            clientID: UUID()
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
            clientID: UUID()
        )

        #expect(response.error?.code == 500)
        #expect(response.error?.message == "boom")
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
            clientID: UUID()
        )
        let unsubResponse = await server.processRequest(
            MuxyRequest(
                id: "10",
                method: .unsubscribe,
                params: .unsubscribe(UnsubscribeParams(events: [.workspaceChanged]))
            ),
            clientID: UUID()
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
            clientID: UUID()
        )

        #expect(response.error?.code == 500)
    }
}
