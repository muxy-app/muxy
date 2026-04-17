import Foundation
import MuxyShared
import os
import SwiftUI
import UIKit

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
    var terminalTheme: TerminalTheme?
    var paneOwners: [UUID: PaneOwnerDTO] = [:]
    private(set) var savedDevices: [SavedDevice] = []
    private(set) var myClientID: UUID?

    var deviceName: String {
        UIDevice.current.name
    }

    func paneOwner(for paneID: UUID) -> PaneOwnerDTO? {
        paneOwners[paneID]
    }

    func paneIsOwnedBySelf(_ paneID: UUID) -> Bool {
        guard let myClientID, let owner = paneOwners[paneID] else { return false }
        if case let .remote(id, _) = owner, id == myClientID { return true }
        return false
    }

    struct TerminalTheme: Equatable {
        let fg: UInt32
        let bg: UInt32

        var fgColor: Color { Self.color(rgb: fg) }
        var bgColor: Color { Self.color(rgb: bg) }

        var isDark: Bool {
            let r = Double((bg >> 16) & 0xFF) / 255.0
            let g = Double((bg >> 8) & 0xFF) / 255.0
            let b = Double(bg & 0xFF) / 255.0
            let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b
            return luminance < 0.5
        }

        private static func color(rgb: UInt32) -> Color {
            Color(
                red: Double((rgb >> 16) & 0xFF) / 255.0,
                green: Double((rgb >> 8) & 0xFF) / 255.0,
                blue: Double(rgb & 0xFF) / 255.0
            )
        }
    }

    private var connection: URLSessionWebSocketTask?
    private var session: URLSession?
    private var pendingRequests: [String: CheckedContinuation<MuxyResponse, Never>] = [:]
    private var lastHost: String?
    private var lastPort: UInt16?

    var lastSavedHost: String? { savedDevices.first?.host }
    var lastSavedPort: UInt16? { savedDevices.first?.port }

    struct SavedDevice: Codable, Identifiable {
        var id: String { "\(host):\(port)" }
        let name: String
        let host: String
        let port: UInt16
    }

    init() {
        loadDevices()
    }

    func connect(host: String, port: UInt16 = 4865, name: String = "Mac") {
        lastHost = host
        lastPort = port
        addDevice(name: name, host: host, port: port)
        state = .connecting
        activeProjectID = nil
        workspace = nil
        paneOwners = [:]

        let url = URL(string: "ws://\(host):\(port)")!
        session = URLSession(configuration: .default)
        connection = session?.webSocketTask(with: url)
        connection?.resume()

        receiveLoop()

        Task {
            try? await Task.sleep(for: .milliseconds(500))
            await registerSelf()
            await refreshProjects()
            state = .connected
        }
    }

    private func registerSelf() async {
        let params = RegisterDeviceParams(deviceName: deviceName)
        guard let response = await send(.registerDevice, params: .registerDevice(params)) else { return }
        if case let .deviceInfo(info) = response.result {
            myClientID = info.clientID
        }
    }

    func takeOverPane(paneID: UUID, cols: UInt32, rows: UInt32) async {
        let params = TakeOverPaneParams(paneID: paneID, cols: cols, rows: rows)
        _ = await send(.takeOverPane, params: .takeOverPane(params))
    }

    func releasePane(paneID: UUID) async {
        let params = ReleasePaneParams(paneID: paneID)
        _ = await send(.releasePane, params: .releasePane(params))
    }

    func disconnect() {
        state = .disconnected
        connection?.cancel(with: .goingAway, reason: nil)
        connection = nil
        session = nil
        activeProjectID = nil
        workspace = nil
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
        workspace = nil
        paneOwners = [:]
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
        await refreshWorkspace(projectID: projectID)
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

    func resizeTerminal(paneID: UUID, cols: UInt32, rows: UInt32) async {
        let params = TerminalResizeParams(paneID: paneID, cols: cols, rows: rows)
        _ = await send(.terminalResize, params: .terminalResize(params))
    }

    func scrollTerminal(paneID: UUID, deltaX: Double, deltaY: Double, precise: Bool) async {
        let params = TerminalScrollParams(paneID: paneID, deltaX: deltaX, deltaY: deltaY, precise: precise)
        _ = await send(.terminalScroll, params: .terminalScroll(params))
    }

    func getTerminalCells(paneID: UUID) async -> TerminalCellsDTO? {
        let params = GetTerminalContentParams(paneID: paneID)
        guard let response = await send(.getTerminalContent, params: .getTerminalContent(params)) else { return nil }
        if case let .terminalCells(cells) = response.result {
            let theme = TerminalTheme(fg: cells.defaultFg, bg: cells.defaultBg)
            if terminalTheme != theme {
                terminalTheme = theme
            }
            return cells
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
                    guard case .connected = self.state else { return }
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
        case let .paneOwnership(dto):
            paneOwners[dto.paneID] = dto.owner
        case .tab,
             .terminalOutput:
            break
        }
    }

    private static let devicesKey = "savedDevices"

    func addDevice(name: String, host: String, port: UInt16) {
        let device = SavedDevice(name: name, host: host, port: port)
        savedDevices.removeAll { $0.host == host && $0.port == port }
        savedDevices.insert(device, at: 0)
        saveDevices()
    }

    func removeDevice(_ device: SavedDevice) {
        savedDevices.removeAll { $0.id == device.id }
        saveDevices()
    }

    private func saveDevices() {
        guard let data = try? JSONEncoder().encode(savedDevices) else { return }
        UserDefaults.standard.set(data, forKey: Self.devicesKey)
    }

    private func loadDevices() {
        guard let data = UserDefaults.standard.data(forKey: Self.devicesKey),
              let devices = try? JSONDecoder().decode([SavedDevice].self, from: data)
        else { return }
        savedDevices = devices
    }
}
