import SwiftUI

struct ExtensionTopbarItems: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @Environment(ExtensionStore.self) private var extensionStore
    @State private var popoverHost = PopoverHost.shared

    var body: some View {
        ForEach(extensionStore.topbarItems) { binding in
            ExtensionIconButton(
                icon: binding.displayIcon,
                muxyExtension: binding.muxyExtension,
                accessibilityLabel: binding.item.tooltip ?? binding.item.id,
                action: { triggerCommand(binding: binding) }
            )
            .help(binding.item.tooltip ?? binding.item.id)
            .extensionPopover(anchorID: binding.id, host: popoverHost)
        }
    }

    private func triggerCommand(binding: ExtensionStore.TopbarItemBinding) {
        if let popover = extensionStore.popover(for: binding.muxyExtension, command: binding.item.command) {
            popoverHost.toggle(
                anchorID: binding.id,
                extensionID: binding.muxyExtension.id,
                popover: popover,
                data: nil
            )
            return
        }
        extensionStore.triggerCommand(
            ExtensionStore.CommandInvocation(
                extensionID: binding.muxyExtension.id,
                commandID: binding.item.command,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore
            )
        )
    }
}
