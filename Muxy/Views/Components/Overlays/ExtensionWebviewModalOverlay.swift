import AppKit
import SwiftUI

struct ExtensionWebviewModalOverlay: View {
    let request: ExtensionWebviewModalService.Request
    let onDismiss: () -> Void

    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore

    @State private var keyMonitor: Any?

    var body: some View {
        ZStack {
            Color.black.opacity(0.3)
                .ignoresSafeArea()
                .onTapGesture {
                    guard request.dismissOnOutsideClick else { return }
                    onDismiss()
                }

            OverlayPanel(
                width: UIMetrics.scaled(request.width),
                height: UIMetrics.scaled(request.height),
                verticalAlignment: .center
            ) {
                content
            }
        }
        .onAppear { installKeyMonitor() }
        .onDisappear { removeKeyMonitor() }
    }

    @ViewBuilder
    private var content: some View {
        if let muxyExtension = ExtensionStore.shared.loadedExtension(id: request.extensionID),
           let entryURL = ExtensionWebView.entryURL(for: muxyExtension, entry: request.entry)
        {
            ExtensionWebView(
                extensionID: muxyExtension.id,
                instanceID: request.id,
                surfaceKind: .modalWebview,
                entryURL: entryURL,
                initialData: request.initialData,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore,
                focused: true,
                onFocus: {}
            )
        } else {
            Color.clear
        }
    }

    private func installKeyMonitor() {
        guard keyMonitor == nil else { return }
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            guard event.keyCode == 53 else { return event }
            onDismiss()
            return nil
        }
    }

    private func removeKeyMonitor() {
        guard let keyMonitor else { return }
        NSEvent.removeMonitor(keyMonitor)
        self.keyMonitor = nil
    }
}
