import Foundation

struct ExtensionPanelSnapshot: Equatable {
    let extensionID: String
    let panelID: String
    let position: PanelPosition
    let mode: PanelMode
    let initialData: ExtensionJSON?
    let entry: String
    let title: String?
    let icon: ExtensionIcon?
    let hiddenControls: [PanelHeaderControl]
    let headerButtons: [ExtensionPanelHeaderButton]
    let hideTopbar: Bool

    init(
        extensionID: String,
        panelID: String,
        position: PanelPosition,
        mode: PanelMode,
        initialData: ExtensionJSON?,
        entry: String,
        title: String? = nil,
        icon: ExtensionIcon? = nil,
        hiddenControls: [PanelHeaderControl] = [],
        headerButtons: [ExtensionPanelHeaderButton] = [],
        hideTopbar: Bool = false
    ) {
        self.extensionID = extensionID
        self.panelID = panelID
        self.position = position
        self.mode = mode
        self.initialData = initialData
        self.entry = entry
        self.title = title
        self.icon = icon
        self.hiddenControls = hiddenControls
        self.headerButtons = headerButtons
        self.hideTopbar = hideTopbar
    }

    init(
        extensionID: String,
        panel: ExtensionPanel,
        position: PanelPosition,
        mode: PanelMode,
        initialData: ExtensionJSON?
    ) {
        self.init(
            extensionID: extensionID,
            panelID: panel.id,
            position: position,
            mode: mode,
            initialData: initialData,
            entry: panel.entry,
            title: panel.title,
            icon: panel.icon,
            hiddenControls: panel.hiddenControls,
            headerButtons: panel.headerButtons,
            hideTopbar: panel.hideTopbar
        )
    }

    var hostPanelID: String {
        ExtensionPanelState.hostPanelID(extensionID: extensionID, panelID: panelID)
    }
}

@MainActor
@Observable
final class ExtensionPanelRegistry {
    static let shared = ExtensionPanelRegistry()

    private(set) var openStates: [ExtensionPanelState] = []
    private(set) var activeProjectID: UUID?
    private var snapshotsByProject: [UUID: [ExtensionPanelSnapshot]] = [:]
    private let panelHost: PanelHost

    init(panelHost: PanelHost = .shared) {
        self.panelHost = panelHost
        panelHost.onDisplace = { [weak self] _ in self?.pruneClosed() }
    }

    func state(forHostPanelID hostPanelID: String) -> ExtensionPanelState? {
        openStates.first { $0.hostPanelID == hostPanelID }
    }

    func activateProject(_ projectID: UUID?, from previousProjectID: UUID?) {
        if previousProjectID == projectID, activeProjectID == projectID {
            return
        }

        if let previousProjectID {
            snapshotsByProject[previousProjectID] = capturedSnapshots(for: previousProjectID)
        }

        clearLiveExtensionPanels()
        activeProjectID = projectID

        guard let projectID else { return }
        var deferred: [ExtensionPanelSnapshot] = []
        for snapshot in snapshotsByProject.removeValue(forKey: projectID) ?? [] {
            guard restore(snapshot) else {
                deferred.append(snapshot)
                continue
            }
        }
        guard !deferred.isEmpty else { return }
        snapshotsByProject[projectID] = deferred
    }

    func purgeProject(_ projectID: UUID) {
        snapshotsByProject.removeValue(forKey: projectID)
        guard activeProjectID == projectID else { return }
        clearLiveExtensionPanels()
        activeProjectID = nil
    }

    @discardableResult
    func open(
        extensionID: String,
        panel: ExtensionPanel,
        data: ExtensionJSON?,
        position: PanelPosition? = nil,
        mode: PanelMode? = nil
    ) -> ExtensionPanelState {
        let hostPanelID = ExtensionPanelState.hostPanelID(extensionID: extensionID, panelID: panel.id)
        openStates.removeAll { $0.hostPanelID == hostPanelID }
        let state = ExtensionPanelState(
            extensionID: extensionID,
            panel: panel,
            initialData: data ?? panel.defaultData
        )
        openStates.append(state)
        panelHost.open(
            hostPanelID,
            at: position ?? panel.position,
            mode: mode ?? panel.mode,
            usesPreferredMode: panel.allowsModeSelection
        )
        ExtensionLifecycleEvents.panelOpened(extensionID: extensionID, panelID: panel.id)
        return state
    }

    func toggle(extensionID: String, panel: ExtensionPanel, data: ExtensionJSON?) {
        let hostPanelID = ExtensionPanelState.hostPanelID(extensionID: extensionID, panelID: panel.id)
        if panelHost.isOpen(hostPanelID) {
            forceClose(hostPanelID: hostPanelID)
            return
        }
        open(extensionID: extensionID, panel: panel, data: data)
    }

    func setMode(_ mode: PanelMode, forHostPanelID hostPanelID: String) {
        panelHost.setMode(mode, for: hostPanelID)
    }

    func move(_ position: PanelPosition, forHostPanelID hostPanelID: String) {
        panelHost.move(hostPanelID, to: position)
    }

    func close(hostPanelID: String) {
        guard let state = state(forHostPanelID: hostPanelID) else {
            PanelFocusRestoration.shared.restoreAfterClosing(panelID: hostPanelID)
            panelHost.close(hostPanelID)
            return
        }
        let surfaceKey = LifecycleSurfaceKey(kind: .panel, instanceID: state.id.uuidString)
        Task { @MainActor in
            let verdict = await ExtensionSurfaceBridgeRegistry.shared.requestBeforeClose(surfaceKey)
            guard verdict == .allow,
                  self.state(forHostPanelID: hostPanelID)?.id == state.id
            else { return }
            forceClose(hostPanelID: hostPanelID)
        }
    }

    func forceClose(hostPanelID: String) {
        let closed = openStates.filter { $0.hostPanelID == hostPanelID }
        PanelFocusRestoration.shared.restoreAfterClosing(panelID: hostPanelID)
        panelHost.close(hostPanelID)
        openStates.removeAll { $0.hostPanelID == hostPanelID }
        for state in closed {
            ExtensionLifecycleEvents.panelClosed(extensionID: state.extensionID, panelID: state.panelID)
        }
    }

    func forceClose(instanceID: String) {
        guard let state = openStates.first(where: { $0.id.uuidString == instanceID }) else { return }
        forceClose(hostPanelID: state.hostPanelID)
    }

    func closeAll(extensionID: String) {
        let closed = openStates.filter { $0.extensionID == extensionID }
        for state in closed {
            PanelFocusRestoration.shared.restoreAfterClosing(panelID: state.hostPanelID)
            panelHost.close(state.hostPanelID)
        }
        openStates.removeAll { $0.extensionID == extensionID }
        for state in closed {
            ExtensionLifecycleEvents.panelClosed(extensionID: state.extensionID, panelID: state.panelID)
        }
        for projectID in Array(snapshotsByProject.keys) {
            guard var snapshots = snapshotsByProject[projectID] else { continue }
            snapshots.removeAll { $0.extensionID == extensionID }
            if snapshots.isEmpty {
                snapshotsByProject.removeValue(forKey: projectID)
            } else {
                snapshotsByProject[projectID] = snapshots
            }
        }
    }

    func captureLiveSnapshots() -> [ExtensionPanelSnapshot] {
        openStates.compactMap { state in
            guard let placement = panelHost.placement(for: state.hostPanelID) else { return nil }
            let panel = ExtensionStore.shared.loadedExtension(id: state.extensionID)?
                .manifest.panel(id: state.panelID) ?? state.panel
            return ExtensionPanelSnapshot(
                extensionID: state.extensionID,
                panel: panel,
                position: placement.position,
                mode: placement.mode,
                initialData: state.initialData
            )
        }
    }

    private func capturedSnapshots(for projectID: UUID) -> [ExtensionPanelSnapshot] {
        let live = captureLiveSnapshots()
        let liveHostPanelIDs = Set(live.map(\.hostPanelID))
        let deferred = (snapshotsByProject[projectID] ?? [])
            .filter { !liveHostPanelIDs.contains($0.hostPanelID) }
        return live + deferred
    }

    private func restore(_ snapshot: ExtensionPanelSnapshot) -> Bool {
        guard let panel = panelForRestore(snapshot) else { return true }
        let usesPreferredMode = panel.allowsModeSelection
        let defaultMode = usesPreferredMode ? snapshot.mode : panel.mode
        let mode = panelHost.resolvedMode(
            for: snapshot.hostPanelID,
            default: defaultMode,
            usesPreferredMode: usesPreferredMode
        )
        guard !wouldDisplaceExtensionConsole(position: snapshot.position, mode: mode) else { return false }
        open(
            extensionID: snapshot.extensionID,
            panel: panel,
            data: snapshot.initialData,
            position: snapshot.position,
            mode: mode
        )
        return true
    }

    private func wouldDisplaceExtensionConsole(position: PanelPosition, mode: PanelMode) -> Bool {
        panelHost.panel(at: position, mode: mode)?.panelID == BuiltinPanel.extensionConsole
    }

    private func panelForRestore(_ snapshot: ExtensionPanelSnapshot) -> ExtensionPanel? {
        if let panel = ExtensionStore.shared.loadedExtension(id: snapshot.extensionID)?
            .manifest.panel(id: snapshot.panelID)
        {
            return panel
        }
        if ExtensionStore.shared.hasLoadedFromDisk {
            return nil
        }
        return ExtensionPanel(
            id: snapshot.panelID,
            title: snapshot.title,
            icon: snapshot.icon,
            entry: snapshot.entry,
            position: snapshot.position,
            mode: snapshot.mode,
            hiddenControls: snapshot.hiddenControls,
            headerButtons: snapshot.headerButtons,
            hideTopbar: snapshot.hideTopbar,
            defaultData: snapshot.initialData
        )
    }

    private func clearLiveExtensionPanels() {
        let closed = openStates
        for state in closed {
            PanelFocusRestoration.shared.discard(panelID: state.hostPanelID)
            panelHost.close(state.hostPanelID)
        }
        openStates = []
        for state in closed {
            ExtensionLifecycleEvents.panelClosed(extensionID: state.extensionID, panelID: state.panelID)
        }
    }

    private func pruneClosed() {
        let closed = openStates.filter { !panelHost.isOpen($0.hostPanelID) }
        for state in closed {
            PanelFocusRestoration.shared.restoreAfterClosing(panelID: state.hostPanelID)
        }
        openStates.removeAll { !panelHost.isOpen($0.hostPanelID) }
        for state in closed {
            ExtensionLifecycleEvents.panelClosed(extensionID: state.extensionID, panelID: state.panelID)
        }
    }
}
