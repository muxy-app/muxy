import Foundation
import MuxyShared
import os

private let workspaceLogger = Logger(subsystem: "app.muxy", category: "RemoteMacWorkspace")

@MainActor
@Observable
final class RemoteMacWorkspaceStore {
    typealias ConnectionFactory = @MainActor (RemoteDevice) -> RemoteMacConnection

    private(set) var activeDeviceID: UUID?
    private(set) var operationError: String?
    private var connections: [UUID: RemoteMacConnection] = [:]
    private var setupConnections: [UUID: RemoteMacConnection] = [:]
    private var activationGenerations: [UUID: UUID] = [:]
    @ObservationIgnored private var identityMaps: [UUID: RemoteWorkspaceIdentityMap] = [:]
    private let connectionFactory: ConnectionFactory
    private let credentialStore: any RemoteMacCredentialStoring

    init(
        credentialStore: any RemoteMacCredentialStoring = KeychainRemoteMacCredentialStore(),
        connectionFactory: ConnectionFactory? = nil
    ) {
        self.credentialStore = credentialStore
        self.connectionFactory = connectionFactory ?? { device in
            RemoteMacConnection(device: device, credentialStore: credentialStore)
        }
    }

    var activeConnection: RemoteMacConnection? {
        guard let activeDeviceID else { return nil }
        return connections[activeDeviceID]
    }

    var state: RemoteMacConnectionState {
        activeConnection?.state ?? .disconnected
    }

    var projects: [ProjectDTO] {
        activeConnection?.projects ?? []
    }

    var activeProject: ProjectDTO? {
        guard let activeProjectID = activeConnection?.activeProjectID else { return nil }
        return projects.first(where: { $0.id == activeProjectID })
    }

    var workspace: WorkspaceDTO? {
        activeConnection?.workspace
    }

    var presentedProjects: [Project] {
        guard let activeDeviceID, let connection = activeConnection else { return [] }
        return RemoteWorkspacePresentationBuilder.projects(
            from: connection.projects,
            deviceID: activeDeviceID,
            identities: identityMap(for: activeDeviceID)
        )
    }

    var presentedProject: Project? {
        guard let activeProjectID = activeConnection?.activeProjectID else { return nil }
        let presentationID = identityMapForActiveDevice?.presentationID(for: activeProjectID, entity: .project)
        return presentedProjects.first(where: { $0.id == presentationID })
    }

    var presentedWorkspace: RemoteWorkspacePresentation? {
        guard let workspace, let identities = identityMapForActiveDevice else { return nil }
        return RemoteWorkspacePresentationBuilder.workspace(from: workspace, identities: identities)
    }

    func connectionState(for deviceID: UUID) -> RemoteMacConnectionState {
        connections[deviceID]?.state ?? .disconnected
    }

    func activate(_ device: RemoteDevice, allowPairing: Bool = false) async {
        guard device.kind == .muxy else { return }
        if activeDeviceID == device.id,
           let connection = activeConnection,
           connection.state == .connecting || connection.state == .awaitingApproval
        {
            return
        }
        if activeDeviceID != device.id {
            if let activeDeviceID {
                activationGenerations[activeDeviceID] = UUID()
            }
            activeConnection?.disconnect()
            activeDeviceID = device.id
        }
        let generation = UUID()
        activationGenerations[device.id] = generation
        defer {
            if activationGenerations[device.id] == generation {
                activationGenerations.removeValue(forKey: device.id)
            }
        }
        operationError = nil
        let connection = connection(for: device)
        do {
            if !connection.isConnected {
                try await connection.connect(allowPairing: allowPairing)
            }
            if connection.activeProjectID == nil, let project = connection.projects.first {
                try await connection.selectProject(project.id)
            }
        } catch {
            guard activationGenerations[device.id] == generation,
                  activeDeviceID == device.id
            else { return }
            report(error)
        }
    }

    func connectForSetup(_ device: RemoteDevice) async throws {
        try Task.checkCancellation()
        operationError = nil
        setupConnections.removeValue(forKey: device.id)?.disconnect()
        let connection = connectionFactory(device)
        setupConnections[device.id] = connection
        defer {
            if setupConnections[device.id] === connection {
                setupConnections.removeValue(forKey: device.id)
            }
            connection.disconnect()
        }
        try Task.checkCancellation()
        try await connection.connect(allowPairing: true, loadProjects: false)
    }

    func cancelSetup(for deviceID: UUID, discardCredentialScope: String?) {
        setupConnections.removeValue(forKey: deviceID)?.disconnect()
        guard let discardCredentialScope else { return }
        do {
            try credentialStore.delete(for: deviceID, endpointScope: discardCredentialScope)
        } catch {
            workspaceLogger.error("Failed to delete setup credentials: \(error.localizedDescription)")
        }
    }

    func deactivate() {
        if let activeDeviceID {
            activationGenerations[activeDeviceID] = UUID()
        }
        activeConnection?.disconnect()
        activeDeviceID = nil
        operationError = nil
    }

    func retryActiveDevice(from deviceStore: RemoteDeviceStore) async {
        guard let device = deviceStore.device(id: activeDeviceID) else { return }
        await activate(device)
    }

    func selectProject(_ projectID: UUID) async {
        guard let connection = activeConnection else { return }
        operationError = nil
        do {
            try await connection.selectProject(projectID)
        } catch {
            report(error)
        }
    }

    func selectPresentedProject(_ projectID: UUID) async {
        guard let remoteID = identityMapForActiveDevice?.remoteID(for: projectID) else { return }
        await selectProject(remoteID)
    }

    func remoteID(for presentationID: UUID) -> UUID? {
        identityMapForActiveDevice?.remoteID(for: presentationID)
    }

    func workspaceActions() -> WorkspaceViewActions? {
        guard let connection = activeConnection,
              let workspace = connection.workspace
        else { return nil }
        let projectID = workspace.projectID
        return WorkspaceViewActions(
            projectID: presentedWorkspace?.projectID ?? projectID,
            focusArea: { [weak self, weak connection] areaID in
                self?.performPresentationAction(areaID: areaID) { remoteAreaID in
                    try await connection?.focusArea(projectID: projectID, areaID: remoteAreaID)
                }
            },
            selectTab: { [weak self, weak connection] areaID, tabID in
                self?.performPresentationAction(areaID: areaID, tabID: tabID) { remoteAreaID, remoteTabID in
                    try await connection?.selectTab(
                        projectID: projectID,
                        areaID: remoteAreaID,
                        tabID: remoteTabID
                    )
                }
            },
            createTab: { [weak self, weak connection] areaID in
                self?.performPresentationAction(areaID: areaID) { remoteAreaID in
                    try await connection?.createTab(projectID: projectID, areaID: remoteAreaID)
                }
            },
            closeTab: { [weak self, weak connection] areaID, tabID in
                self?.performPresentationAction(areaID: areaID, tabID: tabID) { remoteAreaID, remoteTabID in
                    try await connection?.closeTab(
                        projectID: projectID,
                        areaID: remoteAreaID,
                        tabID: remoteTabID
                    )
                }
            },
            forceCloseTab: { [weak self, weak connection] areaID, tabID in
                self?.performPresentationAction(areaID: areaID, tabID: tabID) { remoteAreaID, remoteTabID in
                    try await connection?.closeTab(
                        projectID: projectID,
                        areaID: remoteAreaID,
                        tabID: remoteTabID
                    )
                }
            },
            splitArea: { [weak self, weak connection] areaID, direction, position in
                self?.performPresentationAction(areaID: areaID) { remoteAreaID in
                    try await connection?.splitArea(
                        projectID: projectID,
                        areaID: remoteAreaID,
                        direction: direction == .horizontal ? .horizontal : .vertical,
                        position: position == .first ? .first : .second
                    )
                }
            },
            closeArea: { [weak self, weak connection] areaID in
                self?.performPresentationAction(areaID: areaID) { remoteAreaID in
                    try await connection?.closeArea(projectID: projectID, areaID: remoteAreaID)
                }
            }
        )
    }

    func refreshWorkspace() async {
        guard let connection = activeConnection,
              let projectID = connection.activeProjectID
        else { return }
        do {
            try await connection.refreshWorkspace(projectID: projectID)
        } catch {
            report(error)
        }
    }

    func removeDevice(_ deviceID: UUID) {
        activationGenerations[deviceID] = UUID()
        connections.removeValue(forKey: deviceID)?.disconnect()
        identityMaps.removeValue(forKey: deviceID)
        if activeDeviceID == deviceID {
            activeDeviceID = nil
            operationError = nil
        }
        do {
            try credentialStore.delete(for: deviceID)
        } catch {
            workspaceLogger.error("Failed to delete remote Mac credentials: \(error.localizedDescription)")
        }
    }

    func resetConnection(for deviceID: UUID, retiringCredentialScope: String? = nil) {
        activationGenerations[deviceID] = UUID()
        connections.removeValue(forKey: deviceID)?.disconnect()
        identityMaps.removeValue(forKey: deviceID)
        operationError = nil
        guard let retiringCredentialScope else { return }
        do {
            try credentialStore.delete(for: deviceID, endpointScope: retiringCredentialScope)
        } catch {
            workspaceLogger.error("Failed to retire remote Mac credentials: \(error.localizedDescription)")
        }
    }

    func clearOperationError() {
        operationError = nil
    }

    func performShortcutAction(_ action: ShortcutAction) -> Bool {
        guard activeDeviceID != nil else { return false }

        switch action {
        case .toggleThemePicker,
             .reloadConfig,
             .toggleSidebar,
             .toggleAppLayout,
             .toggleVoiceRecording,
             .toggleFullScreen,
             .toggleExtensionConsole:
            return false
        default:
            break
        }

        guard let connection = activeConnection,
              let workspace = connection.workspace,
              let area = RemoteWorkspaceNavigation.focusedArea(in: workspace)
        else { return true }

        if let index = action.tabSelectionIndex {
            guard area.tabs.indices.contains(index) else { return true }
            selectTab(area.tabs[index], in: area, workspace: workspace, connection: connection)
            return true
        }

        if let index = action.projectSelectionIndex {
            guard connection.projects.indices.contains(index) else { return true }
            Task { await selectProject(connection.projects[index].id) }
            return true
        }

        switch action {
        case .newTab:
            perform {
                try await connection.createTab(projectID: workspace.projectID, areaID: area.id)
            }
        case .closeTab:
            guard let tab = area.tabs.first(where: { $0.id == area.activeTabID }) else { return true }
            perform {
                try await connection.closeTab(projectID: workspace.projectID, areaID: area.id, tabID: tab.id)
            }
        case .splitRight:
            split(.horizontal, area: area, workspace: workspace, connection: connection)
        case .splitDown:
            split(.vertical, area: area, workspace: workspace, connection: connection)
        case .closePane:
            guard RemoteWorkspaceNavigation.areas(in: workspace.root).count > 1 else { return true }
            perform {
                try await connection.closeArea(projectID: workspace.projectID, areaID: area.id)
            }
        case .nextTab:
            selectTab(offset: 1, in: area, workspace: workspace, connection: connection)
        case .previousTab:
            selectTab(offset: -1, in: area, workspace: workspace, connection: connection)
        case .cycleNextTabAcrossPanes:
            selectTabAcrossPanes(offset: 1, workspace: workspace, connection: connection)
        case .cyclePreviousTabAcrossPanes:
            selectTabAcrossPanes(offset: -1, workspace: workspace, connection: connection)
        case .nextProject:
            selectProject(offset: 1, connection: connection)
        case .previousProject:
            selectProject(offset: -1, connection: connection)
        case .newHomeTab,
             .newBrowserTab,
             .renameTab,
             .pinUnpinTab,
             .focusPaneLeft,
             .focusPaneRight,
             .focusPaneUp,
             .focusPaneDown,
             .toggleMaximizePane,
             .findInTerminal,
             .terminalOmnibox,
             .terminalOmniboxProjects,
             .terminalOmniboxWorktrees,
             .terminalOmniboxWorkspaces,
             .terminalOmniboxCommands,
             .toggleRichInput,
             .submitRichInput,
             .submitRichInputWithoutReturn,
             .refreshWorktrees,
             .createWorktree,
             .removeCurrentWorktree,
             .openProject,
             .newProject,
             .navigateBack,
             .navigateForward,
             .inspectElement:
            break
        case .toggleThemePicker,
             .reloadConfig,
             .selectTab1,
             .selectTab2,
             .selectTab3,
             .selectTab4,
             .selectTab5,
             .selectTab6,
             .selectTab7,
             .selectTab8,
             .selectTab9,
             .selectProject1,
             .selectProject2,
             .selectProject3,
             .selectProject4,
             .selectProject5,
             .selectProject6,
             .selectProject7,
             .selectProject8,
             .selectProject9,
             .toggleSidebar,
             .toggleAppLayout,
             .toggleVoiceRecording,
             .toggleFullScreen,
             .toggleExtensionConsole:
            return false
        }
        return true
    }

    private func connection(for device: RemoteDevice) -> RemoteMacConnection {
        if let connection = connections[device.id] {
            connection.update(device: device)
            return connection
        }
        let connection = connectionFactory(device)
        connections[device.id] = connection
        return connection
    }

    private func split(
        _ direction: SplitDirectionDTO,
        area: TabAreaDTO,
        workspace: WorkspaceDTO,
        connection: RemoteMacConnection
    ) {
        perform {
            try await connection.splitArea(
                projectID: workspace.projectID,
                areaID: area.id,
                direction: direction,
                position: .second
            )
        }
    }

    private func selectTab(
        offset: Int,
        in area: TabAreaDTO,
        workspace: WorkspaceDTO,
        connection: RemoteMacConnection
    ) {
        guard let tab = RemoteWorkspaceNavigation.tab(in: area, offset: offset) else { return }
        selectTab(tab, in: area, workspace: workspace, connection: connection)
    }

    private func selectTabAcrossPanes(
        offset: Int,
        workspace: WorkspaceDTO,
        connection: RemoteMacConnection
    ) {
        guard let selection = RemoteWorkspaceNavigation.tabAcrossAreas(in: workspace, offset: offset) else { return }
        selectTab(selection.tab, in: selection.area, workspace: workspace, connection: connection)
    }

    private func selectTab(
        _ tab: TabDTO,
        in area: TabAreaDTO,
        workspace: WorkspaceDTO,
        connection: RemoteMacConnection
    ) {
        perform {
            if workspace.focusedAreaID != area.id {
                try await connection.focusArea(projectID: workspace.projectID, areaID: area.id)
            }
            try await connection.selectTab(projectID: workspace.projectID, areaID: area.id, tabID: tab.id)
        }
    }

    private func selectProject(offset: Int, connection: RemoteMacConnection) {
        guard !connection.projects.isEmpty else { return }
        let currentIndex = connection.projects.firstIndex { $0.id == connection.activeProjectID } ?? 0
        let index = (currentIndex + offset + connection.projects.count) % connection.projects.count
        Task { await selectProject(connection.projects[index].id) }
    }

    private func perform(_ operation: @escaping () async throws -> Void) {
        Task {
            do {
                try await operation()
            } catch {
                report(error)
                ToastState.shared.show(error.localizedDescription)
            }
        }
    }

    private func report(_ error: Error) {
        workspaceLogger.error("Remote workspace operation failed: \(error.localizedDescription)")
        operationError = error.localizedDescription
    }

    private func performPresentationAction(
        areaID: UUID,
        operation: @escaping (UUID) async throws -> Void
    ) {
        guard let remoteAreaID = remoteID(for: areaID) else { return }
        perform { try await operation(remoteAreaID) }
    }

    private func performPresentationAction(
        areaID: UUID,
        tabID: UUID,
        operation: @escaping (UUID, UUID) async throws -> Void
    ) {
        guard let remoteAreaID = remoteID(for: areaID),
              let remoteTabID = remoteID(for: tabID)
        else { return }
        perform { try await operation(remoteAreaID, remoteTabID) }
    }

    private var identityMapForActiveDevice: RemoteWorkspaceIdentityMap? {
        guard let activeDeviceID else { return nil }
        return identityMap(for: activeDeviceID)
    }

    private func identityMap(for deviceID: UUID) -> RemoteWorkspaceIdentityMap {
        if let existing = identityMaps[deviceID] {
            return existing
        }
        let identities = RemoteWorkspaceIdentityMap()
        identityMaps[deviceID] = identities
        return identities
    }
}
