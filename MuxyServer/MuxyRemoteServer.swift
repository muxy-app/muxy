import Foundation
import MuxyShared
import Network
import os

private let logger = Logger(subsystem: "app.muxy", category: "RemoteServer")

public enum DeviceAuthDecision: Sendable {
    case approved(deviceName: String)
    case unknown
    case denied
}

public enum MuxyRemoteServerError: LocalizedError {
    case invalidPort(UInt16)
    case startSuperseded

    public var errorDescription: String? {
        switch self {
        case let .invalidPort(port):
            "Invalid port \(port)."
        case .startSuperseded:
            "Server start was superseded by a new start request."
        }
    }
}

@MainActor
public protocol MuxyRemoteServerDelegate: AnyObject {
    func listProjects() -> [ProjectDTO]
    func selectProject(_ projectID: UUID)
    func listWorktrees(projectID: UUID) -> [WorktreeDTO]
    func selectWorktree(projectID: UUID, worktreeID: UUID)
    func getWorkspace(projectID: UUID) -> WorkspaceDTO?
    func createTab(projectID: UUID, areaID: UUID?, kind: TabKindDTO) -> TabDTO?
    func closeTab(projectID: UUID, areaID: UUID, tabID: UUID)
    func selectTab(projectID: UUID, areaID: UUID, tabID: UUID)
    func splitArea(projectID: UUID, areaID: UUID, direction: SplitDirectionDTO, position: SplitPositionDTO)
    func closeArea(projectID: UUID, areaID: UUID)
    func focusArea(projectID: UUID, areaID: UUID)
    func sendTerminalInput(paneID: UUID, bytes: Data, clientID: UUID)
    func resizeTerminal(paneID: UUID, cols: UInt32, rows: UInt32, clientID: UUID)
    func scrollTerminal(paneID: UUID, deltaX: Double, deltaY: Double, precise: Bool, clientID: UUID)
    func getTerminalContent(paneID: UUID) -> TerminalCellsDTO?
    func takeOverPane(paneID: UUID, clientID: UUID, cols: UInt32, rows: UInt32)
    func releasePane(paneID: UUID, clientID: UUID)
    func registerDevice(clientID: UUID, name: String)
    func authenticateDevice(deviceID: UUID, token: String, name: String) -> DeviceAuthDecision
    func requestPairing(deviceID: UUID, token: String, name: String) async -> DeviceAuthDecision
    func getDeviceTheme() -> DeviceThemeEventDTO?
    func clientDisconnected(clientID: UUID)
    func getPaneOwner(paneID: UUID) -> PaneOwnerDTO?
    func getVCSStatus(projectID: UUID) async -> VCSStatusDTO?
    func vcsRefresh(projectID: UUID) async -> VCSStatusDTO?
    func vcsCommit(projectID: UUID, message: String, stageAll: Bool) async throws
    func vcsPush(projectID: UUID) async throws
    func vcsPull(projectID: UUID) async throws
    func vcsStageFiles(projectID: UUID, paths: [String]) async throws
    func vcsUnstageFiles(projectID: UUID, paths: [String]) async throws
    func vcsDiscardFiles(projectID: UUID, paths: [String], untrackedPaths: [String]) async throws
    func vcsListBranches(projectID: UUID) async throws -> VCSBranchesDTO
    func vcsSwitchBranch(projectID: UUID, branch: String) async throws
    func vcsCreateBranch(projectID: UUID, name: String) async throws
    func vcsCreatePR(projectID: UUID, title: String, body: String, baseBranch: String?, draft: Bool) async throws -> VCSCreatePRResultDTO
    func vcsAddWorktree(projectID: UUID, name: String, branch: String, createBranch: Bool) async throws -> WorktreeDTO
    func vcsRemoveWorktree(projectID: UUID, worktreeID: UUID) async throws
    func getProjectLogo(projectID: UUID) -> ProjectLogoDTO?
    func listNotifications() -> [NotificationDTO]
    func markNotificationRead(_ notificationID: UUID)
}

public final class MuxyRemoteServer: @unchecked Sendable {
    public static let defaultPort: UInt16 = 4865

    private let port: UInt16
    private var listener: NWListener?
    private var connections: [UUID: ClientConnection] = [:]
    private var authenticatedClients: Set<UUID> = []
    private var deviceIDByClient: [UUID: UUID] = [:]
    private let queue = DispatchQueue(label: "app.muxy.remoteServer")
    private var startCompletion: (@Sendable (Result<Void, Error>) -> Void)?
    private var stopCompletions: [@Sendable () -> Void] = []
    public weak var delegate: (any MuxyRemoteServerDelegate)?

    public init(port: UInt16 = MuxyRemoteServer.defaultPort) {
        self.port = port
    }

    public func start(completion: (@Sendable (Result<Void, Error>) -> Void)? = nil) {
        queue.async { [weak self] in
            guard let self else { return }
            self.finishStart(.failure(MuxyRemoteServerError.startSuperseded))
            self.startCompletion = completion
            self.startListener()
        }
    }

    public func stop(completion: (@Sendable () -> Void)? = nil) {
        queue.async { [weak self] in
            guard let self else {
                completion?()
                return
            }
            for connection in self.connections.values {
                connection.cancel()
            }
            self.connections.removeAll()
            self.authenticatedClients.removeAll()
            self.deviceIDByClient.removeAll()

            guard let listener = self.listener else {
                logger.info("Remote server stopped")
                completion?()
                return
            }
            if let completion { self.stopCompletions.append(completion) }
            listener.cancel()
        }
    }

    public func broadcast(_ event: MuxyEvent) {
        guard let data = try? MuxyCodec.encode(.event(event)) else { return }
        queue.async { [weak self] in
            guard let self else { return }
            for clientID in self.authenticatedClients {
                self.connections[clientID]?.send(data)
            }
        }
    }

    public func send(_ event: MuxyEvent, to clientID: UUID) {
        guard let data = try? MuxyCodec.encode(.event(event)) else { return }
        queue.async { [weak self] in
            guard let self,
                  self.authenticatedClients.contains(clientID)
            else { return }
            self.connections[clientID]?.send(data)
        }
    }

    public func disconnect(clientID: UUID) {
        queue.async { [weak self] in
            self?.connections[clientID]?.cancel()
        }
    }

    public func disconnect(deviceID: UUID) {
        queue.async { [weak self] in
            guard let self else { return }
            let clientIDs = self.deviceIDByClient.filter { $0.value == deviceID }.map(\.key)
            for clientID in clientIDs {
                self.connections[clientID]?.cancel()
            }
        }
    }

    private func startListener() {
        guard let endpointPort = NWEndpoint.Port(rawValue: port) else {
            logger.error("Invalid port: \(self.port)")
            finishStart(.failure(MuxyRemoteServerError.invalidPort(port)))
            return
        }

        do {
            let params = NWParameters.tcp
            params.allowLocalEndpointReuse = true
            let ws = NWProtocolWebSocket.Options()
            params.defaultProtocolStack.applicationProtocols.insert(ws, at: 0)
            listener = try NWListener(using: params, on: endpointPort)
        } catch {
            logger.error("Failed to create listener: \(error)")
            finishStart(.failure(error))
            return
        }

        listener?.stateUpdateHandler = { [weak self] state in
            guard let self else { return }
            switch state {
            case .ready:
                logger.info("Remote server listening on port \(self.port)")
                self.finishStart(.success(()))
            case let .failed(error):
                logger.error("Listener failed: \(error)")
                self.finishStart(.failure(error))
                self.listener?.cancel()
            case .cancelled:
                self.listener = nil
                logger.info("Remote server stopped")
                let completions = self.stopCompletions
                self.stopCompletions.removeAll()
                for completion in completions {
                    completion()
                }
            default:
                break
            }
        }

        listener?.newConnectionHandler = { [weak self] nwConnection in
            self?.handleNewConnection(nwConnection)
        }

        listener?.start(queue: queue)
    }

    private func finishStart(_ result: Result<Void, Error>) {
        guard let completion = startCompletion else { return }
        startCompletion = nil
        completion(result)
    }

    private func handleNewConnection(_ nwConnection: NWConnection) {
        let id = UUID()
        let connection = ClientConnection(id: id, connection: nwConnection, server: self)
        connections[id] = connection
        connection.start(on: queue)
        logger.info("Client connected: \(id)")
    }

    func removeConnection(_ id: UUID) {
        queue.async { [weak self] in
            self?.connections.removeValue(forKey: id)
            self?.authenticatedClients.remove(id)
            self?.deviceIDByClient.removeValue(forKey: id)
            logger.info("Client disconnected: \(id)")
        }
        Task { @MainActor in
            self.delegate?.clientDisconnected(clientID: id)
        }
    }

    private func markAuthenticated(_ id: UUID, deviceID: UUID) {
        queue.async { [weak self] in
            self?.authenticatedClients.insert(id)
            self?.deviceIDByClient[id] = deviceID
        }
    }

    func _testingMarkAuthenticated(_ id: UUID) {
        queue.sync {
            authenticatedClients.insert(id)
        }
    }

    private func isAuthenticated(_ id: UUID) -> Bool {
        queue.sync { authenticatedClients.contains(id) }
    }

    func handleRequest(_ request: MuxyRequest, from clientID: UUID) {
        if Self.voidMethods.contains(request.method) {
            Task { @MainActor in _ = await processRequest(request, clientID: clientID) }
            return
        }
        Task { @MainActor in
            let response = await processRequest(request, clientID: clientID)
            guard let data = try? MuxyCodec.encode(.response(response)) else { return }
            self.queue.async { [weak self] in
                self?.connections[clientID]?.send(data)
            }
        }
    }

    private static let voidMethods: Set<MuxyMethod> = [.terminalInput]

    @MainActor
    func processRequest(_ request: MuxyRequest, clientID: UUID) async -> MuxyResponse {
        guard let delegate else {
            return MuxyResponse(id: request.id, error: MuxyError.internalError)
        }

        switch request.method {
        case .pairDevice:
            guard case let .pairDevice(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            let decision = await delegate.requestPairing(
                deviceID: params.deviceID,
                token: params.token,
                name: params.deviceName
            )
            return finalizeAuth(
                requestID: request.id,
                clientID: clientID,
                deviceID: params.deviceID,
                decision: decision
            )

        case .authenticateDevice:
            guard case let .authenticateDevice(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            let decision = delegate.authenticateDevice(
                deviceID: params.deviceID,
                token: params.token,
                name: params.deviceName
            )
            return finalizeAuth(
                requestID: request.id,
                clientID: clientID,
                deviceID: params.deviceID,
                decision: decision
            )

        default:
            break
        }

        guard isAuthenticated(clientID) else {
            return MuxyResponse(id: request.id, error: .unauthorized)
        }

        switch request.method {
        case .pairDevice,
             .authenticateDevice:
            return MuxyResponse(id: request.id, error: .internalError)

        case .listProjects:
            let projects = delegate.listProjects()
            return MuxyResponse(id: request.id, result: .projects(projects))

        case .selectProject:
            guard case let .selectProject(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            delegate.selectProject(params.projectID)
            return MuxyResponse(id: request.id, result: .ok)

        case .listWorktrees:
            guard case let .listWorktrees(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            let worktrees = delegate.listWorktrees(projectID: params.projectID)
            return MuxyResponse(id: request.id, result: .worktrees(worktrees))

        case .selectWorktree:
            guard case let .selectWorktree(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            delegate.selectWorktree(projectID: params.projectID, worktreeID: params.worktreeID)
            return MuxyResponse(id: request.id, result: .ok)

        case .getWorkspace:
            guard case let .getWorkspace(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            guard let workspace = delegate.getWorkspace(projectID: params.projectID) else {
                return MuxyResponse(id: request.id, error: .notFound)
            }
            return MuxyResponse(id: request.id, result: .workspace(workspace))

        case .createTab:
            guard case let .createTab(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            guard let tab = delegate.createTab(projectID: params.projectID, areaID: params.areaID, kind: params.kind) else {
                return MuxyResponse(id: request.id, error: .internalError)
            }
            return MuxyResponse(id: request.id, result: .tab(tab))

        case .closeTab:
            guard case let .closeTab(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            delegate.closeTab(projectID: params.projectID, areaID: params.areaID, tabID: params.tabID)
            return MuxyResponse(id: request.id, result: .ok)

        case .selectTab:
            guard case let .selectTab(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            delegate.selectTab(projectID: params.projectID, areaID: params.areaID, tabID: params.tabID)
            return MuxyResponse(id: request.id, result: .ok)

        case .splitArea:
            guard case let .splitArea(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            delegate.splitArea(
                projectID: params.projectID,
                areaID: params.areaID,
                direction: params.direction,
                position: params.position
            )
            return MuxyResponse(id: request.id, result: .ok)

        case .closeArea:
            guard case let .closeArea(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            delegate.closeArea(projectID: params.projectID, areaID: params.areaID)
            return MuxyResponse(id: request.id, result: .ok)

        case .focusArea:
            guard case let .focusArea(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            delegate.focusArea(projectID: params.projectID, areaID: params.areaID)
            return MuxyResponse(id: request.id, result: .ok)

        case .terminalInput:
            guard case let .terminalInput(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            delegate.sendTerminalInput(paneID: params.paneID, bytes: params.bytes, clientID: clientID)
            return MuxyResponse(id: request.id, result: .ok)

        case .terminalResize:
            guard case let .terminalResize(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            delegate.resizeTerminal(
                paneID: params.paneID,
                cols: params.cols,
                rows: params.rows,
                clientID: clientID
            )
            return MuxyResponse(id: request.id, result: .ok)

        case .terminalScroll:
            guard case let .terminalScroll(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            delegate.scrollTerminal(
                paneID: params.paneID,
                deltaX: params.deltaX,
                deltaY: params.deltaY,
                precise: params.precise,
                clientID: clientID
            )
            return MuxyResponse(id: request.id, result: .ok)

        case .getTerminalContent:
            guard case let .getTerminalContent(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            guard let content = delegate.getTerminalContent(paneID: params.paneID) else {
                return MuxyResponse(id: request.id, error: .notFound)
            }
            return MuxyResponse(id: request.id, result: .terminalCells(content))

        case .getVCSStatus:
            guard case let .getVCSStatus(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            guard let status = await delegate.getVCSStatus(projectID: params.projectID) else {
                return MuxyResponse(id: request.id, error: .notFound)
            }
            return MuxyResponse(id: request.id, result: .vcsStatus(status))

        case .vcsRefresh:
            guard case let .vcsRefresh(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            guard let status = await delegate.vcsRefresh(projectID: params.projectID) else {
                return MuxyResponse(id: request.id, error: .notFound)
            }
            return MuxyResponse(id: request.id, result: .vcsStatus(status))

        case .vcsCommit:
            guard case let .vcsCommit(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            do {
                try await delegate.vcsCommit(projectID: params.projectID, message: params.message, stageAll: params.stageAll)
                return MuxyResponse(id: request.id, result: .ok)
            } catch {
                return MuxyResponse(id: request.id, error: MuxyError(code: 500, message: error.localizedDescription))
            }

        case .vcsPush:
            guard case let .vcsPush(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            do {
                try await delegate.vcsPush(projectID: params.projectID)
                return MuxyResponse(id: request.id, result: .ok)
            } catch {
                return MuxyResponse(id: request.id, error: MuxyError(code: 500, message: error.localizedDescription))
            }

        case .vcsPull:
            guard case let .vcsPull(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            do {
                try await delegate.vcsPull(projectID: params.projectID)
                return MuxyResponse(id: request.id, result: .ok)
            } catch {
                return MuxyResponse(id: request.id, error: MuxyError(code: 500, message: error.localizedDescription))
            }

        case .vcsStageFiles:
            guard case let .vcsStageFiles(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            do {
                try await delegate.vcsStageFiles(projectID: params.projectID, paths: params.paths)
                return MuxyResponse(id: request.id, result: .ok)
            } catch {
                return MuxyResponse(id: request.id, error: MuxyError(code: 500, message: error.localizedDescription))
            }

        case .vcsUnstageFiles:
            guard case let .vcsUnstageFiles(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            do {
                try await delegate.vcsUnstageFiles(projectID: params.projectID, paths: params.paths)
                return MuxyResponse(id: request.id, result: .ok)
            } catch {
                return MuxyResponse(id: request.id, error: MuxyError(code: 500, message: error.localizedDescription))
            }

        case .vcsDiscardFiles:
            guard case let .vcsDiscardFiles(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            do {
                try await delegate.vcsDiscardFiles(
                    projectID: params.projectID,
                    paths: params.paths,
                    untrackedPaths: params.untrackedPaths
                )
                return MuxyResponse(id: request.id, result: .ok)
            } catch {
                return MuxyResponse(id: request.id, error: MuxyError(code: 500, message: error.localizedDescription))
            }

        case .vcsListBranches:
            guard case let .vcsListBranches(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            do {
                let branches = try await delegate.vcsListBranches(projectID: params.projectID)
                return MuxyResponse(id: request.id, result: .vcsBranches(branches))
            } catch {
                return MuxyResponse(id: request.id, error: MuxyError(code: 500, message: error.localizedDescription))
            }

        case .vcsSwitchBranch:
            guard case let .vcsSwitchBranch(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            do {
                try await delegate.vcsSwitchBranch(projectID: params.projectID, branch: params.branch)
                return MuxyResponse(id: request.id, result: .ok)
            } catch {
                return MuxyResponse(id: request.id, error: MuxyError(code: 500, message: error.localizedDescription))
            }

        case .vcsCreateBranch:
            guard case let .vcsCreateBranch(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            do {
                try await delegate.vcsCreateBranch(projectID: params.projectID, name: params.name)
                return MuxyResponse(id: request.id, result: .ok)
            } catch {
                return MuxyResponse(id: request.id, error: MuxyError(code: 500, message: error.localizedDescription))
            }

        case .vcsCreatePR:
            guard case let .vcsCreatePR(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            do {
                let info = try await delegate.vcsCreatePR(
                    projectID: params.projectID,
                    title: params.title,
                    body: params.body,
                    baseBranch: params.baseBranch,
                    draft: params.draft
                )
                return MuxyResponse(id: request.id, result: .vcsPRCreated(info))
            } catch {
                return MuxyResponse(id: request.id, error: MuxyError(code: 500, message: error.localizedDescription))
            }

        case .vcsAddWorktree:
            guard case let .vcsAddWorktree(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            do {
                let worktree = try await delegate.vcsAddWorktree(
                    projectID: params.projectID,
                    name: params.name,
                    branch: params.branch,
                    createBranch: params.createBranch
                )
                return MuxyResponse(id: request.id, result: .worktrees([worktree]))
            } catch {
                return MuxyResponse(id: request.id, error: MuxyError(code: 500, message: error.localizedDescription))
            }

        case .vcsRemoveWorktree:
            guard case let .vcsRemoveWorktree(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            do {
                try await delegate.vcsRemoveWorktree(projectID: params.projectID, worktreeID: params.worktreeID)
                return MuxyResponse(id: request.id, result: .ok)
            } catch {
                return MuxyResponse(id: request.id, error: MuxyError(code: 500, message: error.localizedDescription))
            }

        case .getProjectLogo:
            guard case let .getProjectLogo(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            guard let logo = delegate.getProjectLogo(projectID: params.projectID) else {
                return MuxyResponse(id: request.id, error: .notFound)
            }
            return MuxyResponse(id: request.id, result: .projectLogo(logo))

        case .listNotifications:
            let notifications = delegate.listNotifications()
            return MuxyResponse(id: request.id, result: .notifications(notifications))

        case .markNotificationRead:
            guard case let .markNotificationRead(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            delegate.markNotificationRead(params.notificationID)
            return MuxyResponse(id: request.id, result: .ok)

        case .subscribe,
             .unsubscribe:
            return MuxyResponse(id: request.id, result: .ok)

        case .registerDevice:
            guard case let .registerDevice(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            delegate.registerDevice(clientID: clientID, name: params.deviceName)
            let theme = delegate.getDeviceTheme()
            let info = DeviceInfoDTO(
                clientID: clientID,
                deviceName: params.deviceName,
                themeFg: theme?.fg,
                themeBg: theme?.bg,
                themePalette: theme?.palette
            )
            return MuxyResponse(id: request.id, result: .deviceInfo(info))

        case .takeOverPane:
            guard case let .takeOverPane(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            delegate.takeOverPane(
                paneID: params.paneID,
                clientID: clientID,
                cols: params.cols,
                rows: params.rows
            )
            return MuxyResponse(id: request.id, result: .ok)

        case .releasePane:
            guard case let .releasePane(params) = request.params else {
                return MuxyResponse(id: request.id, error: .invalidParams)
            }
            delegate.releasePane(paneID: params.paneID, clientID: clientID)
            return MuxyResponse(id: request.id, result: .ok)
        }
    }

    @MainActor
    private func finalizeAuth(
        requestID: String,
        clientID: UUID,
        deviceID: UUID,
        decision: DeviceAuthDecision
    ) -> MuxyResponse {
        switch decision {
        case let .approved(deviceName):
            markAuthenticated(clientID, deviceID: deviceID)
            delegate?.registerDevice(clientID: clientID, name: deviceName)
            let theme = delegate?.getDeviceTheme()
            let result = PairingResultDTO(
                clientID: clientID,
                deviceName: deviceName,
                themeFg: theme?.fg,
                themeBg: theme?.bg,
                themePalette: theme?.palette
            )
            return MuxyResponse(id: requestID, result: .pairing(result))
        case .unknown:
            return MuxyResponse(id: requestID, error: .unauthorized)
        case .denied:
            return MuxyResponse(id: requestID, error: .pairingDenied)
        }
    }
}
