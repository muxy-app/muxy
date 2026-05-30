import SwiftUI

struct ExtensionWebViewPane: View {
    let state: ExtensionTabState
    let focused: Bool
    let onFocus: () -> Void

    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore

    var body: some View {
        Group {
            if let muxyExtension = ExtensionStore.shared.loadedExtension(id: state.extensionID),
               let tabType = muxyExtension.manifest.tabType(id: state.tabTypeID),
               let entryURL = ExtensionWebView.entryURL(for: muxyExtension, entry: tabType.entry)
            {
                ExtensionWebView(
                    extensionID: muxyExtension.id,
                    instanceID: state.id.uuidString,
                    entryURL: entryURL,
                    initialData: state.initialData,
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore,
                    onFocus: onFocus
                )
                .contentShape(Rectangle())
                .onTapGesture { onFocus() }
            } else {
                placeholder
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .contentShape(Rectangle())
                    .onTapGesture { onFocus() }
            }
        }
    }

    private var placeholder: some View {
        VStack(spacing: 8) {
            Image(systemName: "puzzlepiece.extension")
                .font(.system(size: 32, weight: .light))
            Text("Extension \(state.extensionID) is not loaded")
                .font(.headline)
            Text("Tab type: \(state.tabTypeID)")
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
        .padding()
    }
}
