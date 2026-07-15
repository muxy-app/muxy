import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("Remote Mac connection")
@MainActor
struct RemoteMacConnectionTests {
    @Test("unknown desktop client pairs, loads projects, and selects a workspace")
    func pairingAndWorkspaceLoad() async throws {
        let remoteDeviceID = UUID()
        let credentials = RemoteMacCredentials(
            deviceID: UUID(),
            token: "secret",
            endpointScope: "studio.local:4865"
        )
        let credentialStore = InMemoryRemoteMacCredentialStore(values: [remoteDeviceID: credentials])
        let socket = FakeRemoteMacSocket()
        let project = ProjectDTO(id: UUID(), name: "Muxy", path: "/repo", sortOrder: 0, createdAt: .now)
        let workspace = workspace(projectID: project.id)
        var receivedKinds: [RemoteClientKindDTO] = []

        socket.handler = { request in
            switch request.method {
            case .authenticateDevice:
                guard case let .authenticateDevice(params) = request.params else { return nil }
                receivedKinds.append(params.resolvedClientKind)
                #expect(params.deviceID == credentials.deviceID)
                return MuxyResponse(id: request.id, error: .unauthorized)
            case .pairDevice:
                guard case let .pairDevice(params) = request.params else { return nil }
                receivedKinds.append(params.resolvedClientKind)
                return MuxyResponse(
                    id: request.id,
                    result: .pairing(PairingResultDTO(clientID: UUID(), deviceName: "Test Mac"))
                )
            case .listProjects:
                return MuxyResponse(id: request.id, result: .projects([project]))
            case .selectProject:
                return MuxyResponse(id: request.id, result: .ok)
            case .getWorkspace:
                return MuxyResponse(id: request.id, result: .workspace(workspace))
            default:
                return MuxyResponse(id: request.id, result: .ok)
            }
        }

        let device = RemoteDevice(
            id: remoteDeviceID,
            name: "Studio",
            muxy: MuxyRemoteServerData(host: "studio.local")
        )
        let connection = RemoteMacConnection(
            device: device,
            credentialStore: credentialStore,
            socketFactory: { socket }
        )

        try await connection.connect(allowPairing: true)
        try await connection.selectProject(project.id)

        #expect(connection.state == .connected)
        #expect(connection.projects.map(\.id) == [project.id])
        #expect(connection.workspace?.projectID == project.id)
        #expect(receivedKinds == [.desktop, .desktop])
    }

    @Test("terminal input is sent without a pending response")
    func terminalInput() async throws {
        let remoteDeviceID = UUID()
        let credentialStore = InMemoryRemoteMacCredentialStore()
        let socket = FakeRemoteMacSocket()
        let paneID = UUID()
        var input: Data?
        socket.handler = { request in
            if case let .terminalInput(params) = request.params {
                input = params.bytes
                return nil
            }
            switch request.method {
            case .authenticateDevice:
                return MuxyResponse(
                    id: request.id,
                    result: .pairing(PairingResultDTO(clientID: UUID(), deviceName: "Test Mac"))
                )
            case .listProjects:
                return MuxyResponse(id: request.id, result: .projects([]))
            default:
                return MuxyResponse(id: request.id, result: .ok)
            }
        }
        let connection = RemoteMacConnection(
            device: RemoteDevice(
                id: remoteDeviceID,
                name: "Studio",
                muxy: MuxyRemoteServerData(host: "studio.local")
            ),
            credentialStore: credentialStore,
            socketFactory: { socket }
        )
        try await connection.connect(allowPairing: false)

        connection.sendTerminalInput(paneID: paneID, bytes: Data([1, 2, 3]))
        await Task.yield()

        #expect(input == Data([1, 2, 3]))
    }

    @Test("a slower project selection cannot overwrite a newer workspace")
    func projectSelectionOrdering() async throws {
        let socket = FakeRemoteMacSocket()
        let firstProject = ProjectDTO(id: UUID(), name: "First", path: "/first", sortOrder: 0, createdAt: .now)
        let secondProject = ProjectDTO(id: UUID(), name: "Second", path: "/second", sortOrder: 1, createdAt: .now)
        let firstWorkspace = workspace(projectID: firstProject.id)
        let secondWorkspace = workspace(projectID: secondProject.id)
        socket.handler = { request in
            switch request.method {
            case .authenticateDevice:
                return MuxyResponse(
                    id: request.id,
                    result: .pairing(PairingResultDTO(clientID: UUID(), deviceName: "Test Mac"))
                )
            case .listProjects:
                return MuxyResponse(id: request.id, result: .projects([firstProject, secondProject]))
            case .selectProject:
                guard case let .selectProject(params) = request.params else { return nil }
                if params.projectID == firstProject.id {
                    try? await Task.sleep(for: .milliseconds(50))
                }
                return MuxyResponse(id: request.id, result: .ok)
            case .getWorkspace:
                guard case let .getWorkspace(params) = request.params else { return nil }
                let workspace = params.projectID == firstProject.id ? firstWorkspace : secondWorkspace
                return MuxyResponse(id: request.id, result: .workspace(workspace))
            default:
                return MuxyResponse(id: request.id, result: .ok)
            }
        }
        let connection = RemoteMacConnection(
            device: RemoteDevice(name: "Studio", muxy: MuxyRemoteServerData(host: "studio.local")),
            credentialStore: InMemoryRemoteMacCredentialStore(),
            socketFactory: { socket }
        )
        try await connection.connect(allowPairing: false)

        async let first: Void = connection.selectProject(firstProject.id)
        await Task.yield()
        async let second: Void = connection.selectProject(secondProject.id)
        try await first
        try await second

        #expect(connection.activeProjectID == secondProject.id)
        #expect(connection.workspace?.projectID == secondProject.id)
    }

    @Test("release remains the final ownership operation when takeover is in flight")
    func takeoverReleaseOrdering() async throws {
        let socket = FakeRemoteMacSocket()
        let paneID = UUID()
        var completedMethods: [MuxyMethod] = []
        socket.handler = { request in
            switch request.method {
            case .authenticateDevice:
                return MuxyResponse(
                    id: request.id,
                    result: .pairing(PairingResultDTO(clientID: UUID(), deviceName: "Test Mac"))
                )
            case .listProjects:
                return MuxyResponse(id: request.id, result: .projects([]))
            case .takeOverPane:
                try? await Task.sleep(for: .milliseconds(50))
                completedMethods.append(.takeOverPane)
                return MuxyResponse(id: request.id, result: .ok)
            case .releasePane:
                completedMethods.append(.releasePane)
                return MuxyResponse(id: request.id, result: .ok)
            default:
                return MuxyResponse(id: request.id, result: .ok)
            }
        }
        let connection = RemoteMacConnection(
            device: RemoteDevice(name: "Studio", muxy: MuxyRemoteServerData(host: "studio.local")),
            credentialStore: InMemoryRemoteMacCredentialStore(),
            socketFactory: { socket }
        )
        try await connection.connect(allowPairing: false)

        let takeover = connection.takeOverPane(paneID: paneID, cols: 80, rows: 24)
        await connection.releasePane(paneID: paneID)
        try await takeover.value

        #expect(completedMethods.last == .releasePane)
        #expect(completedMethods.filter { $0 == .releasePane }.count == 2)
    }

    @Test("removing the active project clears its workspace")
    func activeProjectRemoval() async throws {
        let socket = FakeRemoteMacSocket()
        let project = ProjectDTO(id: UUID(), name: "Removed", path: "/removed", sortOrder: 0, createdAt: .now)
        let workspace = workspace(projectID: project.id)
        socket.handler = { request in
            switch request.method {
            case .authenticateDevice:
                return MuxyResponse(
                    id: request.id,
                    result: .pairing(PairingResultDTO(clientID: UUID(), deviceName: "Test Mac"))
                )
            case .listProjects:
                return MuxyResponse(id: request.id, result: .projects([project]))
            case .selectProject:
                return MuxyResponse(id: request.id, result: .ok)
            case .getWorkspace:
                return MuxyResponse(id: request.id, result: .workspace(workspace))
            default:
                return MuxyResponse(id: request.id, result: .ok)
            }
        }
        let connection = RemoteMacConnection(
            device: RemoteDevice(name: "Studio", muxy: MuxyRemoteServerData(host: "studio.local")),
            credentialStore: InMemoryRemoteMacCredentialStore(),
            socketFactory: { socket }
        )
        try await connection.connect(allowPairing: false)
        try await connection.selectProject(project.id)

        try socket.sendEvent(MuxyEvent(event: .projectsChanged, data: .projects([])))
        for _ in 0 ..< 10 where connection.activeProjectID != nil {
            await Task.yield()
        }

        #expect(connection.projects.isEmpty)
        #expect(connection.activeProjectID == nil)
        #expect(connection.workspace == nil)
    }

    private func workspace(projectID: UUID) -> WorkspaceDTO {
        let tab = TabDTO(id: UUID(), kind: .terminal, title: "Shell", isPinned: false, paneID: UUID())
        let area = TabAreaDTO(id: UUID(), projectPath: "/repo", tabs: [tab], activeTabID: tab.id)
        return WorkspaceDTO(
            projectID: projectID,
            worktreeID: UUID(),
            focusedAreaID: area.id,
            root: .tabArea(area)
        )
    }
}

@MainActor
private final class FakeRemoteMacSocket: RemoteMacSocket {
    var handler: (MuxyRequest) async -> MuxyResponse? = { _ in nil }
    private var queuedMessages: [Data] = []
    private var receiver: CheckedContinuation<Data, any Error>?
    private var isConnected = false

    func connect(to _: URL) {
        isConnected = true
    }

    func send(_ data: Data) async throws {
        guard isConnected else { throw RemoteMacConnectionError.disconnected }
        guard case let .request(request) = try MuxyCodec.decode(data) else {
            throw RemoteMacConnectionError.invalidMessage
        }
        guard let response = await handler(request) else { return }
        enqueue(try MuxyCodec.encode(.response(response)))
    }

    func receive() async throws -> Data {
        guard isConnected else { throw RemoteMacConnectionError.disconnected }
        if !queuedMessages.isEmpty {
            return queuedMessages.removeFirst()
        }
        return try await withCheckedThrowingContinuation { continuation in
            receiver = continuation
        }
    }

    func disconnect() {
        isConnected = false
        receiver?.resume(throwing: RemoteMacConnectionError.disconnected)
        receiver = nil
        queuedMessages.removeAll()
    }

    func sendEvent(_ event: MuxyEvent) throws {
        enqueue(try MuxyCodec.encode(.event(event)))
    }

    private func enqueue(_ data: Data) {
        guard let receiver else {
            queuedMessages.append(data)
            return
        }
        self.receiver = nil
        receiver.resume(returning: data)
    }
}
