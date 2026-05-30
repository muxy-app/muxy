import SwiftUI

struct ExtensionPanelView: View {
    let state: ExtensionPanelState
    let placement: PanelPlacement

    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore

    var body: some View {
        if let muxyExtension = ExtensionStore.shared.loadedExtension(id: state.extensionID),
           let panel = muxyExtension.manifest.panel(id: state.panelID),
           let entryURL = ExtensionWebView.entryURL(for: muxyExtension, entry: panel.entry)
        {
            PanelContainer(
                chrome: chrome(for: panel),
                mode: placement.mode,
                position: placement.position,
                onClose: { ExtensionPanelRegistry.shared.close(hostPanelID: state.hostPanelID) },
                onTogglePin: { togglePin() },
                onTogglePosition: { togglePosition() },
                content: {
                    ExtensionWebView(
                        extensionID: muxyExtension.id,
                        instanceID: state.id.uuidString,
                        entryURL: entryURL,
                        initialData: state.initialData,
                        appState: appState,
                        projectStore: projectStore,
                        worktreeStore: worktreeStore,
                        onFocus: {}
                    )
                }
            )
        }
    }

    private func chrome(for panel: ExtensionPanel) -> PanelChrome {
        PanelChrome(
            iconSymbol: panel.icon.flatMap(symbol(from:)),
            title: panel.title,
            hiddenControls: Set(panel.hiddenControls)
        )
    }

    private func symbol(from icon: ExtensionIcon) -> String? {
        guard case let .symbol(name) = icon else { return nil }
        return name
    }

    private func togglePin() {
        let nextMode: PanelMode = placement.mode == .pinned ? .floating : .pinned
        ExtensionPanelRegistry.shared.setMode(nextMode, forHostPanelID: state.hostPanelID)
    }

    private func togglePosition() {
        ExtensionPanelRegistry.shared.move(placement.position.opposite, forHostPanelID: state.hostPanelID)
    }
}
