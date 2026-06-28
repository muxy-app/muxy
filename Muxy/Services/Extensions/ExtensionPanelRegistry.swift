import Foundation

@MainActor
@Observable
final class ExtensionPanelRegistry {
    static let shared = ExtensionPanelRegistry()

    private(set) var openStates: [ExtensionPanelState] = []

    init() {
        PanelHost.shared.onDisplace = { [weak self] _ in self?.pruneClosed() }
    }

    func state(forHostPanelID hostPanelID: String) -> ExtensionPanelState? {
        openStates.first { $0.hostPanelID == hostPanelID }
    }

    @discardableResult
    func open(
        extensionID: String,
        panel: ExtensionPanel,
        data: ExtensionJSON?
    ) -> ExtensionPanelState {
        let hostPanelID = ExtensionPanelState.hostPanelID(extensionID: extensionID, panelID: panel.id)
        openStates.removeAll { $0.hostPanelID == hostPanelID }
        let state = ExtensionPanelState(
            extensionID: extensionID,
            panelID: panel.id,
            initialData: data ?? panel.defaultData
        )
        openStates.append(state)
        PanelHost.shared.open(hostPanelID, at: panel.position, mode: panel.mode)
        ExtensionLifecycleEvents.panelOpened(extensionID: extensionID, panelID: panel.id)
        return state
    }

    func toggle(extensionID: String, panel: ExtensionPanel, data: ExtensionJSON?) {
        let hostPanelID = ExtensionPanelState.hostPanelID(extensionID: extensionID, panelID: panel.id)
        if PanelHost.shared.isOpen(hostPanelID) {
            forceClose(hostPanelID: hostPanelID)
            return
        }
        open(extensionID: extensionID, panel: panel, data: data)
    }

    func setMode(_ mode: PanelMode, forHostPanelID hostPanelID: String) {
        PanelHost.shared.setMode(mode, for: hostPanelID)
    }

    func move(_ position: PanelPosition, forHostPanelID hostPanelID: String) {
        PanelHost.shared.move(hostPanelID, to: position)
    }

    func close(hostPanelID: String) {
        guard let state = state(forHostPanelID: hostPanelID) else {
            PanelHost.shared.close(hostPanelID)
            return
        }
        let surfaceKey = LifecycleSurfaceKey(kind: .panel, instanceID: state.id.uuidString)
        Task { @MainActor in
            let verdict = await ExtensionSurfaceBridgeRegistry.shared.requestBeforeClose(surfaceKey)
            guard verdict == .allow else { return }
            forceClose(hostPanelID: hostPanelID)
        }
    }

    func forceClose(hostPanelID: String) {
        let closed = openStates.filter { $0.hostPanelID == hostPanelID }
        PanelHost.shared.close(hostPanelID)
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
            PanelHost.shared.close(state.hostPanelID)
        }
        openStates.removeAll { $0.extensionID == extensionID }
        for state in closed {
            ExtensionLifecycleEvents.panelClosed(extensionID: state.extensionID, panelID: state.panelID)
        }
    }

    private func pruneClosed() {
        let closed = openStates.filter { !PanelHost.shared.isOpen($0.hostPanelID) }
        openStates.removeAll { !PanelHost.shared.isOpen($0.hostPanelID) }
        for state in closed {
            ExtensionLifecycleEvents.panelClosed(extensionID: state.extensionID, panelID: state.panelID)
        }
    }
}
