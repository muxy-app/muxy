import AppKit
import SwiftUI

struct TerminalPane: View {
    let state: TerminalPaneState
    let focused: Bool
    let visible: Bool
    let areaID: UUID
    let onFocus: () -> Void
    let onProcessExit: () -> Void
    let onSplitRequest: (SplitDirection, SplitPosition) -> Void

    @Bindable private var ownership = PaneOwnershipStore.shared
    @Environment(\.overlayActive) private var overlayActive

    private var remoteOwnerName: String? {
        if case let .remote(_, name) = ownership.owner(for: state.id) { name } else { nil }
    }

    var body: some View {
        terminalLayer
            .onAppear { state.branchObserver.start() }
            .onDisappear { state.branchObserver.stop() }
            .onReceive(NotificationCenter.default.publisher(for: .refocusActiveTerminal)) { _ in
                guard focused, visible else { return }
                let view = TerminalViewRegistry.shared.existingView(for: state.id)
                DispatchQueue.main.async {
                    view?.window?.makeFirstResponder(view)
                }
            }
    }

    private var terminalLayer: some View {
        ZStack(alignment: .topTrailing) {
            TerminalBridge(
                state: state,
                focused: focused,
                visible: visible,
                areaID: areaID,
                onFocus: onFocus,
                onProcessExit: onProcessExit,
                onSplitRequest: onSplitRequest
            )
            .accessibilityElement(children: .contain)
            .accessibilityLabel("Terminal")
            .accessibilityAddTraits(.allowsDirectInteraction)
            .opacity(remoteOwnerName == nil ? 1 : 0)
            .allowsHitTesting(remoteOwnerName == nil)

            if let name = remoteOwnerName {
                RemoteControlledPlaceholder(deviceName: name) {
                    PaneOwnershipStore.shared.releaseToMac(paneID: state.id)
                }
                .transition(.opacity)
            }

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
        }
    }
}

struct RemoteControlledPlaceholder: View {
    let deviceName: String
    let onTakeOver: () -> Void

    var body: some View {
        VStack(spacing: UIMetrics.spacing7) {
            Spacer()
            Image(systemName: "iphone.gen3")
                .font(.system(size: UIMetrics.fontMega))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text("Controlled by \(deviceName)")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Text("This terminal session is currently being used on \(deviceName). Take over to resume on Mac.")
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 360)
            Button {
                onTakeOver()
            } label: {
                HStack(spacing: UIMetrics.spacing4) {
                    Text("Take Over")
                    Text("⌘↩")
                        .font(.system(size: UIMetrics.fontFootnote, weight: .medium, design: .rounded))
                        .opacity(0.72)
                }
            }
            .keyboardShortcut(.return, modifiers: .command)
            .buttonStyle(.borderedProminent)
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(MuxyTheme.bg)
    }
}

struct TerminalBridge: NSViewRepresentable {
    let state: TerminalPaneState
    let focused: Bool
    let visible: Bool
    let areaID: UUID
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
            workingDirectory: state.currentWorkingDirectory ?? state.projectPath,
            command: state.startupCommand,
            commandInteractive: state.startupCommandInteractive
        )
        if view.envVars.isEmpty, let key = worktreeKey {
            view.envVars = TerminalEnvVarBuilder.build(paneID: state.id, worktreeKey: key)
        }
        view.isFocused = focused
        view.overlayActive = overlayActive
        view.setVisible(visible)
        view.onFocus = onFocus
        view.onProcessExit = onProcessExit
        view.onSplitRequest = onSplitRequest
        view.onExternalDragHoverChange = makeExternalDragHoverHandler(areaID: areaID)
        view.onTitleChange = { [weak state] title in
            DispatchQueue.main.async {
                state?.setTitle(title)
            }
        }
        view.onWorkingDirectoryChange = { [weak state] path in
            DispatchQueue.main.async {
                state?.setWorkingDirectory(path)
            }
        }
        configureSearchCallbacks(view)
        configureFileOpenCallback(view)
        configureProgressCallback(view)
        context.coordinator.wasFocused = focused
        if focused, !overlayActive {
            view.notifySurfaceFocused()
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                view.window?.makeFirstResponder(view)
            }
        } else {
            view.notifySurfaceUnfocused()
            if view.window?.firstResponder === view {
                view.window?.makeFirstResponder(nil)
            }
        }
        return view
    }

    func updateNSView(_ nsView: GhosttyTerminalNSView, context: Context) {
        if nsView.envVars.isEmpty, nsView.surface == nil, let key = worktreeKey {
            nsView.envVars = TerminalEnvVarBuilder.build(paneID: state.id, worktreeKey: key)
        }
        nsView.overlayActive = overlayActive
        nsView.setVisible(visible)
        nsView.onFocus = onFocus
        nsView.onProcessExit = onProcessExit
        nsView.onSplitRequest = onSplitRequest
        nsView.onExternalDragHoverChange = makeExternalDragHoverHandler(areaID: areaID)
        nsView.onTitleChange = { [weak state] title in
            DispatchQueue.main.async {
                state?.setTitle(title)
            }
        }
        nsView.onWorkingDirectoryChange = { [weak state] path in
            DispatchQueue.main.async {
                state?.setWorkingDirectory(path)
            }
        }
        configureSearchCallbacks(nsView)
        configureFileOpenCallback(nsView)
        configureProgressCallback(nsView)
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

    private func makeExternalDragHoverHandler(areaID: UUID) -> (Bool) -> Void {
        { hovering in
            NotificationCenter.default.post(
                name: .externalDragHoverChanged,
                object: nil,
                userInfo: [
                    ExternalDragHoverUserInfoKey.isHovering: hovering,
                    ExternalDragHoverUserInfoKey.areaID: areaID,
                ]
            )
        }
    }

    private func configureFileOpenCallback(_ view: GhosttyTerminalNSView) {
        let projectID = worktreeKey?.projectID
        let projectPath = state.projectPath
        view.onCmdClickFile = { token in
            guard let projectID else { return }
            guard let resolved = Self.resolveFilePath(token, projectPath: projectPath) else { return }
            Task { @MainActor in
                NotificationStore.shared.appState?.openFile(resolved, projectID: projectID, preserveFocus: true)
            }
        }
        view.resolveCmdHoverFile = { token in
            Self.resolveFilePath(token, projectPath: projectPath) != nil
        }
        view.onOpenURL = { url in
            if let projectID, url.isFileURL {
                let path = url.path
                guard !path.isEmpty, FileManager.default.fileExists(atPath: path) else { return false }
                Task { @MainActor in
                    NotificationStore.shared.appState?.openFile(path, projectID: projectID, preserveFocus: true)
                }
                return true
            }
            return NSWorkspace.shared.open(url)
        }
    }

    static func resolveFilePath(_ token: String, projectPath: String) -> String? {
        let cleaned = token.trimmingCharacters(in: CharacterSet(charactersIn: "\"' \t\n\r()[]<>"))
        guard !cleaned.isEmpty else { return nil }
        let expanded = (cleaned as NSString).expandingTildeInPath
        let candidate: String = if expanded.hasPrefix("/") {
            expanded
        } else {
            (projectPath as NSString).appendingPathComponent(expanded)
        }
        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(atPath: candidate, isDirectory: &isDirectory) else { return nil }
        guard !isDirectory.boolValue else { return nil }
        return candidate
    }

    private func configureProgressCallback(_ view: GhosttyTerminalNSView) {
        let paneID = state.id
        let projectID = worktreeKey?.projectID
        view.onProgressReport = { progress in
            Task { @MainActor in
                TerminalProgressStore.shared.setProgress(progress, for: paneID, projectID: projectID)
            }
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
}
