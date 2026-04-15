import Foundation
import MuxyShared
import os

private let logger = Logger(subsystem: "app.muxy.mobile", category: "Connection")

@MainActor
@Observable
final class ConnectionManager {
    enum State {
        case disconnected
        case connecting
        case connected
        case error(String)
    }

    var state: State = .disconnected
    var projects: [ProjectDTO] = []
    var activeProjectID: UUID?
    var worktrees: [WorktreeDTO] = []
    var workspace: WorkspaceDTO?
    var notifications: [NotificationDTO] = []
    var projectLogos: [UUID: Data] = [:]
    var projectWorktrees: [UUID: [WorktreeDTO]] = [:]
    private(set) var lastSavedHost: String?
    private(set) var lastSavedPort: UInt16?

    private var connection: URLSessionWebSocketTask?
    private var session: URLSession?
    private var pendingRequests: [String: CheckedContinuation<MuxyResponse, Never>] = [:]
    private var lastHost: String?
    private var lastPort: UInt16?

    func connect(host: String, port: UInt16 = 4865) {
        lastHost = host
        lastPort = port
        lastSavedHost = host
        lastSavedPort = port
        state = .connecting

        let url = URL(string: "ws://\(host):\(port)")!
        session = URLSession(configuration: .default)
        connection = session?.webSocketTask(with: url)
        connection?.resume()

        receiveLoop()

        Task {
            try? await Task.sleep(for: .milliseconds(500))
            await refreshProjects()
            state = .connected
        }
    }

    func disconnect() {
        connection?.cancel(with: .goingAway, reason: nil)
        connection = nil
        session = nil
        state = .disconnected
    }

    func reconnect() {
        guard let host = lastHost, let port = lastPort else { return }
        connect(host: host, port: port)
    }

    func refreshProjects() async {
        guard let response = await send(.listProjects) else { return }
        if case let .projects(list) = response.result {
            projects = list
            for project in list {
                if project.logo != nil {
                    await fetchLogo(for: project.id)
                }
                await refreshWorktrees(projectID: project.id)
            }
        }
    }

    func fetchLogo(for projectID: UUID) async {
        guard projectLogos[projectID] == nil else { return }
        let params = GetProjectLogoParams(projectID: projectID)
        guard let response = await send(.getProjectLogo, params: .getProjectLogo(params)),
              case let .projectLogo(logo) = response.result,
              let data = Data(base64Encoded: logo.pngData)
        else { return }
        projectLogos[projectID] = data
    }

    func selectProject(_ projectID: UUID) async {
        activeProjectID = projectID
        let params = SelectProjectParams(projectID: projectID)
        _ = await send(.selectProject, params: .selectProject(params))
        await refreshWorkspace(projectID: projectID)
    }

    func refreshWorktrees(projectID: UUID) async {
        let params = ListWorktreesParams(projectID: projectID)
        guard let response = await send(.listWorktrees, params: .listWorktrees(params)) else { return }
        if case let .worktrees(list) = response.result {
            worktrees = list
            projectWorktrees[projectID] = list
        }
    }

    func refreshWorkspace(projectID: UUID) async {
        let params = GetWorkspaceParams(projectID: projectID)
        guard let response = await send(.getWorkspace, params: .getWorkspace(params)) else { return }
        if case let .workspace(ws) = response.result {
            workspace = ws
        }
    }

    func createTab(projectID: UUID, areaID: UUID? = nil) async {
        let params = CreateTabParams(projectID: projectID, areaID: areaID)
        _ = await send(.createTab, params: .createTab(params))
        await refreshWorkspace(projectID: projectID)
    }

    func selectTab(projectID: UUID, areaID: UUID, tabID: UUID) async {
        let params = SelectTabParams(projectID: projectID, areaID: areaID, tabID: tabID)
        _ = await send(.selectTab, params: .selectTab(params))
    }

    func closeTab(projectID: UUID, areaID: UUID, tabID: UUID) async {
        let params = CloseTabParams(projectID: projectID, areaID: areaID, tabID: tabID)
        _ = await send(.closeTab, params: .closeTab(params))
        await refreshWorkspace(projectID: projectID)
    }

    func sendTerminalInput(paneID: UUID, text: String) async {
        let params = TerminalInputParams(paneID: paneID, text: text)
        _ = await send(.terminalInput, params: .terminalInput(params))
    }

    func getTerminalContent(paneID: UUID) async -> TerminalContentDTO? {
        let params = GetTerminalContentParams(paneID: paneID)
        guard let response = await send(.getTerminalContent, params: .getTerminalContent(params)) else { return nil }
        if case let .terminalContent(content) = response.result {
            return content
        }
        return nil
    }

    private func send(_ method: MuxyMethod, params: MuxyParams? = nil) async -> MuxyResponse? {
        let id = UUID().uuidString
        let request = MuxyRequest(id: id, method: method, params: params)
        let message = MuxyMessage.request(request)

        guard let data = try? MuxyCodec.encode(message),
              let text = String(data: data, encoding: .utf8)
        else { return nil }

        do {
            try await connection?.send(.string(text))
        } catch {
            logger.error("Send failed: \(error)")
            state = .error("Connection lost")
            return nil
        }

        return await withCheckedContinuation { continuation in
            pendingRequests[id] = continuation
            Task {
                try? await Task.sleep(for: .seconds(10))
                if let pending = pendingRequests.removeValue(forKey: id) {
                    pending.resume(returning: MuxyResponse(id: id, error: MuxyError(code: 408, message: "Timeout")))
                }
            }
        }
    }

    private func receiveLoop() {
        connection?.receive { [weak self] result in
            Task { @MainActor in
                guard let self else { return }
                switch result {
                case let .success(message):
                    self.handleMessage(message)
                    self.receiveLoop()
                case let .failure(error):
                    logger.error("Receive failed: \(error)")
                    self.state = .error("Connection lost")
                }
            }
        }
    }

    private func handleMessage(_ message: URLSessionWebSocketTask.Message) {
        let data: Data
        switch message {
        case let .string(text): data = Data(text.utf8)
        case let .data(d): data = d
        @unknown default: return
        }

        guard let muxyMessage = try? MuxyCodec.decode(data) else { return }

        switch muxyMessage {
        case let .response(response):
            if let continuation = pendingRequests.removeValue(forKey: response.id) {
                continuation.resume(returning: response)
            }
        case let .event(event):
            handleEvent(event)
        case .request:
            break
        }
    }

    private func handleEvent(_ event: MuxyEvent) {
        switch event.data {
        case let .projects(list):
            projects = list
        case let .workspace(ws):
            workspace = ws
        case let .notification(notification):
            notifications.insert(notification, at: 0)
        case .tab,
             .terminalOutput:
            break
        }
    }
}
