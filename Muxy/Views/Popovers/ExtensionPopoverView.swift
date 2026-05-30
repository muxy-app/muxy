import SwiftUI

struct ExtensionPopoverView: View {
    let state: ExtensionPopoverState

    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore

    var body: some View {
        Group {
            if let muxyExtension = ExtensionStore.shared.loadedExtension(id: state.extensionID),
               let popover = muxyExtension.manifest.popover(id: state.popoverID),
               let entryURL = ExtensionWebView.entryURL(for: muxyExtension, entry: popover.entry)
            {
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
            } else {
                Color.clear
            }
        }
        .frame(width: state.width, height: state.height)
    }
}

extension View {
    func extensionPopover(anchorID: String, host: PopoverHost) -> some View {
        let item = Binding<ExtensionPopoverState?>(
            get: { host.isOpen(anchorID: anchorID) ? host.open?.state : nil },
            set: { newValue in if newValue == nil { host.close(anchorID: anchorID) } }
        )
        return popover(item: item, arrowEdge: .bottom) { state in
            ExtensionPopoverView(state: state)
        }
    }
}
