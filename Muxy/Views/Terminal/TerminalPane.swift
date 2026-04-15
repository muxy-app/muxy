import AppKit
import SwiftUI

struct TerminalPane: View {
    let state: TerminalPaneState
    let focused: Bool
    let visible: Bool
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
                        DispatchQueue.main.async {
                            view?.window?.makeFirstResponder(view)
                        }
                    }
                )
                .transition(.move(edge: .top).combined(with: .opacity))
            }

            if state.quickSelectState.isVisible {
                TerminalQuickSelectOverlay(state: state.quickSelectState)
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .quickSelect)) { _ in
            guard focused else { return }
            let view = TerminalViewRegistry.shared.existingView(for: state.id)
            state.quickSelectState.activate(snapshot: view?.quickSelectSnapshot())
            view?.window?.makeFirstResponder(view)
        }
    }
}

struct TerminalBridge: NSViewRepresentable {
    let state: TerminalPaneState
    let focused: Bool
    let onFocus: () -> Void
    let onProcessExit: () -> Void
    let onSplitRequest: (SplitDirection, SplitPosition) -> Void
    @Environment(\.overlayActive) private var overlayActive
    @Environment(\.activeWorktreeKey) private var worktreeKey

    final class Coordinator {
        var wasFocused = false
        var wasOverlayActive = false
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> GhosttyTerminalNSView {
        let registry = TerminalViewRegistry.shared
        let view = registry.view(
            for: state.id,
            workingDirectory: state.projectPath,
            command: state.startupCommand
        )
        if view.envVars.isEmpty, let key = worktreeKey {
            view.envVars = Self.buildEnvVars(paneID: state.id, worktreeKey: key)
        }
        view.isFocused = focused
        view.overlayActive = overlayActive
        view.quickSelectActive = state.quickSelectState.isVisible
        view.onFocus = onFocus
        view.onProcessExit = onProcessExit
        view.onSplitRequest = onSplitRequest
        view.onQuickSelectInput = { [weak state, weak view] input in
            Self.handleQuickSelectInput(input, state: state, view: view)
        }
        view.onTitleChange = { [weak state] title in
            DispatchQueue.main.async {
                state?.setTitle(title)
            }
        }
        configureSearchCallbacks(view)
        context.coordinator.wasFocused = focused
        if focused, !overlayActive {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                view.window?.makeFirstResponder(view)
            }
        }
        return view
    }

    func updateNSView(_ nsView: GhosttyTerminalNSView, context: Context) {
        if nsView.envVars.isEmpty, nsView.surface == nil, let key = worktreeKey {
            nsView.envVars = Self.buildEnvVars(paneID: state.id, worktreeKey: key)
        }
        nsView.overlayActive = overlayActive
        nsView.onFocus = onFocus
        nsView.onProcessExit = onProcessExit
        nsView.onSplitRequest = onSplitRequest
        nsView.quickSelectActive = state.quickSelectState.isVisible
        nsView.onQuickSelectInput = { [weak state, weak nsView] input in
            Self.handleQuickSelectInput(input, state: state, view: nsView)
        }
        nsView.onTitleChange = { [weak state] title in
            DispatchQueue.main.async {
                state?.setTitle(title)
            }
        }
        configureSearchCallbacks(nsView)
        let wasFocused = context.coordinator.wasFocused
        let wasOverlayActive = context.coordinator.wasOverlayActive
        context.coordinator.wasFocused = focused
        context.coordinator.wasOverlayActive = overlayActive
        nsView.isFocused = focused

        if overlayActive {
            if nsView.window?.firstResponder === nsView || nsView.window?.firstResponder === nsView.inputContext {
                nsView.window?.makeFirstResponder(nil)
            }
            if !wasOverlayActive {
                nsView.notifySurfaceUnfocused()
            }
        } else if focused, !wasFocused || wasOverlayActive {
            nsView.notifySurfaceFocused()
            DispatchQueue.main.async {
                nsView.window?.makeFirstResponder(nsView)
            }
        } else if !focused, wasFocused {
            nsView.notifySurfaceUnfocused()
        }
    }

    private static func buildEnvVars(paneID: UUID, worktreeKey key: WorktreeKey) -> [(key: String, value: String)] {
        var vars: [(key: String, value: String)] = [
            (key: "MUXY_PANE_ID", value: paneID.uuidString),
            (key: "MUXY_PROJECT_ID", value: key.projectID.uuidString),
            (key: "MUXY_WORKTREE_ID", value: key.worktreeID.uuidString),
            (key: "MUXY_SOCKET_PATH", value: NotificationSocketServer.socketPath),
        ]
        if let hookPath = MuxyNotificationHooks.hookScriptPath {
            vars.append((key: "MUXY_HOOK_SCRIPT", value: hookPath))
        }
        return vars
    }

    private func configureSearchCallbacks(_ view: GhosttyTerminalNSView) {
        view.onSearchStart = { [weak state] needle in
            guard let state else { return }
            let searchState = state.searchState
            if let needle, !needle.isEmpty {
                searchState.needle = needle
            }
            searchState.isVisible = true
            searchState.focusVersion += 1
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

    private static func handleQuickSelectInput(
        _ input: TerminalQuickSelectInput,
        state: TerminalPaneState?,
        view: GhosttyTerminalNSView?
    ) {
        guard let state else { return }
        switch state.quickSelectState.handle(input) {
        case .none,
             .dismiss:
            break
        case let .copy(text, paste):
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
            if paste {
                view?.sendText(text)
            }
            DispatchQueue.main.async {
                view?.window?.makeFirstResponder(view)
            }
        }
    }
}

private struct TerminalQuickSelectOverlay: View {
    let state: TerminalQuickSelectState

    var body: some View {
        ZStack(alignment: .topLeading) {
            Color.black.opacity(0.08)
                .ignoresSafeArea()

            ForEach(state.matches) { match in
                Text(match.label.uppercased())
                    .font(.system(size: 11, weight: .bold, design: .monospaced))
                    .foregroundStyle(.black)
                    .padding(.horizontal, 4)
                    .padding(.vertical, 1)
                    .background(MuxyTheme.accent, in: RoundedRectangle(cornerRadius: 4))
                    .overlay(RoundedRectangle(cornerRadius: 4).stroke(.black.opacity(0.35), lineWidth: 1))
                    .position(x: match.frame.minX + 12, y: match.frame.minY + 8)
            }

            HStack(spacing: 8) {
                Text(state.prefix.isEmpty ? state.status : "\(state.status): \(state.prefix.uppercased())")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                Text("Esc")
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(MuxyTheme.bg.opacity(0.92), in: RoundedRectangle(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(MuxyTheme.border, lineWidth: 1))
            .padding(8)
        }
        .allowsHitTesting(false)
        .transition(.opacity)
    }
}
