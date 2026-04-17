import Foundation
import MuxyShared
import Network
import os

private let logger = Logger(subsystem: "app.muxy", category: "RemoteServer")

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
    func sendTerminalInput(paneID: UUID, text: String, clientID: UUID)
    func resizeTerminal(paneID: UUID, cols: UInt32, rows: UInt32, clientID: UUID)
    func scrollTerminal(paneID: UUID, deltaX: Double, deltaY: Double, precise: Bool, clientID: UUID)
    func getTerminalContent(paneID: UUID) -> TerminalCellsDTO?
    func takeOverPane(paneID: UUID, clientID: UUID, cols: UInt32, rows: UInt32)
    func releasePane(paneID: UUID, clientID: UUID)
    func registerDevice(clientID: UUID, name: String)
    func clientDisconnected(clientID: UUID)
    func getPaneOwner(paneID: UUID) -> PaneOwnerDTO?
    func getVCSStatus(projectID: UUID) async -> VCSStatusDTO?
    func vcsCommit(projectID: UUID, message: String, stageAll: Bool) async throws
    func vcsPush(projectID: UUID) async throws
    func vcsPull(projectID: UUID) async throws
    func getProjectLogo(projectID: UUID) -> ProjectLogoDTO?
    func listNotifications() -> [NotificationDTO]
    func markNotificationRead(_ notificationID: UUID)
}

public final class MuxyRemoteServer: @unchecked Sendable {
    private let port: UInt16
    private var listener: NWListener?
    private var connections: [UUID: ClientConnection] = [:]
    private let queue = DispatchQueue(label: "app.muxy.remoteServer")
    public weak var delegate: (any MuxyRemoteServerDelegate)?

    public init(port: UInt16 = 4865) {
        self.port = port
    }

    public func start() {
        queue.async { [weak self] in
            self?.startListener()
        }
    }

    public func stop() {
        queue.async { [weak self] in
            guard let self else { return }
            self.listener?.cancel()
            self.listener = nil
            for connection in self.connections.values {
                connection.cancel()
            }
            self.connections.removeAll()
            logger.info("Remote server stopped")
        }
    }

    public func broadcast(_ event: MuxyEvent) {
        guard let data = try? MuxyCodec.encode(.event(event)) else { return }
        queue.async { [weak self] in
            guard let self else { return }
            for connection in self.connections.values {
                connection.send(data)
            }
        }
    }

    private func startListener() {
        do {
            let params = NWParameters.tcp
            params.allowLocalEndpointReuse = true
            let ws = NWProtocolWebSocket.Options()
            params.defaultProtocolStack.applicationProtocols.insert(ws, at: 0)
            listener = try NWListener(using: params, on: NWEndpoint.Port(rawValue: port)!)
        } catch {
            logger.error("Failed to create listener: \(error)")
            return
        }

        listener?.stateUpdateHandler = { state in
            switch state {
            case .ready:
                logger.info("Remote server listening on port \(self.port)")
            case let .failed(error):
                logger.error("Listener failed: \(error)")
            default:
                break
            }
        }

        listener?.newConnectionHandler = { [weak self] nwConnection in
            self?.handleNewConnection(nwConnection)
        }

        listener?.start(queue: queue)
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
            logger.info("Client disconnected: \(id)")
        }
        Task { @MainActor in
            self.delegate?.clientDisconnected(clientID: id)
        }
    }

    func handleRequest(_ request: MuxyRequest, from clientID: UUID) {
        Task { @MainActor in
            let response = await processRequest(request, clientID: clientID)
            guard let data = try? MuxyCodec.encode(.response(response)) else { return }
            self.queue.async { [weak self] in
                self?.connections[clientID]?.send(data)
            }
        }
    }

    @MainActor
    func processRequest(_ request: MuxyRequest, clientID: UUID) async -> MuxyResponse {
        guard let delegate else {
            return MuxyResponse(id: request.id, error: MuxyError.internalError)
        }

        switch request.method {
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
            delegate.sendTerminalInput(paneID: params.paneID, text: params.text, clientID: clientID)
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
            let info = DeviceInfoDTO(clientID: clientID, deviceName: params.deviceName)
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
}
