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
        return state
    }

    func toggle(extensionID: String, panel: ExtensionPanel, data: ExtensionJSON?) {
        let hostPanelID = ExtensionPanelState.hostPanelID(extensionID: extensionID, panelID: panel.id)
        if PanelHost.shared.isOpen(hostPanelID) {
            close(hostPanelID: hostPanelID)
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
        PanelHost.shared.close(hostPanelID)
        openStates.removeAll { $0.hostPanelID == hostPanelID }
    }

    func closeAll(extensionID: String) {
        for state in openStates where state.extensionID == extensionID {
            PanelHost.shared.close(state.hostPanelID)
        }
        openStates.removeAll { $0.extensionID == extensionID }
    }

    private func pruneClosed() {
        openStates.removeAll { !PanelHost.shared.isOpen($0.hostPanelID) }
    }
}
