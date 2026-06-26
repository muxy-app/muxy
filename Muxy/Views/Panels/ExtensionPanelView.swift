import SwiftUI

struct ExtensionPanelView: View {
    let state: ExtensionPanelState
    let placement: PanelPlacement

    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @Environment(BrowserProfileStore.self) private var browserProfileStore: BrowserProfileStore?

    var body: some View {
        if let muxyExtension = ExtensionStore.shared.loadedExtension(id: state.extensionID),
           let panel = muxyExtension.manifest.panel(id: state.panelID),
           let entryURL = ExtensionWebView.entryURL(for: muxyExtension, entry: panel.entry)
        {
            PanelContainer(
                chrome: chrome(for: panel, in: muxyExtension),
                mode: placement.mode,
                position: placement.position,
                onClose: { ExtensionPanelRegistry.shared.close(hostPanelID: state.hostPanelID) },
                onTogglePin: { togglePin() },
                onTogglePosition: { togglePosition() },
                content: {
                    ExtensionWebView(
                        extensionID: muxyExtension.id,
                        instanceID: state.id.uuidString,
                        surfaceKind: .panel,
                        entryURL: entryURL,
                        initialData: state.initialData,
                        appState: appState,
                        projectStore: projectStore,
                        worktreeStore: worktreeStore,
                        projectGroupStore: projectGroupStore,
                        focused: true,
                        onFocus: {}
                    )
                }
            )
        }
    }

    private func chrome(for panel: ExtensionPanel, in muxyExtension: MuxyExtension) -> PanelChrome {
        PanelChrome(
            iconSymbol: panel.icon.flatMap(symbol(from:)),
            title: panel.title,
            hiddenControls: Set(panel.hiddenControls),
            trailingButtons: headerButtons(for: panel, in: muxyExtension),
            hidesHeader: panel.hideTopbar
        )
    }

    private func headerButtons(for panel: ExtensionPanel, in muxyExtension: MuxyExtension) -> [PanelHeaderButton] {
        panel.headerButtons.map { button in
            PanelHeaderButton(
                id: button.id,
                icon: .extensionIcon(button.icon, muxyExtension),
                label: button.tooltip ?? button.id,
                action: { trigger(command: button.command) }
            )
        }
    }

    private func trigger(command commandID: String) {
        ExtensionStore.shared.triggerCommand(
            ExtensionStore.CommandInvocation(
                extensionID: state.extensionID,
                commandID: commandID,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore,
                browserProfileStore: browserProfileStore
            )
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
