import AppKit
import SwiftUI

struct TerminalPane: View {
    let state: TerminalPaneState
    let focused: Bool
    let onFocus: () -> Void
    let onProcessExit: () -> Void

    var body: some View {
        TerminalBridge(state: state, focused: focused, onFocus: onFocus, onProcessExit: onProcessExit)
    }
}

struct TerminalBridge: NSViewRepresentable {
    let state: TerminalPaneState
    let focused: Bool
    let onFocus: () -> Void
    let onProcessExit: () -> Void

    final class Coordinator {
        var wasFocused = false
        var paneID: UUID?
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> GhosttyTerminalNSView {
        let registry = TerminalViewRegistry.shared
        let view = registry.view(for: state.id, workingDirectory: state.projectPath)
        view.isFocused = focused
        view.onFocus = onFocus
        view.onProcessExit = onProcessExit
        view.onTitleChange = { [weak state] title in
            state?.title = title
        }
        context.coordinator.wasFocused = focused
        context.coordinator.paneID = state.id
        if focused {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                view.window?.makeFirstResponder(view)
            }
        }
        return view
    }

    func updateNSView(_ nsView: GhosttyTerminalNSView, context: Context) {
        nsView.onFocus = onFocus
        nsView.onProcessExit = onProcessExit
        nsView.onTitleChange = { [weak state] title in
            state?.title = title
        }
        let wasFocused = context.coordinator.wasFocused
        context.coordinator.wasFocused = focused
        nsView.isFocused = focused
        if focused, !wasFocused {
            DispatchQueue.main.async {
                nsView.window?.makeFirstResponder(nsView)
            }
        } else if !focused, wasFocused {
            nsView.notifySurfaceUnfocused()
        }
    }
}
