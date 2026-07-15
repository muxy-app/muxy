import AppKit
import Foundation
import MuxyShared
import os

private let logger = Logger(subsystem: "app.muxy", category: "RemoteMacConnection")

enum RemoteMacConnectionState: Equatable {
    case disconnected
    case connecting
    case awaitingApproval
    case connected
    case failed(String)
}

enum RemoteMacConnectionError: LocalizedError {
    case disconnected
    case invalidAddress
    case invalidMessage
    case unexpectedResult(String)
    case requestTimedOut

    var errorDescription: String? {
        switch self {
        case .disconnected:
            "The remote Mac disconnected."
        case .invalidAddress:
            "The remote Mac address is invalid."
        case .invalidMessage:
            "The remote Mac sent an invalid message."
        case let .unexpectedResult(method):
            "The remote Mac returned an unexpected result for \(method)."
        case .requestTimedOut:
            "The remote Mac did not respond in time."
        }
    }
}

@MainActor
@Observable
final class RemoteMacConnection {
    typealias SocketFactory = @MainActor () -> any RemoteMacSocket

    private struct PendingRequest {
        let continuation: CheckedContinuation<MuxyResponse, Never>
        let timeoutTask: Task<Void, Never>
    }

    let deviceID: UUID
    private(set) var endpoint: MuxyRemoteServerData
    private(set) var deviceName: String
    private(set) var state: RemoteMacConnectionState = .disconnected
    private(set) var projects: [ProjectDTO] = []
    private(set) var activeProjectID: UUID?
    private(set) var workspace: WorkspaceDTO?
    private(set) var clientID: UUID?
    private(set) var deviceTheme: DeviceThemeEventDTO?
    private(set) var paneOwners: [UUID: PaneOwnerDTO] = [:]

    private let credentialStore: any RemoteMacCredentialStoring
    private let socketFactory: SocketFactory
    private var socket: (any RemoteMacSocket)?
    private var receiveTask: Task<Void, Never>?
    private var pendingRequests: [String: PendingRequest] = [:]
    private var eventObservers: [UUID: (MuxyEvent) -> Void] = [:]
    private var desiredOwnedPanes: Set<UUID> = []
    private var paneTakeoverGenerations: [UUID: UUID] = [:]
    private var shouldReportDisconnect = true
    private var connectionGeneration = UUID()
    private var projectSelectionGeneration = UUID()
    private var projectSelectionTargetID: UUID?

    init(
        device: RemoteDevice,
        credentialStore: any RemoteMacCredentialStoring,
        socketFactory: @escaping SocketFactory = { URLSessionRemoteMacSocket() }
    ) {
        precondition(device.kind == .muxy)
        guard let endpoint = device.muxy else { preconditionFailure() }
        deviceID = device.id
        self.endpoint = endpoint
        deviceName = device.displayName
        self.credentialStore = credentialStore
        self.socketFactory = socketFactory
    }

    var isConnected: Bool { state == .connected }

    func update(device: RemoteDevice) {
        guard device.id == deviceID, let endpoint = device.muxy else { return }
        self.endpoint = endpoint
        deviceName = device.displayName
    }

    func connect(allowPairing: Bool, loadProjects: Bool = true) async throws {
        disconnect()
        let generation = UUID()
        connectionGeneration = generation
        guard let url = endpoint.webSocketURL else {
            state = .failed(RemoteMacConnectionError.invalidAddress.localizedDescription)
            throw RemoteMacConnectionError.invalidAddress
        }

        state = .connecting
        shouldReportDisconnect = true
        let socket = socketFactory()
        self.socket = socket
        socket.connect(to: url)
        startReceiveLoop()

        do {
            let credentials = try credentialStore.loadOrCreate(
                for: deviceID,
                endpointScope: endpoint.credentialScope
            )
            do {
                try await authenticate(credentials)
            } catch let error as MuxyError where error.code == MuxyError.unauthorized.code && allowPairing {
                state = .awaitingApproval
                try await pair(credentials)
            }
            if loadProjects {
                try await refreshProjects()
            }
            state = .connected
        } catch {
            guard connectionGeneration == generation else { throw CancellationError() }
            fail(error)
            throw error
        }
    }

    func disconnect() {
        shouldReportDisconnect = false
        connectionGeneration = UUID()
        projectSelectionGeneration = UUID()
        projectSelectionTargetID = nil
        receiveTask?.cancel()
        receiveTask = nil
        socket?.disconnect()
        socket = nil
        cancelPendingRequests()
        state = .disconnected
        activeProjectID = nil
        workspace = nil
        clientID = nil
        deviceTheme = nil
        paneOwners.removeAll()
        desiredOwnedPanes.removeAll()
        paneTakeoverGenerations.removeAll()
    }

    func refreshProjects() async throws {
        guard case let .projects(projects) = try await request(.listProjects) else {
            throw RemoteMacConnectionError.unexpectedResult(MuxyMethod.listProjects.rawValue)
        }
        self.projects = sortedProjects(projects)
    }

    func selectProject(_ projectID: UUID) async throws {
        let generation = UUID()
        projectSelectionGeneration = generation
        projectSelectionTargetID = projectID
        defer {
            if projectSelectionGeneration == generation {
                projectSelectionTargetID = nil
            }
        }
        guard case .ok = try await request(
            .selectProject,
            params: .selectProject(SelectProjectParams(projectID: projectID))
        )
        else {
            throw RemoteMacConnectionError.unexpectedResult(MuxyMethod.selectProject.rawValue)
        }
        guard projectSelectionGeneration == generation else { return }
        activeProjectID = projectID
        workspace = nil
        paneOwners.removeAll()
        let workspace = try await workspace(projectID: projectID)
        guard projectSelectionGeneration == generation,
              activeProjectID == projectID
        else { return }
        self.workspace = workspace
    }

    func refreshWorkspace(projectID: UUID) async throws {
        let workspace = try await workspace(projectID: projectID)
        guard activeProjectID == projectID else { return }
        self.workspace = workspace
    }

    private func workspace(projectID: UUID) async throws -> WorkspaceDTO {
        guard case let .workspace(workspace) = try await request(
            .getWorkspace,
            params: .getWorkspace(GetWorkspaceParams(projectID: projectID))
        )
        else {
            throw RemoteMacConnectionError.unexpectedResult(MuxyMethod.getWorkspace.rawValue)
        }
        return workspace
    }

    func createTab(projectID: UUID, areaID: UUID?) async throws {
        guard case .tab = try await request(
            .createTab,
            params: .createTab(CreateTabParams(projectID: projectID, areaID: areaID))
        )
        else {
            throw RemoteMacConnectionError.unexpectedResult(MuxyMethod.createTab.rawValue)
        }
    }

    func closeTab(projectID: UUID, areaID: UUID, tabID: UUID) async throws {
        try await requestOK(
            .closeTab,
            params: .closeTab(CloseTabParams(projectID: projectID, areaID: areaID, tabID: tabID))
        )
    }

    func selectTab(projectID: UUID, areaID: UUID, tabID: UUID) async throws {
        try await requestOK(
            .selectTab,
            params: .selectTab(SelectTabParams(projectID: projectID, areaID: areaID, tabID: tabID))
        )
    }

    func splitArea(projectID: UUID, areaID: UUID, direction: SplitDirectionDTO, position: SplitPositionDTO) async throws {
        try await requestOK(
            .splitArea,
            params: .splitArea(SplitAreaParams(
                projectID: projectID,
                areaID: areaID,
                direction: direction,
                position: position
            ))
        )
    }

    func closeArea(projectID: UUID, areaID: UUID) async throws {
        try await requestOK(
            .closeArea,
            params: .closeArea(CloseAreaParams(projectID: projectID, areaID: areaID))
        )
    }

    func focusArea(projectID: UUID, areaID: UUID) async throws {
        try await requestOK(
            .focusArea,
            params: .focusArea(FocusAreaParams(projectID: projectID, areaID: areaID))
        )
    }

    func takeOverPane(paneID: UUID, cols: UInt32, rows: UInt32) -> Task<Void, any Error> {
        let generation = UUID()
        desiredOwnedPanes.insert(paneID)
        paneTakeoverGenerations[paneID] = generation
        return Task { @MainActor [weak self] in
            guard let self else { throw CancellationError() }
            do {
                guard case .ok = try await request(
                    .takeOverPane,
                    params: .takeOverPane(TakeOverPaneParams(paneID: paneID, cols: cols, rows: rows))
                )
                else {
                    throw RemoteMacConnectionError.unexpectedResult(MuxyMethod.takeOverPane.rawValue)
                }
            } catch {
                if paneTakeoverGenerations[paneID] == generation {
                    paneTakeoverGenerations.removeValue(forKey: paneID)
                    desiredOwnedPanes.remove(paneID)
                }
                throw error
            }
            if paneTakeoverGenerations[paneID] == generation {
                paneTakeoverGenerations.removeValue(forKey: paneID)
            }
            if !desiredOwnedPanes.contains(paneID) {
                await releasePane(paneID: paneID)
            }
        }
    }

    func releasePane(paneID: UUID) async {
        desiredOwnedPanes.remove(paneID)
        paneTakeoverGenerations.removeValue(forKey: paneID)
        _ = try? await request(
            .releasePane,
            params: .releasePane(ReleasePaneParams(paneID: paneID))
        )
        paneOwners.removeValue(forKey: paneID)
    }

    func resizeTerminal(paneID: UUID, cols: UInt32, rows: UInt32) async throws {
        try await requestOK(
            .terminalResize,
            params: .terminalResize(TerminalResizeParams(paneID: paneID, cols: cols, rows: rows))
        )
    }

    func sendTerminalInput(paneID: UUID, bytes: Data) {
        sendFireAndForget(
            .terminalInput,
            params: .terminalInput(TerminalInputParams(paneID: paneID, bytes: bytes))
        )
    }

    @discardableResult
    func addEventObserver(_ observer: @escaping (MuxyEvent) -> Void) -> UUID {
        let id = UUID()
        eventObservers[id] = observer
        return id
    }

    func removeEventObserver(_ id: UUID) {
        eventObservers.removeValue(forKey: id)
    }

    func isPaneOwnedByThisMac(_ paneID: UUID) -> Bool {
        guard let clientID, case let .remote(ownerID, _)? = paneOwners[paneID] else { return false }
        return clientID == ownerID
    }

    private func authenticate(_ credentials: RemoteMacCredentials) async throws {
        let params = AuthenticateDeviceParams(
            deviceID: credentials.deviceID,
            deviceName: Host.current().localizedName ?? "Mac",
            token: credentials.token,
            clientKind: .desktop
        )
        try await applyPairingResult(request(.authenticateDevice, params: .authenticateDevice(params)))
    }

    private func pair(_ credentials: RemoteMacCredentials) async throws {
        let params = PairDeviceParams(
            deviceID: credentials.deviceID,
            deviceName: Host.current().localizedName ?? "Mac",
            token: credentials.token,
            clientKind: .desktop
        )
        try await applyPairingResult(request(
            .pairDevice,
            params: .pairDevice(params),
            timeout: .seconds(120)
        ))
    }

    private func applyPairingResult(_ result: MuxyResult) throws {
        guard case let .pairing(info) = result else {
            throw RemoteMacConnectionError.unexpectedResult("authentication")
        }
        clientID = info.clientID
        if let fg = info.themeFg, let bg = info.themeBg {
            deviceTheme = DeviceThemeEventDTO(fg: fg, bg: bg, palette: info.themePalette)
        }
    }

    private func request(
        _ method: MuxyMethod,
        params: MuxyParams? = nil,
        timeout: Duration = .seconds(15)
    ) async throws -> MuxyResult {
        guard let socket else { throw RemoteMacConnectionError.disconnected }
        let id = UUID().uuidString
        let message = MuxyMessage.request(MuxyRequest(id: id, method: method, params: params))
        let data = try MuxyCodec.encode(message)

        let response = await withCheckedContinuation { continuation in
            let timeoutTask = Task { @MainActor [weak self] in
                try? await Task.sleep(for: timeout)
                guard !Task.isCancelled else { return }
                self?.resolveRequest(
                    id: id,
                    response: MuxyResponse(id: id, error: .timeout)
                )
            }
            pendingRequests[id] = PendingRequest(continuation: continuation, timeoutTask: timeoutTask)
            Task { @MainActor [weak self] in
                do {
                    try await socket.send(data)
                } catch {
                    self?.resolveRequest(
                        id: id,
                        response: MuxyResponse(id: id, error: MuxyError(code: 503, message: error.localizedDescription))
                    )
                }
            }
        }

        if let error = response.error {
            if error.code == MuxyError.timeout.code { throw RemoteMacConnectionError.requestTimedOut }
            throw error
        }
        guard let result = response.result else {
            throw RemoteMacConnectionError.unexpectedResult(method.rawValue)
        }
        return result
    }

    private func requestOK(_ method: MuxyMethod, params: MuxyParams) async throws {
        guard case .ok = try await request(method, params: params) else {
            throw RemoteMacConnectionError.unexpectedResult(method.rawValue)
        }
    }

    private func sendFireAndForget(_ method: MuxyMethod, params: MuxyParams) {
        guard let socket,
              let data = try? MuxyCodec.encode(.request(MuxyRequest(
                  id: UUID().uuidString,
                  method: method,
                  params: params
              )))
        else { return }
        Task {
            do {
                try await socket.send(data)
            } catch {
                logger.error("Failed to send \(method.rawValue): \(error.localizedDescription)")
            }
        }
    }

    private func startReceiveLoop() {
        receiveTask?.cancel()
        let generation = connectionGeneration
        receiveTask = Task { @MainActor [weak self] in
            guard let self, let socket else { return }
            do {
                while !Task.isCancelled {
                    let data = try await socket.receive()
                    try handle(MuxyCodec.decode(data))
                }
            } catch is CancellationError {
                return
            } catch {
                guard connectionGeneration == generation, shouldReportDisconnect else { return }
                fail(error)
            }
        }
    }

    private func handle(_ message: MuxyMessage) throws {
        switch message {
        case let .response(response):
            resolveRequest(id: response.id, response: response)
        case let .event(event):
            handle(event)
        case .request:
            throw RemoteMacConnectionError.invalidMessage
        }
    }

    private func handle(_ event: MuxyEvent) {
        switch event.data {
        case let .projects(projects):
            let projects = sortedProjects(projects)
            self.projects = projects
            let projectIDs = Set(projects.map(\.id))
            if let projectSelectionTargetID, !projectIDs.contains(projectSelectionTargetID) {
                projectSelectionGeneration = UUID()
                self.projectSelectionTargetID = nil
            }
            if let activeProjectID, !projectIDs.contains(activeProjectID) {
                self.activeProjectID = nil
                workspace = nil
                paneOwners.removeAll()
            }
        case let .workspace(workspace):
            guard workspace.projectID == activeProjectID else { break }
            self.workspace = workspace
        case let .paneOwnership(ownership):
            paneOwners[ownership.paneID] = ownership.owner
        case let .deviceTheme(theme):
            deviceTheme = theme
        case .terminalOutput,
             .terminalSnapshot,
             .notification:
            break
        }
        for observer in Array(eventObservers.values) {
            observer(event)
        }
    }

    private func sortedProjects(_ projects: [ProjectDTO]) -> [ProjectDTO] {
        projects.sorted {
            if $0.sortOrder != $1.sortOrder { return $0.sortOrder < $1.sortOrder }
            return $0.name.localizedStandardCompare($1.name) == .orderedAscending
        }
    }

    private func resolveRequest(id: String, response: MuxyResponse) {
        guard let pending = pendingRequests.removeValue(forKey: id) else { return }
        pending.timeoutTask.cancel()
        pending.continuation.resume(returning: response)
    }

    private func cancelPendingRequests() {
        let requests = pendingRequests
        pendingRequests.removeAll()
        for (id, pending) in requests {
            pending.timeoutTask.cancel()
            pending.continuation.resume(returning: MuxyResponse(
                id: id,
                error: MuxyError(code: 499, message: "Connection closed")
            ))
        }
    }

    private func fail(_ error: Error) {
        logger.error("Remote Mac connection failed: \(error.localizedDescription)")
        connectionGeneration = UUID()
        projectSelectionGeneration = UUID()
        projectSelectionTargetID = nil
        state = .failed(error.localizedDescription)
        socket?.disconnect()
        socket = nil
        cancelPendingRequests()
        projects.removeAll()
        activeProjectID = nil
        workspace = nil
        clientID = nil
        deviceTheme = nil
        paneOwners.removeAll()
        desiredOwnedPanes.removeAll()
        paneTakeoverGenerations.removeAll()
    }
}
