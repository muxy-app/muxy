import AppKit
import SwiftUI

struct TerminalPane: View {
    let state: TerminalPaneState
    let focused: Bool
    let visible: Bool
    let areaID: UUID
    let topLevelGroupID: UUID
    let onFocus: () -> Void
    let onProcessExit: () -> Void
    let onSplitRequest: (SplitDirection, SplitPosition) -> Void

    @Bindable private var ownership = PaneOwnershipStore.shared
    @Environment(\.overlayActive) private var overlayActive

    private var remoteOwnerName: String? {
        if case let .remote(_, name) = ownership.owner(for: state.id) {
            name
        } else {
            nil
        }
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
                DispatchQueue.main.async {
                    let view = TerminalViewRegistry.shared.existingView(for: state.id)
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
                topLevelGroupID: topLevelGroupID,
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
    let topLevelGroupID: UUID
    let onFocus: () -> Void
    let onProcessExit: () -> Void
    let onSplitRequest: (SplitDirection, SplitPosition) -> Void
    @Environment(\.overlayActive) private var overlayActive
    @Environment(\.activeWorktreeKey) private var worktreeKey
    @Environment(\.paneWorkspaceContext) private var workspaceContext
    @Environment(AppState.self) private var appState

    final class Coordinator {
        let claimID = UUID()

        struct FocusState: Equatable {
            let focused: Bool
            let overlayActive: Bool
        }

        private(set) var paneID: UUID?
        var wasFocused = false
        var wasOverlayActive = false

        func transition(
            paneID: UUID,
            focused: Bool,
            overlayActive: Bool,
            reset: Bool = false
        ) -> FocusState {
            if self.paneID != paneID || reset {
                self.paneID = paneID
                wasFocused = false
                wasOverlayActive = false
            }
            let previous = FocusState(focused: wasFocused, overlayActive: wasOverlayActive)
            wasFocused = focused
            wasOverlayActive = overlayActive
            return previous
        }
    }

    private static let mountBroker = ReparentingNSViewBroker<GhosttyTerminalNSView> {
        deactivate($0)
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> ReparentingNSViewHost {
        let host = ReparentingNSViewHost()
        let view = terminalView()
        Self.mountBroker.register(
            claimID: context.coordinator.claimID,
            view: view,
            host: host,
            configuration: mountConfiguration(coordinator: context.coordinator)
        )
        return host
    }

    func updateNSView(_ host: ReparentingNSViewHost, context: Context) {
        let view = terminalView()
        Self.mountBroker.update(
            claimID: context.coordinator.claimID,
            view: view,
            host: host,
            configuration: mountConfiguration(coordinator: context.coordinator)
        )
    }

    static func dismantleNSView(_ host: ReparentingNSViewHost, coordinator: Coordinator) {
        mountBroker.release(claimID: coordinator.claimID, host: host)
    }

    private var isCurrentPresentation: Bool {
        guard let worktreeKey,
              let visibleLayout = appState.visibleLayout(
                  for: worktreeKey,
                  groupID: topLevelGroupID
              )
        else { return false }
        return visibleLayout.allPanes().contains {
            $0.tab.content.pane?.id == state.id
        }
    }

    private func mountConfiguration(
        coordinator: Coordinator
    ) -> ReparentingNSViewBroker<GhosttyTerminalNSView>.Configuration {
        .init(
            isEligible: { isCurrentPresentation },
            prepare: { view in
                configure(view)
            },
            didMount: { view, host, ownershipChanged in
                updateFocus(
                    view,
                    host: host,
                    coordinator: coordinator,
                    ownershipChanged: ownershipChanged
                )
            }
        )
    }

    private func updateFocus(
        _ view: GhosttyTerminalNSView,
        host: ReparentingNSViewHost,
        coordinator: Coordinator,
        ownershipChanged: Bool
    ) {
        let previousFocus = coordinator.transition(
            paneID: state.id,
            focused: focused,
            overlayActive: overlayActive,
            reset: ownershipChanged
        )

        if overlayActive {
            if view.window?.firstResponder === view || view.window?.firstResponder === view.inputContext {
                view.window?.makeFirstResponder(nil)
            }
            if !previousFocus.overlayActive {
                view.notifySurfaceUnfocused()
            }
        } else if TerminalFocusRestorationPolicy.shouldClaimFocus(
            focused: focused,
            wasFocused: previousFocus.focused,
            wasOverlayActive: previousFocus.overlayActive
        ) {
            if ownershipChanged {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak host, weak view] in
                    guard let host, let view else { return }
                    Self.restoreFocus(to: view, in: host)
                }
            } else {
                DispatchQueue.main.async { [weak host, weak view] in
                    guard let host, let view else { return }
                    Self.restoreFocus(to: view, in: host)
                }
            }
        } else if TerminalFocusRestorationPolicy.shouldReleaseFocus(
            focused: focused,
            wasFocused: previousFocus.focused,
            attachmentChanged: ownershipChanged
        ) {
            view.notifySurfaceUnfocused()
            if view.window?.firstResponder === view || view.window?.firstResponder === view.inputContext {
                view.window?.makeFirstResponder(nil)
            }
        }
    }

    private static func restoreFocus(
        to view: GhosttyTerminalNSView,
        in host: ReparentingNSViewHost
    ) {
        guard view.superview === host,
              view.isFocused,
              !view.overlayActive
        else { return }
        view.window?.makeFirstResponder(view)
    }

    private func configure(_ view: GhosttyTerminalNSView) {
        if view.envVars.isEmpty, view.surface == nil, let key = worktreeKey {
            view.envVars = TerminalEnvVarBuilder.build(paneID: state.id, worktreeKey: key)
        }
        view.isFocused = focused
        view.overlayActive = overlayActive
        view.updateResumeWorkingDirectory(state.currentWorkingDirectory ?? state.projectPath)
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
        configureAgentDetectionCallback(view)
        configureSearchCallbacks(view)
        configureFileOpenCallback(view)
        configureProgressCallback(view)
    }

    private static func deactivate(_ view: GhosttyTerminalNSView) {
        if view.window?.firstResponder === view || view.window?.firstResponder === view.inputContext {
            view.window?.makeFirstResponder(nil)
        }
        view.isFocused = false
        view.setFocused(false)
        view.setVisible(false)
        view.notifySurfaceUnfocused()
    }

    private func terminalView() -> GhosttyTerminalNSView {
        let launch = state.consumeRestoredLaunch()
        return TerminalViewRegistry.shared.view(
            for: state.id,
            workingDirectory: state.currentWorkingDirectory ?? state.projectPath,
            command: launch.command,
            commandInteractive: launch.interactive,
            closesOnCommandExit: launch.closesOnCommandExit,
            workspaceContext: workspaceContext
        )
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

    private func configureAgentDetectionCallback(_ view: GhosttyTerminalNSView) {
        view.onDetectedAgentChange = { [weak state, weak view] providerID in
            guard let paneID = state?.id else { return }
            DetectedAgentStore.shared.setAgent(providerID, for: paneID)
            guard providerID != nil else {
                AgentStatusStore.shared.noteDetectionLost(paneID: paneID)
                return
            }
            AgentStatusStore.shared.noteDetectionActive(
                paneID: paneID,
                processID: view?.foregroundProcessID
            )
        }
    }

    private func configureFileOpenCallback(_ view: GhosttyTerminalNSView) {
        let projectPath = state.projectPath
        let appState = appState
        let projectID = worktreeKey?.projectID
        let areaID = areaID
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
            Self.resolveFileLocation(from: token, projectPath: projectPath) != nil
        }
        view.onCmdClickFile = { token in
            guard let location = Self.resolveFileLocation(from: token, projectPath: projectPath) else { return }
            if Self.openWithRegisteredFileOpener(
                location,
                projectPath: projectPath,
                projectID: projectID,
                areaID: areaID,
                appState: appState
            ) {
                return
            }
            _ = IDEIntegrationService.shared.openProject(
                at: projectPath,
                highlightingFileAt: location.path,
                line: location.line,
                column: location.column
            )
        }
        view.onOpenURL = { url in
            if let location = Self.resolveFileLocation(from: url, projectPath: projectPath) {
                if Self.openWithRegisteredFileOpener(
                    location,
                    projectPath: projectPath,
                    projectID: projectID,
                    areaID: areaID,
                    appState: appState
                ) {
                    return true
                }
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

    @MainActor
    private static func openWithRegisteredFileOpener(
        _ location: ResolvedFileLocation,
        projectPath: String,
        projectID: UUID?,
        areaID: UUID,
        appState: AppState
    ) -> Bool {
        guard let projectID,
              let relativePath = relativePath(location.path, inside: projectPath),
              let binding = ExtensionStore.shared.preferredFileOpener(for: relativePath)
        else {
            return false
        }

        var data: [String: ExtensionJSON] = [
            "filePath": .string(relativePath),
            "source": .string("terminal"),
        ]
        if let line = location.line {
            data["line"] = .number(Double(line))
        }
        if let column = location.column {
            data["column"] = .number(Double(column))
        }

        appState.dispatch(.createExtensionTab(
            projectID: projectID,
            areaID: areaID,
            request: AppState.CreateExtensionTabRequest(
                extensionID: binding.muxyExtension.id,
                tabTypeID: binding.opener.tabType,
                title: binding.opener.title ?? binding.tabType.title,
                data: .object(data),
                singleton: binding.opener.singleton
            )
        ))
        return true
    }

    static func relativePath(_ filePath: String, inside projectPath: String) -> String? {
        let fileURL = URL(fileURLWithPath: filePath).standardizedFileURL
        let projectURL = URL(fileURLWithPath: projectPath).standardizedFileURL
        let file = fileURL.path
        let project = projectURL.path
        guard file == project || file.hasPrefix(project + "/") else { return nil }
        let relative = String(file.dropFirst(project.count)).trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        return relative.isEmpty ? nil : relative
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

    static func resolveFileLocation(from token: String, projectPath: String) -> ResolvedFileLocation? {
        if let path = resolveFilePath(token, projectPath: projectPath) {
            return ResolvedFileLocation(path: path, line: nil, column: nil)
        }
        let cleaned = token.trimmingCharacters(in: CharacterSet(charactersIn: "\"' \t\n\r()[]<>"))
        guard let stripped = stripLineColumnSuffix(from: cleaned) else { return nil }
        guard let path = resolveFilePath(stripped.path, projectPath: projectPath) else { return nil }
        return ResolvedFileLocation(path: path, line: stripped.line, column: stripped.column)
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
        let worktreeKey = worktreeKey
        view.onProgressReport = { progress in
            Task { @MainActor in
                TerminalProgressStore.shared.setProgress(progress, for: paneID, worktreeKey: worktreeKey)
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

enum TerminalFocusRestorationPolicy {
    static func shouldClaimFocus(
        focused: Bool,
        wasFocused: Bool,
        wasOverlayActive: Bool
    ) -> Bool {
        focused && (!wasFocused || wasOverlayActive)
    }

    static func shouldReleaseFocus(
        focused: Bool,
        wasFocused: Bool,
        attachmentChanged: Bool
    ) -> Bool {
        !focused && (wasFocused || attachmentChanged)
    }
}
