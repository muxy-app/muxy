import SwiftUI

struct ExtensionSidebarView: View {
    let extensionID: String

    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore

    var body: some View {
        if let muxyExtension = ExtensionStore.shared.loadedExtension(id: extensionID),
           let sidebar = muxyExtension.manifest.sidebar,
           let entryURL = ExtensionWebView.entryURL(for: muxyExtension, entry: sidebar.entry)
        {
            ExtensionWebView(
                extensionID: muxyExtension.id,
                instanceID: instanceID(for: muxyExtension.id, sidebarID: sidebar.id),
                surfaceKind: .sidebar,
                entryURL: entryURL,
                initialData: sidebar.defaultData,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore,
                focused: true,
                onFocus: {}
            )
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func instanceID(for extensionID: String, sidebarID: String) -> String {
        "sidebar:\(extensionID):\(sidebarID)"
    }
}
