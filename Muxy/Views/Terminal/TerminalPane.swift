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

    private var showsSleepingPlaceholder: Bool {
        SleepingTabPlaceholderPolicy.shouldPresent(
            isVisible: visible,
            isOffline: state.isOffline,
            isRemotelyOwned: remoteOwnerName != nil
        )
    }

    private func wakePane() {
        TerminalViewRegistry.shared.existingView(for: state.id)?.wake()
        onFocus()
    }

    var body: some View {
        terminalLayer
            .onReceive(NotificationCenter.default.publisher(for: .refocusActiveTerminal)) { _ in
                guard focused, visible else { return }
                let view = TerminalViewRegistry.shared.existingView(for: state.id)
                DispatchQueue.main.async { [weak view] in
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
                        DispatchQueue.main.async { [weak view] in
                            view?.window?.makeFirstResponder(view)
                        }
                    }
                )
                .transition(.move(edge: .top).combined(with: .opacity))
            }

            if showsSleepingPlaceholder {
                SleepingTabPlaceholder(isFocused: focused, onWake: wakePane)
                    .transition(.opacity)
            }
        }
    }
}

struct SleepingTabPlaceholder: View {
    let isFocused: Bool
    let onWake: () -> Void

    var body: some View {
        VStack(spacing: UIMetrics.spacing7) {
            Spacer()
            Image(systemName: "moon.zzz")
                .font(.system(size: UIMetrics.fontMega))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text("Tab is asleep")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Text("This terminal was freed to save memory. Wake it to resume your session.")
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
                .multilineTextAlignment(.center)
                .frame(maxWidth: UIMetrics.scaled(360))
            Button(action: onWake) {
                HStack(spacing: UIMetrics.spacing4) {
                    Text("Wake")
                    if isFocused {
                        Text("⏎")
                            .font(.system(size: UIMetrics.fontFootnote, weight: .medium, design: .rounded))
                            .opacity(0.72)
                    }
                }
            }
            .keyboardShortcut(isFocused ? KeyboardShortcut(.return, modifiers: []) : nil)
            .buttonStyle(.borderedProminent)
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(MuxyTheme.bg)
        .contentShape(Rectangle())
        .onTapGesture(perform: onWake)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityLabel("Tab is asleep")
        .accessibilityHint("Wake the terminal to resume your session")
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
    @Environment(\.paneWorkspaceContext) private var workspaceContext
    @Environment(AppState.self) private var appState

    final class Coordinator {
        var wasFocused = false
        var wasOverlayActive = false
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> GhosttyTerminalNSView {
        let registry = TerminalViewRegistry.shared
        let launch = state.consumeRestoredLaunch()
        let view = registry.view(
            for: state.id,
            workingDirectory: state.currentWorkingDirectory ?? state.projectPath,
            command: launch.command,
            commandInteractive: launch.interactive,
            closesOnCommandExit: launch.closesOnCommandExit,
            workspaceContext: workspaceContext
        )
        if view.envVars.isEmpty, let key = worktreeKey {
            view.envVars = TerminalEnvVarBuilder.build(paneID: state.id, worktreeKey: key)
        }
        view.isFocused = focused
        view.overlayActive = overlayActive
        view.setVisible(visible)
        view.setFocused(focused)
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
        view.onOfflineChange = { [weak state] offline in
            state?.isOffline = offline
        }
        view.updateResumeWorkingDirectory(state.currentWorkingDirectory ?? state.projectPath)
        configureSearchCallbacks(view)
        configureFileOpenCallback(view)
        configureProgressCallback(view)
        context.coordinator.wasFocused = focused
        if focused, !overlayActive {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak view] in
                guard let view else { return }
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
        nsView.updateResumeWorkingDirectory(state.currentWorkingDirectory ?? state.projectPath)
        nsView.setVisible(visible)
        nsView.setFocused(focused)
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
        nsView.onOfflineChange = { [weak state] offline in
            state?.isOffline = offline
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
            DispatchQueue.main.async { [weak nsView] in
                guard let nsView else { return }
                nsView.window?.makeFirstResponder(nsView)
            }
        } else if !focused, wasFocused {
            nsView.notifySurfaceUnfocused()
            if nsView.window?.firstResponder === nsView || nsView.window?.firstResponder === nsView.inputContext {
                nsView.window?.makeFirstResponder(nil)
            }
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
        let projectPath = state.projectPath
        let appState = appState
        guard !workspaceContext.isRemote else {
            view.resolveCmdHoverFile = { _ in false }
            view.onCmdClickFile = { _ in }
            view.onOpenURL = { url in
                guard Self.isExternalLink(url) else { return false }
                return Self.openExternalLink(url, appState: appState)
            }
            return
        }
        view.resolveCmdHoverFile = { token in
            Self.resolveFilePath(token, projectPath: projectPath) != nil
        }
        view.onCmdClickFile = { token in
            guard let resolved = Self.resolveFilePath(token, projectPath: projectPath) else { return }
            _ = IDEIntegrationService.shared.openProject(at: projectPath, highlightingFileAt: resolved)
        }
        view.onOpenURL = { url in
            if let location = Self.resolveFileLocation(from: url, projectPath: projectPath) {
                return IDEIntegrationService.shared.openProject(
                    at: projectPath,
                    highlightingFileAt: location.path,
                    line: location.line,
                    column: location.column
                )
            }
            guard Self.isExternalLink(url) else {
                ToastState.shared.show("File not found")
                return false
            }
            return Self.openExternalLink(url, appState: appState)
        }
    }

    private static func openExternalLink(_ url: URL, appState: AppState) -> Bool {
        if BrowserPreferences.isEnabled, BrowserPreferences.openLinksInBuiltInBrowser {
            return appState.openInBuiltInBrowser(url)
        }
        return NSWorkspace.shared.open(url)
    }

    struct ResolvedFileLocation: Equatable {
        let path: String
        let line: Int?
        let column: Int?
    }

    static func isExternalLink(_ url: URL) -> Bool {
        guard url.scheme != nil else { return false }
        guard !isLocalPathCandidate(url) else { return false }
        return true
    }

    static func isLocalPathCandidate(_ url: URL) -> Bool {
        guard !url.isFileURL, url.host == nil, !url.absoluteString.contains("//") else { return false }
        let raw = url.absoluteString.removingPercentEncoding ?? url.absoluteString
        return url.scheme == nil || stripLineColumnSuffix(from: raw) != nil
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

    static func resolveLocalFilePath(from url: URL, projectPath: String) -> String? {
        if url.isFileURL {
            let path = url.path
            guard !path.isEmpty else { return nil }
            var isDirectory: ObjCBool = false
            guard FileManager.default.fileExists(atPath: path, isDirectory: &isDirectory) else { return nil }
            guard !isDirectory.boolValue else { return nil }
            return path
        }
        guard isLocalPathCandidate(url) else { return nil }
        let raw = url.absoluteString.removingPercentEncoding ?? url.absoluteString
        return resolveFilePath(raw, projectPath: projectPath)
    }

    static func resolveFileLocation(from url: URL, projectPath: String) -> ResolvedFileLocation? {
        if let path = resolveLocalFilePath(from: url, projectPath: projectPath) {
            return ResolvedFileLocation(path: path, line: nil, column: nil)
        }
        guard isLocalPathCandidate(url) else { return nil }
        let raw = url.absoluteString.removingPercentEncoding ?? url.absoluteString
        guard let stripped = stripLineColumnSuffix(from: raw) else { return nil }
        guard let path = resolveFilePath(stripped.path, projectPath: projectPath) else { return nil }
        return ResolvedFileLocation(path: path, line: stripped.line, column: stripped.column)
    }

    static func stripLineColumnSuffix(from token: String) -> ResolvedFileLocation? {
        let components = token.split(separator: ":", omittingEmptySubsequences: false).map(String.init)
        guard components.count >= 2 else { return nil }

        if components.count >= 3,
           let line = numericComponent(components[components.count - 2]),
           let column = numericComponent(components[components.count - 1])
        {
            let path = components.dropLast(2).joined(separator: ":")
            guard !path.isEmpty else { return nil }
            return ResolvedFileLocation(path: path, line: line, column: column)
        }

        if let line = numericComponent(components[components.count - 1]) {
            let path = components.dropLast().joined(separator: ":")
            guard !path.isEmpty else { return nil }
            return ResolvedFileLocation(path: path, line: line, column: nil)
        }

        return nil
    }

    private static func numericComponent(_ component: String) -> Int? {
        guard !component.isEmpty, component.allSatisfy(\.isNumber) else { return nil }
        return Int(component)
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
