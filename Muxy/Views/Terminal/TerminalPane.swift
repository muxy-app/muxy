import AppKit
import SwiftUI

struct TerminalPane: View {
    let state: TerminalPaneState
    let focused: Bool
    let onFocus: () -> Void
    let onProcessExit: () -> Void
    let onSplitRequest: (SplitDirection, SplitPosition) -> Void

    var body: some View {
        ZStack(alignment: .topTrailing) {
            TerminalBridge(
                state: state,
                focused: focused,
                onFocus: onFocus,
                onProcessExit: onProcessExit,
                onSplitRequest: onSplitRequest
            )

            if state.searchState.isVisible {
                TerminalSearchBar(
                    searchState: state.searchState,
                    onNavigateNext: {
                        let view = TerminalViewRegistry.shared.existingView(for: state.id)
                        view?.navigateSearch(direction: .next)
                    },
                    onNavigatePrevious: {
                        let view = TerminalViewRegistry.shared.existingView(for: state.id)
                        view?.navigateSearch(direction: .previous)
                    },
                    onClose: {
                        let view = TerminalViewRegistry.shared.existingView(for: state.id)
                        view?.endSearch()
                    }
                )
                .transition(.move(edge: .top).combined(with: .opacity))
            }
        }
    }
}

struct TerminalBridge: NSViewRepresentable {
    let state: TerminalPaneState
    let focused: Bool
    let onFocus: () -> Void
    let onProcessExit: () -> Void
    let onSplitRequest: (SplitDirection, SplitPosition) -> Void

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
        view.onSplitRequest = onSplitRequest
        view.onTitleChange = { [weak state] title in
            state?.title = title
        }
        configureSearchCallbacks(view)
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
        nsView.onSplitRequest = onSplitRequest
        nsView.onTitleChange = { [weak state] title in
            state?.title = title
        }
        configureSearchCallbacks(nsView)
        let wasFocused = context.coordinator.wasFocused
        context.coordinator.wasFocused = focused
        nsView.isFocused = focused
        if focused, !wasFocused {
            nsView.notifySurfaceFocused()
            DispatchQueue.main.async {
                nsView.window?.makeFirstResponder(nsView)
            }
        } else if !focused {
            nsView.notifySurfaceUnfocused()
        }
    }

    private func configureSearchCallbacks(_ view: GhosttyTerminalNSView) {
        view.onSearchStart = { [weak state] needle in
            guard let state else { return }
            let searchState = state.searchState
            if let needle, !needle.isEmpty {
                searchState.needle = needle
            }
            searchState.isVisible = true
            searchState.startPublishing { [weak view] query in
                view?.sendSearchQuery(query)
            }
            if !searchState.needle.isEmpty {
                searchState.pushNeedle()
            }
        }
        view.onSearchEnd = { [weak state] in
            guard let state else { return }
            state.searchState.stopPublishing()
            state.searchState.isVisible = false
            state.searchState.needle = ""
            state.searchState.total = nil
            state.searchState.selected = nil
        }
        view.onSearchTotal = { [weak state] total in
            state?.searchState.total = total
        }
        view.onSearchSelected = { [weak state] selected in
            state?.searchState.selected = selected
        }
    }
}
