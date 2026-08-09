import AppKit
import Darwin
import GhosttyKit
import MuxyShared

final class GhosttyTerminalNSView: NSView,
    TerminalSurface,
    TerminalRawOutputSource,
    TerminalGridSnapshotSource,
    TerminalClientThemeSurface,
    TerminalOfflineSurface,
    TerminalSearchSurface,
    TerminalUploadSurface,
    TerminalBackgroundingSurface,
    TerminalSessionRecoverySurface
{
    nonisolated(unsafe) private(set) var surface: ghostty_surface_t?
    var terminalView: NSView { self }
    let backend = TerminalBackend.ghostty
    private var surfaceFocused: Bool?
    private var workingDirectory: String
    private let command: String?
    private let commandInteractive: Bool
    private let commandClosesOnExit: Bool
    private let workspaceContext: WorkspaceContext
    let persistentSessionID: UUID?
    var envVars: [(key: String, value: String)] = []
    var sessionMetadata: [(key: String, value: String)] = []
    var onTitleChange: ((String) -> Void)?
    var onWorkingDirectoryChange: ((String) -> Void)?
    var onFocus: (() -> Void)?
    var onExternalDragHoverChange: ((Bool) -> Void)?
    var onProcessExit: (() -> Void)?
    var onSplitRequest: ((SplitDirection, SplitPosition) -> Void)?
    var canSendToBackground: (() -> Bool)?
    var onSendToBackground: (() -> Void)?
    var onSessionRecoveryFailed: ((Bool) -> Void)?
    private(set) var isSessionRecoveryFailed = false
    var onSearchStart: ((String?) -> Void)?
    var onSearchEnd: (() -> Void)?
    var onSearchTotal: ((Int?) -> Void)?
    var onSearchSelected: ((Int?) -> Void)?
    var onProgressReport: ((TerminalProgress?) -> Void)?
    private let scrollbarOverlay = TerminalScrollbarOverlay()
    var onCmdClickFile: ((String) -> Void)?
    var resolveCmdHoverFile: ((String) -> Bool)?
    var onOpenURL: ((URL) -> Bool)?
    private var isShowingHandCursor = false
    private var fileHoverUnderlineLayer: CAShapeLayer?
    private var lastMouseTopDownPoint: CGPoint?
    private var leftMousePressRouting: LeftMousePressRouting = .ignored
    private var leftMouseDownWindowPoint: NSPoint?
    private var didDragAfterLeftMouseDown = false
    private var forwardedRightMousePress = false
    var hasOSC8LinkUnderCursor: Bool = false
    var isFocused: Bool = false
    var overlayActive: Bool = false

    var processExitHandled = false

    var onOfflineChange: ((Bool) -> Void)?
    var onDetectedAgentChange: ((String?) -> Void)?
    var onAgentProcessExit: (() -> Void)?
    nonisolated(unsafe) private var agentDetectionCoalesced: DispatchWorkItem?
    private let agentProcessWatcher = AgentProcessExitWatcher()
    private var detectedAgentProviderID: String?
    private var agentExecutables: [AIAgentExecutable] = []
    private var hasMaterializedOnce = false
    private var isOfflinedState = false
    private var offlineInvisibleAt: Date?
    private let shellActivityTracker = TerminalShellActivityTracker()
    private var streamedPaneID: UUID?

    var isTakenOffline: Bool { isOfflinedState }
    var offlineInvisibleSince: Date? { offlineInvisibleAt }

    private var isPaneVisible = true
    private var isPaneFocused = false
    private var isWindowVisible = true
    nonisolated(unsafe) private var occlusionObserver: NSObjectProtocol?

    var closesOnCommandExit: Bool {
        command != nil && commandClosesOnExit
    }

    var foregroundProcessID: Int32? {
        guard let surface else { return nil }
        if let persistentSessionID {
            return PersistentSessionService.shared.foregroundProcessID(sessionID: persistentSessionID)
        }
        let processID = ghostty_surface_foreground_pid(surface)
        guard processID > 0, processID <= UInt64(Int32.max) else { return nil }
        return Int32(processID)
    }

    private var _markedText: String = ""
    private var _markedRange: NSRange = .init(location: NSNotFound, length: 0)
    private var _selectedRange: NSRange = .init(location: 0, length: 0)

    private var keyTextAccumulator: [String] = []
    private var currentKeyEvent: NSEvent?
    private var commandSelectorCalled = false
    private var inputTrackingDecisionCache: (expiresAt: Date, canRecord: Bool)?
    nonisolated(unsafe) private var surfaceCStringPointers: [UnsafeMutablePointer<CChar>] = []
    nonisolated(unsafe) private var surfaceEnvVarPointer: UnsafeMutablePointer<ghostty_env_var_s>?
    nonisolated(unsafe) private var surfaceEnvVarCount = 0
    private var rawOutputToken: Int?
    private var surfaceConfigurationOverlay: ((ghostty_surface_t) -> Void)?
    private let terminalInputQueue = TerminalInputQueue()
    private var surfaceGeneration = 0
    private static let imagePasteDelay: Duration = .milliseconds(300)
    private let remoteUploadSession = RemoteUploadSession()
    private var precedingRemoteUploadCleanup: Task<Void, Never>?

    init(
        workingDirectory: String,
        command: String? = nil,
        commandInteractive: Bool = false,
        closesOnCommandExit: Bool = true,
        workspaceContext: WorkspaceContext = .local,
        persistentSessionID: UUID? = nil
    ) {
        self.workingDirectory = workingDirectory
        self.command = command
        self.commandInteractive = commandInteractive
        commandClosesOnExit = closesOnCommandExit
        self.workspaceContext = workspaceContext
        self.persistentSessionID = persistentSessionID
        super.init(frame: .zero)
        wantsLayer = true
        setupTrackingArea()
        installScrollbarOverlay()
        registerForDraggedTypes([.fileURL, .string])
        setAccessibilityRole(.textArea)
        setAccessibilityRoleDescription(L10n.string("Terminal"))
        let directoryName = URL(fileURLWithPath: workingDirectory).lastPathComponent
        let label = directoryName.isEmpty
            ? L10n.string("Terminal")
            : L10n.string("Terminal — \(directoryName)")
        setAccessibilityLabel(label)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    override func accessibilitySelectedText() -> String? {
        readSelectionText()
    }

    private func readSelectionText() -> String? {
        guard let surface, ghostty_surface_has_selection(surface) else { return nil }
        var text = ghostty_text_s()
        guard ghostty_surface_read_selection(surface, &text) else { return nil }
        defer { ghostty_surface_free_text(surface, &text) }
        return extractString(from: text)
    }

    private func extractString(from text: ghostty_text_s) -> String? {
        guard let ptr = text.text, text.text_len > 0 else { return nil }
        let len = Int(text.text_len)
        return ptr.withMemoryRebound(to: UInt8.self, capacity: len) { rawPtr in
            String(bytes: UnsafeBufferPointer(start: rawPtr, count: len), encoding: .utf8)
        }
    }

    private var pendingSurfaceCreation = false

    func createSurface() {
        guard surface == nil, !isSessionRecoveryFailed, let app = GhosttyService.shared.app else { return }

        guard let backingSize = backingPixelSize() else {
            pendingSurfaceCreation = true
            return
        }
        pendingSurfaceCreation = false

        let launchCommand = hasMaterializedOnce ? nil : command

        var config = ghostty_surface_config_new()
        config.platform_tag = GHOSTTY_PLATFORM_MACOS
        config.platform = ghostty_platform_u(
            macos: ghostty_platform_macos_s(nsview: Unmanaged.passUnretained(self).toOpaque())
        )
        config.userdata = Unmanaged.passUnretained(self).toOpaque()
        config.scale_factor = Double(window?.backingScaleFactor ?? 2.0)
        config.context = GHOSTTY_SURFACE_CONTEXT_SPLIT

        cleanupSurfaceConfigPointers()

        var cEnvVars: [ghostty_env_var_s] = []
        let localWorkingDirectory = workspaceContext.isRemote
            ? NSHomeDirectory()
            : workingDirectory
        guard let workingDirectoryPointer = strdup(localWorkingDirectory) else { return }
        surfaceCStringPointers.append(workingDirectoryPointer)
        config.working_directory = UnsafePointer(workingDirectoryPointer)

        let startedPersistentSession = workspaceContext.sshDestination == nil
            && persistentSessionID != nil
            && applyPersistentSessionConfiguration(
                &config,
                environment: &cEnvVars,
                launchCommand: launchCommand,
                workingDirectory: localWorkingDirectory
            )

        if let destination = workspaceContext.sshDestination {
            if let remoteWrapped = strdup(TerminalLaunchCommand.remoteShellCommand(
                destination: destination,
                workingDirectory: workingDirectory,
                startupCommand: launchCommand,
                interactive: commandInteractive,
                keepsShellOpen: !commandClosesOnExit
            )) {
                surfaceCStringPointers.append(remoteWrapped)
                config.command = UnsafePointer(remoteWrapped)
                config.wait_after_command = false
            }
        } else if !startedPersistentSession,
                  let command = launchCommand,
                  let loginWrapped = strdup(TerminalLaunchCommand.shellCommand(
                      interactive: commandInteractive,
                      keepsShellOpen: !commandClosesOnExit
                  )),
                  let commandKey = strdup(TerminalLaunchCommand.environmentKey),
                  let commandValue = strdup(command)
        {
            surfaceCStringPointers.append(contentsOf: [loginWrapped, commandKey, commandValue])
            cEnvVars.append(ghostty_env_var_s(key: commandKey, value: commandValue))
            config.command = UnsafePointer(loginWrapped)
            config.wait_after_command = false
        }

        for pair in envVars {
            guard let ck = strdup(pair.key), let cv = strdup(pair.value) else { continue }
            surfaceCStringPointers.append(contentsOf: [ck, cv])
            cEnvVars.append(ghostty_env_var_s(key: ck, value: cv))
        }

        if !cEnvVars.isEmpty {
            let envVarPointer = UnsafeMutablePointer<ghostty_env_var_s>.allocate(capacity: cEnvVars.count)
            envVarPointer.initialize(from: cEnvVars, count: cEnvVars.count)
            surfaceEnvVarPointer = envVarPointer
            surfaceEnvVarCount = cEnvVars.count
            config.env_vars = envVarPointer
            config.env_var_count = cEnvVars.count
        }

        surface = ghostty_surface_new(app, &config)

        if surface == nil {
            cleanupSurfaceConfigPointers()
        }

        guard let surface else { return }

        hasMaterializedOnce = true
        if isOfflinedState {
            isOfflinedState = false
            processExitHandled = false
            onOfflineChange?(false)
        }
        resetOfflineVisibilityClock()

        let scale = Double(window?.backingScaleFactor ?? 2.0)
        ghostty_surface_set_content_scale(surface, scale, scale)

        ghostty_surface_set_size(surface, backingSize.width, backingSize.height)

        reapplyActiveColors()
        applySurfaceConfigurationOverlay()

        if let screen = window?.screen ?? NSScreen.main,
           let displayID = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? UInt32
        {
            ghostty_surface_set_display_id(surface, displayID)
        }

        syncSurfaceFocus()

        if let paneID = TerminalViewRegistry.shared.paneID(for: self) {
            RemoteTerminalStreamer.shared.attach(paneID: paneID, surface: self)
            streamedPaneID = paneID
        }

        applyOcclusionState()
        if let persistentSessionID {
            PersistentSessionService.shared.noteSurfaceMaterialized(sessionID: persistentSessionID)
        }
        startAgentDetection()
    }

    private func applyPersistentSessionConfiguration(
        _ config: inout ghostty_surface_config_s,
        environment: inout [ghostty_env_var_s],
        launchCommand: String?,
        workingDirectory: String
    ) -> Bool {
        guard let persistentSessionID,
              let attachCommand = PersistentSessionService.shared.attachCommand(),
              let attachPointer = strdup(attachCommand)
        else { return false }

        surfaceCStringPointers.append(attachPointer)
        config.command = UnsafePointer(attachPointer)
        config.wait_after_command = false

        let sessionCommand = launchCommand.map { _ in
            TerminalLaunchCommand.shellCommand(
                interactive: commandInteractive,
                keepsShellOpen: !commandClosesOnExit
            )
        }
        var entries = PersistentSessionService.shared.launchEnvironment(
            sessionID: persistentSessionID,
            workingDirectory: workingDirectory,
            command: sessionCommand,
            metadata: sessionMetadata
        )
        if let launchCommand {
            entries.append((key: TerminalLaunchCommand.environmentKey, value: launchCommand))
        }
        for entry in entries {
            guard let key = strdup(entry.key), let value = strdup(entry.value) else { continue }
            surfaceCStringPointers.append(contentsOf: [key, value])
            environment.append(ghostty_env_var_s(key: key, value: value))
        }
        return true
    }

    func destroySurface() {
        surfaceGeneration += 1
        let cancelledWorker = terminalInputQueue.cancelAll()
        scheduleRemoteUploadCleanup(after: cancelledWorker)
        if let surface {
            if let paneID = streamedPaneID {
                RemoteTerminalStreamer.shared.detach(paneID: paneID, surface: self)
                streamedPaneID = nil
            }
            ghostty_surface_free(surface)
            detachRendererLayer()
        }
        stopAgentDetection()
        surface = nil
        surfaceFocused = nil
        cleanupSurfaceConfigPointers()
    }

    func setSurfaceConfigurationOverlay(_ overlay: @escaping (ghostty_surface_t) -> Void) {
        surfaceConfigurationOverlay = overlay
        applySurfaceConfigurationOverlay()
    }

    private func applySurfaceConfigurationOverlay() {
        guard let surface else { return }
        surfaceConfigurationOverlay?(surface)
    }

    func restoreBaseSurfaceConfiguration() {
        guard !hasActiveClientTheme,
              let surface,
              let base = GhosttyService.shared.config
        else { return }
        ghostty_surface_update_config(surface, base)
        reapplyActiveColors()
    }

    private func detachRendererLayer() {
        layer = nil
        wantsLayer = true
    }

    func tearDown() {
        setHandCursor(false)
        onOpenURL = nil
        onCmdClickFile = nil
        resolveCmdHoverFile = nil
        onTitleChange = nil
        onFocus = nil
        onExternalDragHoverChange = nil
        onProcessExit = nil
        onSplitRequest = nil
        canSendToBackground = nil
        onSendToBackground = nil
        onSessionRecoveryFailed = nil
        onSearchStart = nil
        onSearchEnd = nil
        onSearchTotal = nil
        onSearchSelected = nil
        onProgressReport = nil
        onOfflineChange = nil
        onDetectedAgentChange = nil
        if let observer = screenChangeObserver {
            NotificationCenter.default.removeObserver(observer)
            screenChangeObserver = nil
        }
        if let observer = occlusionObserver {
            NotificationCenter.default.removeObserver(observer)
            occlusionObserver = nil
        }
        if let observer = keyWindowObserver {
            NotificationCenter.default.removeObserver(observer)
            keyWindowObserver = nil
        }
        if let observer = keyWindowResignObserver {
            NotificationCenter.default.removeObserver(observer)
            keyWindowResignObserver = nil
        }
        delayedResizeWorkItem?.cancel()
        delayedResizeWorkItem = nil
        destroySurface()
        removeFromSuperview()
    }

    deinit {
        screenChangeObserver.flatMap { NotificationCenter.default.removeObserver($0) }
        occlusionObserver.flatMap { NotificationCenter.default.removeObserver($0) }
        keyWindowObserver.flatMap { NotificationCenter.default.removeObserver($0) }
        keyWindowResignObserver.flatMap { NotificationCenter.default.removeObserver($0) }
        delayedResizeWorkItem?.cancel()
        agentDetectionCoalesced?.cancel()
        if let surface {
            ghostty_surface_free(surface)
        }
        cleanupSurfaceConfigPointers()
    }

    nonisolated private func cleanupSurfaceConfigPointers() {
        surfaceEnvVarPointer?.deinitialize(count: surfaceEnvVarCount)
        surfaceEnvVarPointer?.deallocate()
        surfaceEnvVarPointer = nil
        surfaceEnvVarCount = 0
        surfaceCStringPointers.forEach { free($0) }
        surfaceCStringPointers.removeAll()
    }

    nonisolated(unsafe) private var screenChangeObserver: NSObjectProtocol?
    nonisolated(unsafe) private var keyWindowObserver: NSObjectProtocol?
    nonisolated(unsafe) private var keyWindowResignObserver: NSObjectProtocol?
    nonisolated(unsafe) private var delayedResizeWorkItem: DispatchWorkItem?

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()

        screenChangeObserver.flatMap { NotificationCenter.default.removeObserver($0) }
        screenChangeObserver = nil
        occlusionObserver.flatMap { NotificationCenter.default.removeObserver($0) }
        occlusionObserver = nil
        keyWindowObserver.flatMap { NotificationCenter.default.removeObserver($0) }
        keyWindowObserver = nil
        keyWindowResignObserver.flatMap { NotificationCenter.default.removeObserver($0) }
        keyWindowResignObserver = nil
        delayedResizeWorkItem?.cancel()
        delayedResizeWorkItem = nil

        updateOfflineVisibilityClock()

        guard let window else { return }

        if surface == nil, !isOfflinedState || isPaneVisible {
            createSurface()
        }

        screenChangeObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.didChangeScreenNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.updateMetalLayerSize(deferred: true)
            }
        }

        occlusionObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.didChangeOcclusionStateNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.updateWindowVisibility()
            }
        }

        keyWindowObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.didBecomeKeyNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.syncSurfaceFocus()
            }
        }

        keyWindowResignObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.didResignKeyNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.syncSurfaceFocus()
            }
        }

        updateWindowVisibility()
        updateMetalLayerSize(deferred: true)
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        if pendingSurfaceCreation {
            createSurface()
        }
        updateMetalLayerSize(deferred: false)
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        updateMetalLayerSize(deferred: true)
    }

    override func layout() {
        super.layout()
        scrollbarOverlay.frame = bounds
    }

    func setVisible(_ visible: Bool) {
        guard isPaneVisible != visible else { return }
        isPaneVisible = visible
        updateOfflineVisibilityClock()
        reviveSurfaceIfNeeded()
        applyOcclusionState()
    }

    func setFocused(_ focused: Bool) {
        guard isPaneFocused != focused else { return }
        isPaneFocused = focused
        updateOfflineVisibilityClock()
        reviveSurfaceIfNeeded()
    }

    private func applyOcclusionState() {
        guard let surface else { return }
        ghostty_surface_set_occlusion(surface, isPaneVisible && isWindowVisible)
    }

    private func updateWindowVisibility() {
        let visible = window?.occlusionState.contains(.visible) ?? true
        guard isWindowVisible != visible else { return }
        isWindowVisible = visible
        updateOfflineVisibilityClock()
        reviveSurfaceIfNeeded()
        applyOcclusionState()
    }

    private func reviveSurfaceIfNeeded() {
        guard isOfflinedState, surface == nil, keepsAwake else { return }
        createSurface()
    }

    func wake() {
        guard isOfflinedState, surface == nil else { return }
        isPaneVisible = true
        isPaneFocused = true
        updateOfflineVisibilityClock()
        createSurface()
        applyOcclusionState()
    }

    private var isCurrentlyVisible: Bool {
        window != nil && isPaneVisible && isWindowVisible
    }

    private var keepsAwake: Bool {
        TerminalOfflinePolicy.keepsAwake(isOnScreen: isCurrentlyVisible, isFocused: isPaneFocused)
    }

    private func updateOfflineVisibilityClock() {
        if keepsAwake {
            offlineInvisibleAt = nil
        } else if offlineInvisibleAt == nil {
            offlineInvisibleAt = Date()
        }
    }

    private func resetOfflineVisibilityClock() {
        offlineInvisibleAt = keepsAwake ? nil : Date()
    }

    func updateResumeWorkingDirectory(_ directory: String) {
        workingDirectory = directory
    }

    func isTerminalIdle() -> Bool {
        guard let surface else { return true }
        if let persistentSessionID {
            return TerminalPersistentSessionPolicy.isIdle(
                activity: PersistentSessionService.shared.commandActivity(sessionID: persistentSessionID),
                isShellCommandRunning: shellActivityTracker.isCommandRunning,
                isAlternateScreen: isAlternateScreenActive(surface: surface)
            )
        }
        if ghostty_surface_needs_confirm_quit(surface) {
            return false
        }
        let foregroundPID = ghostty_surface_foreground_pid(surface)
        return TerminalOfflinePolicy.isIdle(
            hasRunningProcess: TerminalOfflinePolicy.hasRunningProcess(
                foregroundProcessName: Self.processName(pid: foregroundPID),
                foregroundProcessArguments: ProcessArgumentsInspector.invocation(pid: foregroundPID)?.arguments,
                isShellCommandRunning: shellActivityTracker.isCommandRunning
            ),
            isAlternateScreen: isAlternateScreenActive(surface: surface)
        )
    }

    var isOfflineBlockedByRemote: Bool {
        guard let paneID = TerminalViewRegistry.shared.paneID(for: self) else { return false }
        return TerminalViewRegistry.shared.isOwnedByRemote(paneID)
    }

    func takeOffline() {
        guard isEligibleForOffline else { return }
        CATransaction.begin()
        CATransaction.setCompletionBlock { [weak self] in
            MainActor.assumeIsolated {
                self?.performOfflineTeardown()
            }
        }
        CATransaction.commit()
    }

    private var isEligibleForOffline: Bool {
        surface != nil && !keepsAwake && offlineInvisibleAt != nil
            && !isOfflineBlockedByRemote && isTerminalIdle()
    }

    private func performOfflineTeardown() {
        guard isEligibleForOffline else { return }
        processExitHandled = true
        destroySurface()
        isOfflinedState = true
        onOfflineChange?(true)
    }

    func reattachPersistentSession() {
        guard persistentSessionID != nil else { return }
        destroySurface()
        isSessionRecoveryFailed = false
        onSessionRecoveryFailed?(false)
        processExitHandled = false
        createSurface()
        applyOcclusionState()
    }

    func reportSessionRecoveryFailure() {
        guard persistentSessionID != nil, !isSessionRecoveryFailed else { return }
        destroySurface()
        isSessionRecoveryFailed = true
        onSessionRecoveryFailed?(true)
    }

    func applyColorScheme(isDark: Bool) {
        guard let surface else { return }
        ghostty_surface_set_color_scheme(surface, isDark ? GHOSTTY_COLOR_SCHEME_DARK : GHOSTTY_COLOR_SCHEME_LIGHT)
    }

    func applyClientTheme(_ theme: ClientThemeDTO?) {
        guard let surface else { return }
        if let theme {
            ClientThemeApplier.apply(theme, to: surface)
            return
        }
        ClientThemeApplier.revert(surface)
        applyColorScheme(isDark: ThemeService.isCurrentAppearanceDark())
        applySurfaceConfigurationOverlay()
    }

    func reapplyActiveColors() {
        guard surface != nil else { return }
        if let theme = activeClientTheme() {
            applyClientTheme(theme)
            return
        }
        applyColorScheme(isDark: ThemeService.isCurrentAppearanceDark())
    }

    func reapplyClientThemeIfOwned() {
        guard surface != nil, let theme = activeClientTheme() else { return }
        applyClientTheme(theme)
    }

    var hasActiveClientTheme: Bool {
        activeClientTheme() != nil
    }

    private func activeClientTheme() -> ClientThemeDTO? {
        guard let paneID = TerminalViewRegistry.shared.paneID(for: self),
              let clientID = PaneOwnershipStore.shared.remoteOwner(for: paneID)
        else { return nil }
        return ClientThemeStore.shared.theme(for: clientID)
    }

    private func updateMetalLayerSize(deferred: Bool) {
        if deferred {
            delayedResizeWorkItem?.cancel()
            DispatchQueue.main.async { [weak self] in
                self?.updateMetalLayerSize(deferred: false)
            }
            let workItem = DispatchWorkItem { [weak self] in
                self?.updateMetalLayerSize(deferred: false)
            }
            delayedResizeWorkItem = workItem
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1, execute: workItem)
            return
        }

        guard let surface, let window else { return }
        layer?.contentsScale = window.backingScaleFactor
        layoutSubtreeIfNeeded()

        guard let backingSize = backingPixelSize() else { return }

        let scale = Double(window.backingScaleFactor)

        ghostty_surface_set_content_scale(surface, scale, scale)

        if let screen = window.screen,
           let displayID = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? UInt32
        {
            ghostty_surface_set_display_id(surface, displayID)
        }

        if let paneID = TerminalViewRegistry.shared.paneID(for: self),
           TerminalViewRegistry.shared.isOwnedByRemote(paneID)
        {
            return
        }

        ghostty_surface_set_size(surface, backingSize.width, backingSize.height)
    }

    func remoteOwnershipDidChange() {
        updateMetalLayerSize(deferred: false)
    }

    func materializeHeadless() {
        guard surface == nil else { return }
        if frame.size.width <= 0 || frame.size.height <= 0 {
            setFrameSize(NSSize(width: 1, height: 1))
        }
        createSurface()
    }

    private func backingPixelSize() -> (width: UInt32, height: UInt32)? {
        let size = convertToBacking(bounds).size
        let width = Int(floor(size.width))
        let height = Int(floor(size.height))
        guard width > 0, height > 0 else { return nil }
        return (UInt32(width), UInt32(height))
    }

    private func isAppShortcut(_ event: NSEvent) -> Bool {
        let key = KeyCombo.normalized(key: event.charactersIgnoringModifiers ?? "", keyCode: event.keyCode)
        let modifiers = event.modifierFlags.intersection(KeyCombo.supportedModifierMask)
        if modifiers == .command, Self.systemShortcutKeys.contains(key) {
            return true
        }
        let scopes = ShortcutContext.activeScopes(for: window, isTerminalFocused: true)
        return KeyBindingStore.shared.isRegisteredShortcut(event: event, scopes: scopes)
            || CommandShortcutStore.shared.isRegisteredShortcut(event: event, scopes: scopes)
            || ExtensionShortcutStore.shared.isRegisteredShortcut(event: event, scopes: scopes)
    }

    private static let systemShortcutKeys: Set<String> = ["q", "h", "m", ","]

    func needsConfirmQuit() -> Bool {
        guard let surface else { return false }
        if let persistentSessionID {
            return PersistentSessionService.shared.isRunningCommand(sessionID: persistentSessionID)
        }
        return ghostty_surface_needs_confirm_quit(surface)
    }

    func notifySurfaceFocused() {
        setSurfaceFocused(true)
    }

    func notifySurfaceUnfocused() {
        setSurfaceFocused(false)
    }

    private func syncSurfaceFocus() {
        setSurfaceFocused(Self.desiredSurfaceFocus(
            overlayActive: overlayActive,
            isKeyWindow: window?.isKeyWindow == true,
            isFirstResponder: window?.firstResponder === self || window?.firstResponder === inputContext
        ))
    }

    private func setSurfaceFocused(_ focused: Bool) {
        guard let surface else {
            surfaceFocused = nil
            return
        }
        guard Self.shouldApplySurfaceFocusChange(previous: surfaceFocused, next: focused) else {
            surfaceFocused = focused
            return
        }
        ghostty_surface_set_focus(surface, focused)
        surfaceFocused = focused
    }

    static func shouldApplySurfaceFocusChange(previous: Bool?, next: Bool) -> Bool {
        previous != next && (next || previous != nil)
    }

    static func desiredSurfaceFocus(overlayActive: Bool, isKeyWindow: Bool, isFirstResponder: Bool) -> Bool {
        !overlayActive && isKeyWindow && isFirstResponder
    }

    override var acceptsFirstResponder: Bool { !overlayActive }

    override func becomeFirstResponder() -> Bool {
        let result = super.becomeFirstResponder()
        if result {
            setSurfaceFocused(true)
            if !isFocused {
                DispatchQueue.main.async { [weak self] in
                    self?.onFocus?()
                }
            }
        }
        return result
    }

    override func resignFirstResponder() -> Bool {
        let result = super.resignFirstResponder()
        if result {
            setSurfaceFocused(false)
        }
        return result
    }

    private var currentTrackingArea: NSTrackingArea?

    private func setupTrackingArea() {
        if let existing = currentTrackingArea {
            removeTrackingArea(existing)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .mouseMoved, .activeInKeyWindow, .inVisibleRect],
            owner: self
        )
        addTrackingArea(area)
        currentTrackingArea = area
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        setupTrackingArea()
    }

    override func keyDown(with event: NSEvent) {
        if overlayActive {
            return
        }
        guard surface != nil else { super.keyDown(with: event)
            return
        }

        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)

        if Self.isImagePasteShortcut(keyCode: event.keyCode, modifierFlags: flags),
           schedulePasteboardAttachment()
        {
            return
        }
        performOrDefer { $0.processKeyDown(event) }
    }

    private func performOrDefer(_ operation: @escaping @MainActor (GhosttyTerminalNSView) -> Void) {
        let deferred = terminalInputQueue.deferIfPending { [weak self] in
            guard let self else { return }
            operation(self)
        }
        guard !deferred else { return }
        operation(self)
    }

    private func processKeyDown(_ event: NSEvent) {
        guard let surface else { return }
        let action: ghostty_input_action_e = event.isARepeat ? GHOSTTY_ACTION_REPEAT : GHOSTTY_ACTION_PRESS
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let optionAsAlt = translatedOptionAsAlt(for: event)
        if flags.contains(.control), !flags.contains(.command), !flags.contains(.option), !hasMarkedText() {
            if isAppShortcut(event) {
                return
            }
            var keyEvent = buildKeyEvent(from: event, action: action)
            let text = shortcutText(from: event)
            if text.isEmpty {
                keyEvent.text = nil
                _ = ghostty_surface_key(surface, keyEvent)
            } else {
                text.withCString { ptr in
                    keyEvent.text = ptr
                    recordTextInput(text)
                    _ = ghostty_surface_key(surface, keyEvent)
                }
            }
            return
        }

        if flags.contains(.command) {
            if isAppShortcut(event) {
                return
            }
            var keyEvent = buildKeyEvent(from: event, action: action)
            keyEvent.text = nil
            _ = ghostty_surface_key(surface, keyEvent)
            return
        }

        let hadMarkedText = hasMarkedText()
        currentKeyEvent = event
        keyTextAccumulator = []
        commandSelectorCalled = false
        let interpretEvent = optionAsAlt ? eventStrippingOption(event) : event
        interpretKeyEvents([interpretEvent])
        currentKeyEvent = nil

        syncPreedit(clearIfNeeded: hadMarkedText)

        let commandWasCalled = commandSelectorCalled

        if !keyTextAccumulator.isEmpty {
            for text in keyTextAccumulator {
                var keyEvent = buildKeyEvent(from: event, action: action)
                keyEvent.consumed_mods = commandWasCalled ? GHOSTTY_MODS_NONE : consumedModsFromFlags(
                    flags,
                    consumeOption: !optionAsAlt
                )
                text.withCString { ptr in
                    keyEvent.text = ptr
                    recordTextInput(text)
                    _ = ghostty_surface_key(surface, keyEvent)
                }
            }
        } else {
            var keyEvent = buildKeyEvent(from: event, action: action)
            keyEvent.consumed_mods = commandWasCalled ? GHOSTTY_MODS_NONE : consumedModsFromFlags(
                flags,
                consumeOption: !optionAsAlt
            )
            keyEvent.composing = hasMarkedText() || hadMarkedText

            let text = filterSpecialCharacters(event.characters ?? "")
            if !text.isEmpty, !keyEvent.composing {
                text.withCString { ptr in
                    keyEvent.text = ptr
                    recordTextInput(text)
                    _ = ghostty_surface_key(surface, keyEvent)
                }
            } else {
                recordSpecialKey(event)
                keyEvent.consumed_mods = GHOSTTY_MODS_NONE
                keyEvent.text = nil
                _ = ghostty_surface_key(surface, keyEvent)
            }
        }
    }

    override func doCommand(by selector: Selector) {
        commandSelectorCalled = true
    }

    override func insertText(_ insertString: Any) {
        insertText(insertString, replacementRange: NSRange(location: NSNotFound, length: 0))
    }

    override func keyUp(with event: NSEvent) {
        if overlayActive {
            return
        }
        performOrDefer { $0.processKeyUp(event) }
    }

    private func processKeyUp(_ event: NSEvent) {
        guard let surface else { return }
        var keyEvent = buildKeyEvent(from: event, action: GHOSTTY_ACTION_RELEASE)
        keyEvent.text = nil
        _ = ghostty_surface_key(surface, keyEvent)
    }

    override func flagsChanged(with event: NSEvent) {
        if overlayActive {
            return
        }
        performOrDefer { $0.processFlagsChanged(event) }
    }

    private func processFlagsChanged(_ event: NSEvent) {
        guard let surface else { return }
        if hasMarkedText() {
            return
        }
        let action: ghostty_input_action_e = isFlagPress(event) ? GHOSTTY_ACTION_PRESS : GHOSTTY_ACTION_RELEASE
        var keyEvent = buildKeyEvent(from: event, action: action)
        keyEvent.text = nil
        _ = ghostty_surface_key(surface, keyEvent)
        updateCmdHoverCursor(modifierFlags: event.modifierFlags)
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if isAppShortcut(event) {
            return false
        }
        if overlayActive {
            return false
        }
        guard window?.firstResponder === self || window?.firstResponder === inputContext else { return false }
        guard event.type == .keyDown, let surface else { return false }

        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let hasActionModifier = flags.contains(.command) || flags.contains(.control) || flags.contains(.option)
        guard hasActionModifier else { return false }

        if Self.isImagePasteShortcut(keyCode: event.keyCode, modifierFlags: flags),
           schedulePasteboardAttachment()
        {
            return true
        }

        var keyEvent = buildKeyEvent(from: event, action: event.isARepeat ? GHOSTTY_ACTION_REPEAT : GHOSTTY_ACTION_PRESS)
        keyEvent.text = nil
        guard ghostty_surface_key_is_binding(surface, keyEvent, nil) else { return false }
        performOrDefer { $0.processKeyEquivalent(event) }
        return true
    }

    private func processKeyEquivalent(_ event: NSEvent) {
        guard let surface else { return }
        var keyEvent = buildKeyEvent(from: event, action: event.isARepeat ? GHOSTTY_ACTION_REPEAT : GHOSTTY_ACTION_PRESS)
        keyEvent.text = nil
        _ = ghostty_surface_key(surface, keyEvent)
    }

    private func mousePoint(from event: NSEvent) -> NSPoint {
        let local = convert(event.locationInWindow, from: nil)
        return NSPoint(x: local.x, y: bounds.height - local.y)
    }

    @discardableResult
    static func routeMouseDown(
        commandFileToken: String?,
        openCommandFile: (String) -> Void,
        focusTerminal: () -> Void
    ) -> Bool {
        guard let commandFileToken else {
            focusTerminal()
            return false
        }
        openCommandFile(commandFileToken)
        return true
    }

    private func claimFocusForMouseDown() {
        let alreadyFirstResponder = window?.firstResponder === self
        window?.makeFirstResponder(self)
        if alreadyFirstResponder {
            setSurfaceFocused(true)
            DispatchQueue.main.async { [weak self] in
                self?.onFocus?()
            }
        }
    }

    enum LeftMousePressRouting {
        case ignored
        case forwardedToSurface
        case commandPending
        case commandHandled
    }

    static func forwardsLeftMousePress(commandHeld: Bool) -> Bool {
        !commandHeld
    }

    static func forwardsLeftMouseRelease(routing: LeftMousePressRouting, didDrag: Bool) -> Bool {
        switch routing {
        case .forwardedToSurface:
            true
        case .commandPending:
            !didDrag
        case .ignored,
             .commandHandled:
            false
        }
    }

    static func reachesSurfaceWhileOverlayActive(routing: LeftMousePressRouting) -> Bool {
        reachesSurfaceWhileOverlayActive(forwardedPress: routing == .forwardedToSurface)
    }

    static func reachesSurfaceWhileOverlayActive(forwardedPress: Bool) -> Bool {
        forwardedPress
    }

    override func mouseDown(with event: NSEvent) {
        leftMousePressRouting = .ignored
        leftMouseDownWindowPoint = event.locationInWindow
        didDragAfterLeftMouseDown = false
        if overlayActive {
            return
        }
        guard let surface else { return }
        let pt = mousePoint(from: event)
        ghostty_surface_mouse_pos(surface, pt.x, pt.y, modsFromEvent(event))
        let commandHeld = event.modifierFlags.contains(.command)
        let commandWord = commandHeld && !hasOSC8LinkUnderCursor
            ? readQuicklookWordUnderMouse()
            : nil
        let commandFileToken = commandWord.flatMap { resolvedCmdFileToken(for: $0)?.text }
        if Self.routeMouseDown(
            commandFileToken: commandFileToken,
            openCommandFile: { onCmdClickFile?($0) },
            focusTerminal: claimFocusForMouseDown
        ) {
            leftMousePressRouting = .commandHandled
            return
        }
        if commandWord != nil {
            leftMousePressRouting = .commandPending
            return
        }
        guard Self.forwardsLeftMousePress(commandHeld: commandHeld) else {
            leftMousePressRouting = .commandPending
            return
        }
        leftMousePressRouting = .forwardedToSurface
        _ = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_LEFT, modsFromEvent(event))
    }

    override func mouseUp(with event: NSEvent) {
        let routing = leftMousePressRouting
        let didDrag = didDragAfterLeftMouseDown
        leftMousePressRouting = .ignored
        leftMouseDownWindowPoint = nil
        didDragAfterLeftMouseDown = false
        if overlayActive, !Self.reachesSurfaceWhileOverlayActive(routing: routing) {
            return
        }
        guard let surface else { return }
        let pt = mousePoint(from: event)
        ghostty_surface_mouse_pos(surface, pt.x, pt.y, modsFromEvent(event))
        guard Self.forwardsLeftMouseRelease(routing: routing, didDrag: didDrag) else { return }
        _ = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_LEFT, modsFromEvent(event))
        autoCopySelectionIfEnabled()
    }

    private func autoCopySelectionIfEnabled() {
        guard UserDefaults.standard.bool(forKey: GeneralSettingsKeys.autoCopyTerminalSelection) else { return }
        guard let selection = readSelectionText(), !selection.isEmpty else { return }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(selection, forType: .string)
        ToastState.shared.show(L10n.string("Copied"))
    }

    override func mouseDragged(with event: NSEvent) {
        updateLeftDragState(with: event)
        mouseMoved(with: event)
    }

    private func updateLeftDragState(with event: NSEvent) {
        guard !didDragAfterLeftMouseDown, let origin = leftMouseDownWindowPoint else { return }
        didDragAfterLeftMouseDown = DragActivation.reachesDistance(from: origin, to: event.locationInWindow)
    }

    override func rightMouseDragged(with event: NSEvent) {
        mouseMoved(with: event)
    }

    override func otherMouseDragged(with event: NSEvent) {
        mouseMoved(with: event)
    }

    override func mouseMoved(with event: NSEvent) {
        guard let surface else { return }
        let pt = mousePoint(from: event)
        lastMouseTopDownPoint = pt
        ghostty_surface_mouse_pos(surface, pt.x, pt.y, modsFromEvent(event))
        updateCmdHoverCursor(modifierFlags: event.modifierFlags)
        scrollbarOverlay.flashIfNearScroller(point: convert(event.locationInWindow, from: nil))
    }

    override func mouseExited(with event: NSEvent) {
        super.mouseExited(with: event)
        setHandCursor(false)
        hideFileHoverUnderline()
    }

    private func updateCmdHoverCursor(modifierFlags: NSEvent.ModifierFlags) {
        guard modifierFlags.contains(.command) else {
            setHandCursor(false)
            hideFileHoverUnderline()
            return
        }
        if hasOSC8LinkUnderCursor {
            setHandCursor(true)
            hideFileHoverUnderline()
            return
        }
        guard let word = readQuicklookWordUnderMouse(),
              let token = resolvedCmdFileToken(for: word)
        else {
            setHandCursor(false)
            hideFileHoverUnderline()
            return
        }
        showFileHoverUnderline(for: word, token: token)
        setHandCursor(true)
    }

    func refreshCmdHoverCursor() {
        updateCmdHoverCursor(modifierFlags: NSEvent.modifierFlags)
    }

    private func setHandCursor(_ on: Bool) {
        guard on != isShowingHandCursor else { return }
        isShowingHandCursor = on
        if on {
            NSCursor.pointingHand.push()
        } else {
            NSCursor.pop()
        }
    }

    private struct QuicklookWord {
        let text: String
        let topLeftPoints: CGPoint
    }

    private struct CmdFileToken {
        let text: String
        let underlineSegments: [FileUnderlineSegment]
        let cols: Int
    }

    private struct FileUnderlineSegment {
        let row: Int
        let startColumn: Int
        let endColumn: Int
    }

    private struct ScreenSnapshot {
        let lines: [String]
        let cols: Int
    }

    private struct PathFragment {
        let text: String
        let startColumn: Int
        let endColumn: Int
    }

    private func readQuicklookWordUnderMouse() -> QuicklookWord? {
        guard let surface else { return nil }
        var text = ghostty_text_s()
        guard ghostty_surface_quicklook_word(surface, &text) else { return nil }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let value = extractString(from: text) else { return nil }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        return QuicklookWord(
            text: trimmed,
            topLeftPoints: CGPoint(x: text.tl_px_x, y: text.tl_px_y)
        )
    }

    private func resolvedCmdFileToken(for word: QuicklookWord) -> CmdFileToken? {
        if resolveCmdHoverFile?(word.text) == true {
            return CmdFileToken(text: word.text, underlineSegments: [], cols: 0)
        }
        return wrappedCmdFileTokenCandidates(for: word).first { resolveCmdHoverFile?($0.text) == true }
    }

    private func wrappedCmdFileTokenCandidates(for word: QuicklookWord) -> [CmdFileToken] {
        guard let snapshot = readScreenSnapshot(),
              let row = screenRow(for: word, lines: snapshot.lines)
        else {
            return []
        }
        return Self.wrappedFileTokenCandidatesWithFragments(
            word: word.text,
            row: row,
            lines: snapshot.lines,
            cols: snapshot.cols
        )
    }

    private func readScreenSnapshot() -> ScreenSnapshot? {
        guard let surface else { return nil }
        var out = ghostty_cells_s()
        guard ghostty_surface_read_cells(surface, &out) else { return nil }
        defer { ghostty_surface_free_cells(surface, &out) }

        let cols = Int(out.cols)
        let rows = Int(out.rows)
        guard cols > 0, rows > 0, let cells = out.cells else { return nil }

        var lines: [String] = []
        lines.reserveCapacity(rows)
        for row in 0 ..< rows {
            var line = ""
            for col in 0 ..< cols {
                let cell = cells[row * cols + col]
                let cp = cell.codepoint
                if cp == 0 {
                    line.append(" ")
                } else if let scalar = Unicode.Scalar(cp) {
                    line.append(Character(scalar))
                } else {
                    line.append(" ")
                }
            }
            lines.append(line)
        }
        return ScreenSnapshot(lines: lines, cols: cols)
    }

    private func screenRow(for word: QuicklookWord, lines: [String]) -> Int? {
        guard let rowHeight = terminalRowHeight(), rowHeight > 0 else { return nil }
        let y = lastMouseTopDownPoint?.y ?? word.topLeftPoints.y
        let row = Int(floor(y / rowHeight))
        guard row >= 0, row < lines.count else { return nil }
        return row
    }

    static func wrappedFileTokenCandidates(word: String, row: Int, lines: [String]) -> [String] {
        wrappedFileTokenCandidatesWithFragments(word: word, row: row, lines: lines, cols: 0).map(\.text)
    }

    private static func wrappedFileTokenCandidatesWithFragments(
        word: String,
        row: Int,
        lines: [String],
        cols: Int
    ) -> [CmdFileToken] {
        guard row >= 0, row < lines.count,
              let anchor = matchingPathFragment(word: word, line: lines[row])
        else { return [] }

        var fragments: [(row: Int, fragment: PathFragment)] = [(row, anchor)]

        while fragments.count < maxWrappedFileTokenRows,
              let first = fragments.first,
              first.row > 0,
              let previous = trailingPathFragment(from: lines[first.row - 1]),
              shouldPrepend(previous.text, before: joinedPathText(fragments))
        {
            fragments.insert((first.row - 1, previous), at: 0)
        }

        while fragments.count < maxWrappedFileTokenRows,
              let last = fragments.last,
              last.row + 1 < lines.count,
              let next = leadingPathFragment(from: lines[last.row + 1]),
              shouldAppend(next.text, after: joinedPathText(fragments))
        {
            fragments.append((last.row + 1, next))
        }

        let text = joinedPathText(fragments)
        guard text != word else { return [] }
        let segments = fragments.map {
            FileUnderlineSegment(
                row: $0.row,
                startColumn: $0.fragment.startColumn,
                endColumn: $0.fragment.endColumn
            )
        }
        return [CmdFileToken(text: text, underlineSegments: segments, cols: cols)]
    }

    private static let wrappedPathScalars =
        CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789/._-+@~:$%#=&")
    private static let maxWrappedFileTokenRows = 6

    private static func matchingPathFragment(word: String, line: String) -> PathFragment? {
        [leadingPathFragment(from: line), trailingPathFragment(from: line)]
            .compactMap(\.self)
            .first { fragment in
                fragment.text == word
                    || fragment.text == ":\(word)"
                    || ((fragment.text.contains("/") || fragment.text.hasPrefix(":")) && fragment.text.contains(word))
            }
    }

    private static func joinedPathText(_ fragments: [(row: Int, fragment: PathFragment)]) -> String {
        fragments.map(\.fragment.text).joined()
    }

    private static func shouldPrepend(_ previous: String, before current: String) -> Bool {
        previous.contains("/") || previous.hasPrefix("~") || current.hasPrefix(":")
    }

    private static func shouldAppend(_ next: String, after current: String) -> Bool {
        current.contains("/") || current.hasPrefix("~") || next.hasPrefix(":")
    }

    private static func leadingPathFragment(from line: String) -> PathFragment? {
        let scalars = Array(line.unicodeScalars)
        guard let start = scalars.firstIndex(where: { !CharacterSet.whitespaces.contains($0) }) else { return nil }
        var end = start
        while end < scalars.count, wrappedPathScalars.contains(scalars[end]) {
            end += 1
        }
        guard end > start else { return nil }
        let fragment = String(String.UnicodeScalarView(scalars[start ..< end]))
        return PathFragment(text: fragment, startColumn: start, endColumn: end)
    }

    private static func trailingPathFragment(from line: String) -> PathFragment? {
        let scalars = Array(line.unicodeScalars)
        guard let last = scalars.lastIndex(where: { !CharacterSet.whitespaces.contains($0) }) else { return nil }
        var start = last
        while start > 0, wrappedPathScalars.contains(scalars[start - 1]) {
            start -= 1
        }
        guard wrappedPathScalars.contains(scalars[last]) else { return nil }
        let end = last + 1
        let fragment = String(String.UnicodeScalarView(scalars[start ..< end]))
        return PathFragment(text: fragment, startColumn: start, endColumn: end)
    }

    private func showFileHoverUnderline(for word: QuicklookWord, token: CmdFileToken) {
        guard let layer else { return }
        let underlineLayer = fileHoverUnderlineLayer ?? CAShapeLayer()
        if fileHoverUnderlineLayer == nil {
            underlineLayer.fillColor = nil
            underlineLayer.isGeometryFlipped = true
            layer.addSublayer(underlineLayer)
            fileHoverUnderlineLayer = underlineLayer
        }

        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        let font = quicklookFont()
        let textSize = (word.text as NSString).size(withAttributes: [.font: font])
        let rowHeight = terminalRowHeight() ?? max(textSize.height, font.ascender - font.descender + font.leading)
        let thickness = max(font.underlineThickness, 1 / scale)

        let path = CGMutablePath()
        if !token.underlineSegments.isEmpty, token.cols > 0 {
            let cellWidth = bounds.width / CGFloat(token.cols)
            for segment in token.underlineSegments {
                let x = CGFloat(segment.startColumn) * cellWidth
                let y = CGFloat(segment.row) * rowHeight + rowHeight - max(2, font.underlineThickness)
                let width = max(CGFloat(segment.endColumn - segment.startColumn) * cellWidth, 1)
                path.move(to: CGPoint(x: x, y: y))
                path.addLine(to: CGPoint(x: x + width, y: y))
            }
        } else {
            let x = word.topLeftPoints.x
            let mouseY = lastMouseTopDownPoint?.y ?? word.topLeftPoints.y
            let rowTopY = floor(mouseY / rowHeight) * rowHeight
            let y = rowTopY + rowHeight - max(2, font.underlineThickness)
            let width = max(textSize.width, 1)
            path.move(to: CGPoint(x: x, y: y))
            path.addLine(to: CGPoint(x: x + width, y: y))
        }

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        underlineLayer.frame = bounds
        underlineLayer.contentsScale = scale
        underlineLayer.path = path
        underlineLayer.strokeColor = NSColor.controlAccentColor.cgColor
        underlineLayer.lineWidth = thickness
        underlineLayer.isHidden = false
        CATransaction.commit()
    }

    private func hideFileHoverUnderline() {
        fileHoverUnderlineLayer?.isHidden = true
    }

    private func quicklookFont() -> NSFont {
        guard let surface,
              let fontPtr = ghostty_surface_quicklook_font(surface)
        else {
            return .monospacedSystemFont(ofSize: NSFont.systemFontSize, weight: .regular)
        }
        return Unmanaged<NSFont>.fromOpaque(fontPtr).takeUnretainedValue()
    }

    private func terminalRowHeight() -> CGFloat? {
        guard let surface else { return nil }
        var cells = ghostty_cells_s()
        guard ghostty_surface_read_cells(surface, &cells) else { return nil }
        defer { ghostty_surface_free_cells(surface, &cells) }
        guard cells.rows > 0 else { return nil }
        return bounds.height / CGFloat(cells.rows)
    }

    static func forwardsRightMouseButton(mouseCaptured: Bool, shiftHeld: Bool) -> Bool {
        mouseCaptured && !shiftHeld
    }

    override func rightMouseDown(with event: NSEvent) {
        forwardedRightMousePress = false
        if overlayActive {
            return
        }
        guard let surface else { return }
        let pt = mousePoint(from: event)
        ghostty_surface_mouse_pos(surface, pt.x, pt.y, modsFromEvent(event))
        guard Self.forwardsRightMouseButton(
            mouseCaptured: ghostty_surface_mouse_captured(surface),
            shiftHeld: event.modifierFlags.contains(.shift)
        )
        else {
            presentContextMenu(with: event)
            return
        }
        forwardedRightMousePress = true
        let consumed = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_RIGHT, modsFromEvent(event))
        guard !consumed else { return }
        forwardedRightMousePress = false
        _ = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_RIGHT, modsFromEvent(event))
        presentContextMenu(with: event)
    }

    override func rightMouseUp(with event: NSEvent) {
        let forwardedPress = forwardedRightMousePress
        forwardedRightMousePress = false
        if overlayActive, !Self.reachesSurfaceWhileOverlayActive(forwardedPress: forwardedPress) {
            return
        }
        guard let surface else { return }
        let pt = mousePoint(from: event)
        ghostty_surface_mouse_pos(surface, pt.x, pt.y, modsFromEvent(event))
        guard forwardedPress else {
            super.rightMouseUp(with: event)
            return
        }
        let consumed = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_RIGHT, modsFromEvent(event))
        if !consumed {
            super.rightMouseUp(with: event)
        }
    }

    private func presentContextMenu(with event: NSEvent) {
        let menu = makeContextMenu()
        NSMenu.popUpContextMenu(menu, with: event, for: self)
    }

    func makeContextMenu(
        settingsFocusCoordinator: SettingsFocusCoordinator = .shared
    ) -> NSMenu {
        let menu = NSMenu(title: L10n.string("Terminal"))
        menu.autoenablesItems = false

        let paste = ClosureMenuItem(title: L10n.string("Paste")) { [weak self] in
            self?.performContextPaste()
        }
        paste.isEnabled = pasteboardHasPastableContent()
        menu.addItem(paste)

        menu.addItem(.separator())

        let sendToBackground = ClosureMenuItem(title: L10n.string("Send to Background")) { [weak self] in
            self?.onSendToBackground?()
        }
        sendToBackground.isEnabled = persistentSessionID != nil && canSendToBackground?() == true
        menu.addItem(sendToBackground)

        menu.addItem(.separator())

        menu.addItem(contextSplitMenuItem(title: L10n.string("Split Right"), direction: .horizontal, position: .second))
        menu.addItem(contextSplitMenuItem(title: L10n.string("Split Left"), direction: .horizontal, position: .first))
        menu.addItem(contextSplitMenuItem(title: L10n.string("Split Down"), direction: .vertical, position: .second))
        menu.addItem(contextSplitMenuItem(title: L10n.string("Split Up"), direction: .vertical, position: .first))

        menu.addItem(.separator())

        let settings = ClosureMenuItem(title: L10n.string("Terminal Settings…")) {
            settingsFocusCoordinator.openSettings(focusedOn: .terminal)
        }
        menu.addItem(settings)

        return menu
    }

    private func contextSplitMenuItem(title: String, direction: SplitDirection, position: SplitPosition) -> NSMenuItem {
        ClosureMenuItem(title: title) { [weak self] in
            self?.onSplitRequest?(direction, position)
        }
    }

    private func performContextPaste() {
        window?.makeFirstResponder(self)
        if schedulePasteboardAttachment() {
            return
        }
        guard let text = NSPasteboard.general.string(forType: .string), !text.isEmpty else { return }
        insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
    }

    private func schedulePasteboardAttachment() -> Bool {
        if pasteboardHasImage() {
            scheduleSystemImagePaste()
            return true
        }
        let urls = pasteboardFileURLs()
        guard !urls.isEmpty else { return false }
        insertPaths(for: urls)
        return true
    }

    private func pasteboardFileURLs() -> [URL] {
        guard NSPasteboard.general.string(forType: .string) == nil else { return [] }
        return PasteboardFileURLs.urls(in: .general)
    }

    private func pasteboardHasPastableContent() -> Bool {
        if let text = NSPasteboard.general.string(forType: .string) {
            return !text.isEmpty
        }
        return pasteboardHasImage() || !PasteboardFileURLs.urls(in: .general).isEmpty
    }

    nonisolated static func isImagePasteShortcut(
        keyCode: UInt16,
        modifierFlags: NSEvent.ModifierFlags
    ) -> Bool {
        guard keyCode == 9, !modifierFlags.contains(.option) else { return false }
        return modifierFlags.contains(.command) != modifierFlags.contains(.control)
    }

    private func pasteboardHasImage() -> Bool {
        ImagePasteData.hasImage()
    }

    private func scheduleSystemImagePaste() {
        guard uploadDestination != nil else {
            sendRemoteBytes(TerminalControlBytes.pasteShortcut)
            return
        }
        let sourceData: Data
        do {
            sourceData = try ImagePasteData.sourceData()
        } catch {
            ToastState.shared.show(error.localizedDescription)
            return
        }
        terminalInputQueue.enqueue { [weak self] in
            guard !Task.isCancelled, let attempt = self?.beginUpload() else { return }
            do {
                let pngData = try await ImagePasteData.pngData(from: sourceData)
                guard !Task.isCancelled, let self else { return }
                _ = await self.pasteImageData(pngData, attempt: attempt)
            } catch {
                guard !Task.isCancelled else { return }
                ToastState.shared.show(error.localizedDescription)
            }
        }
    }

    @objc
    func paste(_: Any?) {
        performContextPaste()
    }

    override func scrollWheel(with event: NSEvent) {
        guard let surface else { return }
        var mods: ghostty_input_scroll_mods_t = 0
        if event.hasPreciseScrollingDeltas {
            mods |= 1
        }
        ghostty_surface_mouse_scroll(surface, event.scrollingDeltaX, event.scrollingDeltaY, mods)
        scrollbarOverlay.flash()
    }

    private func installScrollbarOverlay() {
        scrollbarOverlay.onScrollToRow = { [weak self] row in
            self?.scrollToRow(row)
        }
        addSubview(scrollbarOverlay, positioned: .above, relativeTo: nil)
    }

    func updateScrollbar(total: Int, offset: Int, len: Int) {
        scrollbarOverlay.update(total: total, offset: offset, len: len, cellHeight: cellHeightPoints())
    }

    private func cellHeightPoints() -> CGFloat {
        guard let surface else { return 0 }
        let size = ghostty_surface_size(surface)
        guard size.cell_height_px > 0, let window else { return 0 }
        return CGFloat(size.cell_height_px) / window.backingScaleFactor
    }

    private func scrollToRow(_ row: Int) {
        guard let surface else { return }
        let action = "scroll_to_row:\(row)"
        ghostty_surface_binding_action(surface, action, UInt(action.utf8.count))
    }

    private func buildKeyEvent(from event: NSEvent, action: ghostty_input_action_e) -> ghostty_input_key_s {
        var keyEvent = ghostty_input_key_s()
        keyEvent.action = action

        let normalized = event.type == .keyDown || event.type == .keyUp
            ? KeyCombo.normalized(key: event.charactersIgnoringModifiers ?? "", keyCode: event.keyCode)
            : KeyCombo.normalized(key: "", keyCode: event.keyCode)
        if let mappedCode = KeyCombo.keyCode(for: normalized) {
            keyEvent.keycode = UInt32(mappedCode)
        } else {
            keyEvent.keycode = UInt32(event.keyCode)
        }

        keyEvent.mods = modsFromEvent(event)
        keyEvent.consumed_mods = GHOSTTY_MODS_NONE
        keyEvent.composing = false
        keyEvent.text = nil
        keyEvent.unshifted_codepoint = unshiftedCodepoint(from: event)
        return keyEvent
    }

    private func consumedModsFromFlags(
        _ flags: NSEvent.ModifierFlags,
        consumeOption: Bool = true
    ) -> ghostty_input_mods_e {
        var mods = GHOSTTY_MODS_NONE.rawValue
        if flags.contains(.shift) {
            mods |= GHOSTTY_MODS_SHIFT.rawValue
        }
        if consumeOption, flags.contains(.option) {
            mods |= GHOSTTY_MODS_ALT.rawValue
        }
        return ghostty_input_mods_e(rawValue: mods)
    }

    private enum RightModifierMask {
        static let shift: UInt = 0x04
        static let control: UInt = 0x2000
        static let option: UInt = 0x40
        static let command: UInt = 0x10
    }

    private func modsFromEvent(_ event: NSEvent) -> ghostty_input_mods_e {
        var mods = GHOSTTY_MODS_NONE.rawValue
        let flags = event.modifierFlags
        let raw = flags.rawValue
        if flags.contains(.shift) {
            mods |= GHOSTTY_MODS_SHIFT.rawValue
        }
        if flags.contains(.control) {
            mods |= GHOSTTY_MODS_CTRL.rawValue
        }
        if flags.contains(.option) {
            mods |= GHOSTTY_MODS_ALT.rawValue
        }
        if flags.contains(.command) {
            mods |= GHOSTTY_MODS_SUPER.rawValue
        }
        if flags.contains(.capsLock) {
            mods |= GHOSTTY_MODS_CAPS.rawValue
        }
        if raw & RightModifierMask.shift != 0 {
            mods |= GHOSTTY_MODS_SHIFT_RIGHT.rawValue
        }
        if raw & RightModifierMask.control != 0 {
            mods |= GHOSTTY_MODS_CTRL_RIGHT.rawValue
        }
        if raw & RightModifierMask.option != 0 {
            mods |= GHOSTTY_MODS_ALT_RIGHT.rawValue
        }
        if raw & RightModifierMask.command != 0 {
            mods |= GHOSTTY_MODS_SUPER_RIGHT.rawValue
        }
        return ghostty_input_mods_e(rawValue: mods)
    }

    private func translatedOptionAsAlt(for event: NSEvent) -> Bool {
        guard let surface else { return false }
        let flags = event.modifierFlags
        guard flags.contains(.option) else { return false }
        let original = modsFromEvent(event)
        let translated = ghostty_surface_key_translation_mods(surface, original)
        return translated.rawValue & GHOSTTY_MODS_ALT.rawValue == 0
    }

    private func eventStrippingOption(_ event: NSEvent) -> NSEvent {
        let stripped = event.modifierFlags.subtracting(.option)
        let synthetic = NSEvent.keyEvent(
            with: event.type,
            location: event.locationInWindow,
            modifierFlags: stripped,
            timestamp: event.timestamp,
            windowNumber: event.windowNumber,
            context: nil,
            characters: event.charactersIgnoringModifiers ?? "",
            charactersIgnoringModifiers: event.charactersIgnoringModifiers ?? "",
            isARepeat: event.isARepeat,
            keyCode: event.keyCode
        )
        return synthetic ?? event
    }

    private func isFlagPress(_ event: NSEvent) -> Bool {
        let flags = event.modifierFlags
        switch event.keyCode {
        case 56,
             60: return flags.contains(.shift)
        case 58,
             61: return flags.contains(.option)
        case 59,
             62: return flags.contains(.control)
        case 55,
             54: return flags.contains(.command)
        case 57: return flags.contains(.capsLock)
        default: return false
        }
    }

    private func syncPreedit(clearIfNeeded: Bool = true) {
        guard let surface else { return }

        if hasMarkedText(), !_markedText.isEmpty {
            let byteCount = _markedText.utf8.count
            _markedText.withCString { ptr in
                ghostty_surface_preedit(surface, ptr, UInt(byteCount))
            }
        } else if clearIfNeeded {
            ghostty_surface_preedit(surface, nil, 0)
        }
    }

    private func filterSpecialCharacters(_ text: String) -> String {
        guard let scalar = text.unicodeScalars.first else { return "" }
        let value = scalar.value
        if value < 0x20 || (0xF700 ... 0xF8FF).contains(value) {
            return ""
        }
        return text
    }

    private func shortcutText(from event: NSEvent) -> String {
        let normalized = KeyCombo.normalized(key: event.charactersIgnoringModifiers ?? "", keyCode: event.keyCode)
        if normalized.unicodeScalars.count == 1,
           let scalar = normalized.unicodeScalars.first,
           scalar.isASCII, scalar.value >= 32, scalar.value <= 126
        {
            return normalized
        }
        if let scalar = KeyCombo.scalar(for: event.keyCode) {
            return String(scalar)
        }
        return event.charactersIgnoringModifiers ?? event.characters ?? ""
    }

    private func unshiftedCodepoint(from event: NSEvent) -> UInt32 {
        guard event.type == .keyDown || event.type == .keyUp else { return 0 }
        let normalized = KeyCombo.normalized(key: event.charactersIgnoringModifiers ?? "", keyCode: event.keyCode)
        if let scalar = normalized.unicodeScalars.first, normalized.unicodeScalars.count == 1 {
            return scalar.value
        }
        if let scalar = KeyCombo.scalar(for: event.keyCode) {
            return scalar.value
        }
        guard let chars = event.characters(byApplyingModifiers: []),
              let scalar = chars.unicodeScalars.first
        else { return 0 }
        return scalar.value
    }

    func sendSearchQuery(_ needle: String) {
        guard let surface else { return }
        let action = "search:\(needle)"
        ghostty_surface_binding_action(surface, action, UInt(action.utf8.count))
    }

    func navigateSearch(direction: TerminalSearchDirection) {
        guard let surface else { return }
        let action = "navigate_search:\(direction.rawValue)"
        ghostty_surface_binding_action(surface, action, UInt(action.utf8.count))
    }

    func endSearch() {
        guard let surface else { return }
        let action = "end_search"
        ghostty_surface_binding_action(surface, action, UInt(action.utf8.count))
    }

    func startSearch() {
        guard let surface else { return }
        let action = "start_search"
        ghostty_surface_binding_action(surface, action, UInt(action.utf8.count))
    }

    func sendText(_ text: String) {
        performOrDefer { $0.sendTextImmediately(text) }
    }

    private func sendTextImmediately(_ text: String) {
        guard let surface else { return }
        text.withCString { ptr in
            recordTextInput(text)
            ghostty_surface_text(surface, ptr, UInt(text.utf8.count))
        }
    }

    func sendReturnKey() {
        sendKeyPress(codepoint: 13, keycode: 36)
    }

    func sendRemoteBytes(_ bytes: Data) {
        performOrDefer { $0.sendRemoteBytesImmediately(bytes) }
    }

    private func sendRemoteBytesImmediately(_ bytes: Data) {
        guard let surface, !bytes.isEmpty else { return }
        bytes.withUnsafeBytes { raw in
            guard let base = raw.bindMemory(to: UInt8.self).baseAddress else { return }
            ghostty_surface_send_input_raw(surface, base, UInt(bytes.count))
        }
    }

    func setRawOutputHandler(_ handler: ((Data) -> Void)?) {
        if let surface {
            ghostty_surface_set_data_callback(surface, nil, nil)
        }
        if let rawOutputToken {
            GhosttyRawOutputHandlers.shared.remove(rawOutputToken)
            GhosttyShellActivitySessions.shared.remove(rawOutputToken)?.invalidate()
            self.rawOutputToken = nil
        }
        guard let surface, let handler else { return }
        let token = GhosttyRawOutputHandlers.shared.insert(handler)
        GhosttyShellActivitySessions.shared.insert(shellActivityTracker.beginSession(), token: token)
        rawOutputToken = token
        ghostty_surface_set_data_callback(
            surface,
            ghosttyRawOutputCallback,
            UnsafeMutableRawPointer(bitPattern: UInt(token))
        )
    }

    func scrollTerminal(deltaX: Double, deltaY: Double, precise: Bool) {
        guard ensureLiveSurfaceForExternalIO(), let surface else { return }
        let mods: ghostty_input_scroll_mods_t = precise ? 1 : 0
        ghostty_surface_mouse_scroll(surface, deltaX, deltaY, mods)
    }

    func resizeTerminal(cols: UInt32, rows: UInt32) -> Bool {
        guard ensureLiveSurfaceForExternalIO(), let surface else { return false }
        let size = ghostty_surface_size(surface)
        guard let pixelSize = TerminalSurfaceSizing.pixelSize(
            cols: cols,
            rows: rows,
            cellWidth: size.cell_width_px,
            cellHeight: size.cell_height_px
        )
        else { return false }
        ghostty_surface_set_size(surface, pixelSize.width, pixelSize.height)
        return true
    }

    func terminalCells(paneID: UUID) -> TerminalCellsDTO? {
        guard ensureLiveSurfaceForExternalIO(), let surface else { return nil }
        var output = ghostty_cells_s()
        guard ghostty_surface_read_cells(surface, &output) else { return nil }
        defer { ghostty_surface_free_cells(surface, &output) }

        let total = Int(output.cells_len)
        var cells: [TerminalCellDTO] = []
        cells.reserveCapacity(total)
        if let pointer = output.cells {
            for index in 0 ..< total {
                let cell = pointer[index]
                cells.append(TerminalCellDTO(
                    codepoint: cell.codepoint,
                    fg: cell.fg_rgb,
                    bg: cell.bg_rgb,
                    flags: cell.flags
                ))
            }
        }

        return TerminalCellsDTO(
            paneID: paneID,
            cols: output.cols,
            rows: output.rows,
            cursorX: output.cursor_x,
            cursorY: output.cursor_y,
            cursorVisible: output.cursor_visible,
            defaultFg: output.default_fg,
            defaultBg: output.default_bg,
            cells: cells,
            altScreen: output.alt_screen,
            cursorKeys: output.cursor_keys,
            bracketedPaste: output.bracketed_paste,
            focusEvent: output.focus_event,
            mouseEvent: output.mouse_event,
            mouseFormat: output.mouse_format
        )
    }

    func ensureLiveSurfaceForExternalIO() -> Bool {
        guard surface == nil else { return true }
        materializeHeadless()
        return surface != nil
    }

    func readScreenText(lastLines: Int = 50) -> String {
        guard let surface else { return "" }
        var out = ghostty_cells_s()
        guard ghostty_surface_read_cells(surface, &out) else { return "" }
        defer { ghostty_surface_free_cells(surface, &out) }

        let cols = Int(out.cols)
        let rows = Int(out.rows)
        guard cols > 0, rows > 0, let cells = out.cells else { return "" }

        var lines: [String] = []
        for row in 0 ..< rows {
            var line = ""
            for col in 0 ..< cols {
                let cell = cells[row * cols + col]
                let cp = cell.codepoint
                if cp == 0 {
                    line.append(" ")
                } else if let scalar = Unicode.Scalar(cp) {
                    line.append(Character(scalar))
                } else {
                    line.append(" ")
                }
            }
            lines.append(line)
        }

        while lines.last?.allSatisfy({ $0 == " " }) == true {
            lines.removeLast()
        }

        let trimmed = lines.map { $0.replacingOccurrences(of: "\\s+$", with: "", options: .regularExpression) }
        let result = trimmed.suffix(lastLines)
        return result.joined(separator: "\n")
    }

    func submitRichInput(text: String) {
        guard !text.isEmpty else { return }
        let sanitized = text.replacingOccurrences(of: "\u{1B}[201~", with: "")
        recordTextInput(sanitized)
        sendRemoteBytes(
            TerminalControlBytes.bracketedPasteStart
                + Data(sanitized.utf8)
                + TerminalControlBytes.bracketedPasteEnd
        )
    }

    func clearTerminalInput(lineBreakCount: Int) {
        if let paneID = TerminalViewRegistry.shared.paneID(for: self) {
            TerminalCommandTracker.shared.clearBuffer(paneID: paneID)
        }
        sendRemoteBytes(TerminalControlBytes.killInput(lineBreakCount: lineBreakCount))
    }

    func enqueueInputTransaction(
        _ operation: @escaping @MainActor () async -> Bool
    ) -> TerminalInputTransactionHandle {
        terminalInputQueue.enqueueTransaction(operation)
    }

    var uploadDestination: SSHDestination? {
        if let destination = workspaceContext.sshDestination {
            return destination
        }
        return detectedSSHDestination()
    }

    private func detectedSSHDestination() -> SSHDestination? {
        guard let processID = foregroundProcessID, processID > 0 else { return nil }
        guard let invocation = ProcessArgumentsInspector.invocation(pid: UInt64(processID)) else { return nil }
        return SSHInvocationParser.destination(from: invocation)
    }

    func beginUpload() -> TerminalUploadAttempt? {
        guard surface != nil else { return nil }
        guard let destination = uploadDestination else {
            return .local(surfaceGeneration: surfaceGeneration)
        }
        return .remote(remoteUploadSession.begin(
            surfaceGeneration: surfaceGeneration,
            destination: destination
        ))
    }

    func beginUpload(matching attempt: TerminalUploadAttempt) -> TerminalUploadAttempt? {
        guard uploadAttemptPermitsSideEffects(attempt) else { return nil }
        guard case let .remote(remoteAttempt) = attempt else { return attempt }
        return .remote(remoteUploadSession.begin(
            surfaceGeneration: remoteAttempt.surfaceGeneration,
            destination: remoteAttempt.destination
        ))
    }

    func pasteImageData(_ pngData: Data, attempt: TerminalUploadAttempt) async -> Bool {
        guard uploadAttemptPermitsSideEffects(attempt) else { return false }
        switch attempt {
        case let .remote(remoteAttempt):
            return await pasteRemoteImage(
                pngData: pngData,
                destination: remoteAttempt.destination,
                attempt: remoteAttempt
            )
        case .local:
            return await pasteLocalImage(pngData: pngData, attempt: attempt)
        }
    }

    private func insertPaths(for urls: [URL]) {
        guard uploadDestination != nil else {
            let text = urls.map { ShellEscaper.escape($0.path) }.joined(separator: " ")
            insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
            return
        }
        terminalInputQueue.enqueue { [weak self] in
            guard let self else { return }
            guard !Task.isCancelled, let attempt = beginUpload() else { return }
            guard let escapedPaths = await submissionPaths(forFilesAt: urls, attempt: attempt) else { return }
            guard !escapedPaths.isEmpty else { return }
            submitRichInput(text: escapedPaths.joined(separator: " "))
        }
    }

    func remotePath(forFileAt url: URL, attempt: TerminalUploadAttempt) async -> String? {
        guard case let .remote(remoteAttempt) = attempt else { return nil }
        guard uploadAttemptPermitsSideEffects(attempt) else { return nil }
        do {
            let data = try await Task.detached(priority: .userInitiated) {
                try RegularFileReader.data(
                    contentsOf: url,
                    maximumByteCount: RemoteUploadService.maximumByteCount
                )
            }.value
            guard uploadAttemptPermitsSideEffects(attempt) else { return nil }
            let remotePath = try await RemoteUploadService.upload(
                data: data,
                fileExtension: RemoteUploadService.sanitizedExtension(for: url),
                destination: remoteAttempt.destination,
                sessionID: remoteAttempt.sessionID,
                uploadID: remoteAttempt.uploadID
            )
            guard uploadAttemptPermitsSideEffects(attempt) else {
                guard !Task.isCancelled else { return nil }
                ToastState.shared.show(
                    title: L10n.string("Could not upload attachment to remote device"),
                    body: "\(url.lastPathComponent): \(L10n.string("The SSH destination changed during upload."))"
                )
                return nil
            }
            return remotePath
        } catch {
            guard !Task.isCancelled else { return nil }
            ToastState.shared.show(
                title: L10n.string("Could not upload attachment to remote device"),
                body: "\(url.lastPathComponent): \(error.localizedDescription)"
            )
            return nil
        }
    }

    private func pasteLocalImage(pngData: Data, attempt: TerminalUploadAttempt) async -> Bool {
        guard uploadAttemptPermitsSideEffects(attempt) else { return false }
        let savedClipboard = SystemPasteboardSnapshot.capture()
        defer { SystemPasteboardSnapshot.restore(items: savedClipboard) }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setData(pngData, forType: .png)
        sendRemoteBytes(TerminalControlBytes.pasteShortcut)
        try? await Task.sleep(for: Self.imagePasteDelay)
        return uploadAttemptPermitsSideEffects(attempt)
    }

    private func pasteRemoteImage(
        pngData: Data,
        destination: SSHDestination,
        attempt: RemoteUploadAttempt
    ) async -> Bool {
        guard uploadAttemptPermitsSideEffects(.remote(attempt)) else { return false }
        do {
            let remotePath = try await RemoteUploadService.upload(
                data: pngData,
                fileExtension: "png",
                destination: destination,
                sessionID: attempt.sessionID,
                uploadID: attempt.uploadID
            )
            guard uploadAttemptPermitsSideEffects(.remote(attempt)) else { return false }
            submitRichInput(text: ShellEscaper.escape(remotePath))
            return true
        } catch {
            guard !Task.isCancelled else { return false }
            ToastState.shared.show(
                title: L10n.string("Could not paste image on remote device"),
                body: error.localizedDescription
            )
            return false
        }
    }

    func uploadAttemptPermitsSideEffects(_ attempt: TerminalUploadAttempt) -> Bool {
        switch attempt {
        case let .local(surfaceGeneration):
            guard attempt.matches(destination: uploadDestination) else { return false }
            return !Task.isCancelled && surface != nil && self.surfaceGeneration == surfaceGeneration
        case let .remote(remoteAttempt):
            return remoteUploadSession.permitsSideEffects(
                for: remoteAttempt,
                currentDestination: uploadDestination,
                surfaceGeneration: surfaceGeneration,
                hasLiveSurface: surface != nil,
                isCancelled: Task.isCancelled
            )
        }
    }

    func scheduleTerminationCleanup() {
        surfaceGeneration += 1
        let cancelledWorker = terminalInputQueue.cancelAll()
        scheduleRemoteUploadCleanup(after: cancelledWorker)
    }

    private func scheduleRemoteUploadCleanup(after cancelledWorker: Task<Void, Never>?) {
        guard let active = remoteUploadSession.takeActiveSessionForCleanup() else { return }
        let precedingTasks = [precedingRemoteUploadCleanup, cancelledWorker].compactMap(\.self)
        precedingRemoteUploadCleanup = TerminalCleanupCoordinator.shared.schedule(after: precedingTasks) {
            for destination in active.destinations {
                await RemoteUploadService.remove(
                    sessionID: active.sessionID,
                    destination: destination
                )
            }
        }
    }

    func sendKeyPress(codepoint: UInt32, keycode: UInt32 = 0, mods: ghostty_input_mods_e = GHOSTTY_MODS_NONE) {
        performOrDefer { $0.sendKeyPressImmediately(codepoint: codepoint, keycode: keycode, mods: mods) }
    }

    private func sendKeyPressImmediately(
        codepoint: UInt32,
        keycode: UInt32,
        mods: ghostty_input_mods_e
    ) {
        guard let surface else { return }
        if codepoint == Codepoint.carriageReturn {
            recordReturnInput()
        } else if codepoint == Codepoint.delete || keycode == UInt32(KeyCode.backspace) {
            recordBackspaceInput()
        }
        var press = ghostty_input_key_s()
        press.action = GHOSTTY_ACTION_PRESS
        press.keycode = keycode
        press.mods = mods
        press.consumed_mods = mods
        press.composing = false
        press.text = nil
        press.unshifted_codepoint = codepoint
        _ = ghostty_surface_key(surface, press)

        var release = press
        release.action = GHOSTTY_ACTION_RELEASE
        _ = ghostty_surface_key(surface, release)
    }

    var hasLiveSurface: Bool {
        surface != nil
    }

    private enum Codepoint {
        static let carriageReturn: UInt32 = 13
        static let delete: UInt32 = 127
    }

    private enum KeyCode {
        static let `return`: UInt16 = 36
        static let backspace: UInt16 = 51
    }

    private func recordTextInput(_ text: String) {
        guard let paneID = TerminalViewRegistry.shared.paneID(for: self) else { return }
        guard canRecordTerminalInput() else { return }
        TerminalCommandTracker.shared.recordText(text, paneID: paneID)
    }

    private func recordReturnInput() {
        guard let paneID = TerminalViewRegistry.shared.paneID(for: self) else { return }
        guard canRecordTerminalInput() else { return }
        TerminalCommandTracker.shared.recordReturn(paneID: paneID)
    }

    private func recordBackspaceInput() {
        guard let paneID = TerminalViewRegistry.shared.paneID(for: self) else { return }
        guard canRecordTerminalInput() else { return }
        TerminalCommandTracker.shared.recordBackspace(paneID: paneID)
    }

    private func canRecordTerminalInput() -> Bool {
        let now = Date()
        if let cache = inputTrackingDecisionCache, cache.expiresAt > now {
            return cache.canRecord
        }
        let canRecord = currentInputTrackingDecision()
        inputTrackingDecisionCache = (now.addingTimeInterval(0.15), canRecord)
        return canRecord
    }

    private func currentInputTrackingDecision() -> Bool {
        guard let surface else { return false }
        let foregroundPID = ghostty_surface_foreground_pid(surface)
        let context = TerminalCommandTrackingInputContext(
            altScreen: isAlternateScreenActive(surface: surface),
            foregroundProcessName: Self.processName(pid: foregroundPID)
        )
        return TerminalCommandTrackingInputGate.shouldRecordInput(context)
    }

    private func isAlternateScreenActive(surface: ghostty_surface_t) -> Bool {
        var cells = ghostty_cells_s()
        guard ghostty_surface_read_cells(surface, &cells) else { return false }
        defer { ghostty_surface_free_cells(surface, &cells) }
        return cells.alt_screen
    }

    private static func processName(pid: UInt64) -> String? {
        guard pid > 0, pid <= UInt64(Int32.max) else { return nil }
        var buffer = [CChar](repeating: 0, count: 1024)
        let length = proc_name(Int32(pid), &buffer, UInt32(buffer.count))
        guard length > 0 else { return nil }
        let bytes = buffer.prefix(Int(length)).map { UInt8(bitPattern: $0) }
        return String(bytes: bytes, encoding: .utf8)
    }

    private func startAgentDetection() {
        if agentExecutables.isEmpty {
            agentExecutables = DetectedAgentStore.executablesSnapshot(from: AIProviderRegistry.shared)
        }
        agentProcessWatcher.onExit = { [weak self] in
            self?.handleAgentProcessExit()
        }
        detectAgentNow()
    }

    private func stopAgentDetection() {
        agentDetectionCoalesced?.cancel()
        agentDetectionCoalesced = nil
        agentProcessWatcher.cancel()
        guard detectedAgentProviderID != nil else { return }
        detectedAgentProviderID = nil
        onDetectedAgentChange?(nil)
    }

    func requestAgentDetection() {
        guard surface != nil, agentDetectionCoalesced == nil else { return }
        let work = DispatchWorkItem { [weak self] in
            self?.agentDetectionCoalesced = nil
            self?.detectAgentNow()
        }
        agentDetectionCoalesced = work
        DispatchQueue.main.async(execute: work)
    }

    private func detectAgentNow() {
        guard let surface else { return }
        let foregroundPID = persistentSessionID == nil
            ? ghostty_surface_foreground_pid(surface)
            : UInt64(max(foregroundProcessID ?? 0, 0))
        let candidateNames = ForegroundProcessInspector.executableNameCandidates(pid: foregroundPID)
        let providerID = AIAgentDetector.providerID(forCandidateNames: candidateNames, executables: agentExecutables)
        watchAgentProcess(providerID: providerID, processID: foregroundPID)
        guard providerID != detectedAgentProviderID else { return }
        detectedAgentProviderID = providerID
        onDetectedAgentChange?(providerID)
    }

    private func watchAgentProcess(providerID: String?, processID: UInt64) {
        guard providerID != nil, processID > 0, processID <= UInt64(Int32.max) else { return }
        agentProcessWatcher.watch(processID: Int32(processID))
    }

    private func handleAgentProcessExit() {
        if detectedAgentProviderID != nil {
            detectedAgentProviderID = nil
            onDetectedAgentChange?(nil)
        }
        onAgentProcessExit?()
        detectAgentNow()
    }

    private func recordSpecialKey(_ event: NSEvent) {
        switch event.keyCode {
        case KeyCode.return:
            recordReturnInput()
        case KeyCode.backspace:
            recordBackspaceInput()
        default:
            return
        }
    }
}

extension GhosttyTerminalNSView {
    override func draggingEntered(_ sender: any NSDraggingInfo) -> NSDragOperation {
        guard !droppedPaths(from: sender).isEmpty else { return [] }
        onExternalDragHoverChange?(true)
        return .copy
    }

    override func draggingUpdated(_ sender: any NSDraggingInfo) -> NSDragOperation {
        droppedPaths(from: sender).isEmpty ? [] : .copy
    }

    override func draggingExited(_: (any NSDraggingInfo)?) {
        onExternalDragHoverChange?(false)
    }

    override func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
        onExternalDragHoverChange?(false)
        let paths = droppedPaths(from: sender)
        guard !paths.isEmpty else { return false }
        scheduleFocusAndInsertAfterDrop(urls: paths.map { URL(fileURLWithPath: $0) })
        return true
    }

    private func scheduleFocusAndInsertAfterDrop(urls: [URL]) {
        RunLoop.main.perform(inModes: [.default]) { [weak self] in
            MainActor.assumeIsolated {
                guard let self else { return }
                NSApp.activate()
                self.window?.makeKeyAndOrderFront(nil)
                self.window?.makeFirstResponder(self)
                self.insertPaths(for: urls)
            }
        }
    }

    private func droppedPaths(from sender: any NSDraggingInfo) -> [String] {
        let pasteboard = sender.draggingPasteboard
        let urls = (pasteboard.readObjects(forClasses: [NSURL.self]) as? [URL]) ?? []
        return DroppedPathsParser.parse(fileURLs: urls, plainString: pasteboard.string(forType: .string))
    }
}

extension GhosttyTerminalNSView: @preconcurrency NSTextInputClient {
    func insertText(_ string: Any, replacementRange: NSRange) {
        let text = (string as? String) ?? (string as? NSAttributedString)?.string ?? ""

        unmarkText()

        guard !text.isEmpty else { return }
        guard currentKeyEvent == nil else {
            keyTextAccumulator.append(text)
            return
        }
        performOrDefer { $0.insertTextImmediately(text) }
    }

    private func insertTextImmediately(_ text: String) {
        guard let surface else { return }
        text.withCString { ptr in
            var keyEvent = ghostty_input_key_s()
            keyEvent.action = GHOSTTY_ACTION_PRESS
            keyEvent.keycode = 0
            keyEvent.mods = GHOSTTY_MODS_NONE
            keyEvent.consumed_mods = GHOSTTY_MODS_NONE
            keyEvent.composing = false
            keyEvent.text = ptr
            recordTextInput(text)
            _ = ghostty_surface_key(surface, keyEvent)
        }
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        let text = (string as? String) ?? (string as? NSAttributedString)?.string ?? ""
        _markedText = text
        _markedRange = text.isEmpty ? NSRange(location: NSNotFound, length: 0) : NSRange(location: 0, length: text.utf16.count)
        _selectedRange = clampedMarkedRange(selectedRange)

        if currentKeyEvent == nil {
            syncPreedit()
        }
    }

    func unmarkText() {
        guard hasMarkedText() else { return }
        _markedText = ""
        _markedRange = NSRange(location: NSNotFound, length: 0)
        _selectedRange = NSRange(location: 0, length: 0)
        syncPreedit()
    }

    func selectedRange() -> NSRange {
        _selectedRange
    }

    func markedRange() -> NSRange {
        _markedRange
    }

    func hasMarkedText() -> Bool {
        _markedRange.location != NSNotFound
    }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        guard hasMarkedText() else {
            actualRange?.pointee = NSRange(location: 0, length: 0)
            return range.location == 0 && range.length == 0 ? NSAttributedString(string: "") : nil
        }
        guard let safeRange = intersection(range, with: _markedRange) else { return nil }
        actualRange?.pointee = safeRange
        return NSAttributedString(string: (_markedText as NSString).substring(with: safeRange))
    }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] {
        []
    }

    func characterIndex(for point: NSPoint) -> Int {
        NSNotFound
    }

    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        guard let surface else { return .zero }
        var x: Double = 0, y: Double = 0, w: Double = 0, h: Double = 0
        ghostty_surface_ime_point(surface, &x, &y, &w, &h)
        let viewPt = NSPoint(x: x, y: bounds.height - y)
        let screenPt = window?.convertPoint(toScreen: convert(viewPt, to: nil)) ?? viewPt
        return NSRect(x: screenPt.x, y: screenPt.y - h, width: w, height: h)
    }

    private func clampedMarkedRange(_ range: NSRange) -> NSRange {
        guard range.location != NSNotFound else { return NSRange(location: 0, length: 0) }
        let length = _markedText.utf16.count
        let location = min(range.location, length)
        return NSRange(location: location, length: min(range.length, length - location))
    }

    private func intersection(_ range: NSRange, with otherRange: NSRange) -> NSRange? {
        guard range.location != NSNotFound, otherRange.location != NSNotFound else { return nil }
        let start = max(range.location, otherRange.location)
        let end = min(range.location + range.length, otherRange.location + otherRange.length)
        guard start <= end else { return nil }
        return NSRange(location: start, length: end - start)
    }
}

@MainActor
private final class GhosttyRawOutputHandlers {
    static let shared = GhosttyRawOutputHandlers()

    private var handlers: [Int: (Data) -> Void] = [:]
    private var nextToken = 1

    func insert(_ handler: @escaping (Data) -> Void) -> Int {
        let token = nextToken
        nextToken += 1
        handlers[token] = handler
        return token
    }

    func remove(_ token: Int) {
        handlers.removeValue(forKey: token)
    }

    func deliver(_ data: Data, token: Int) {
        handlers[token]?(data)
    }
}

private final class GhosttyShellActivitySessions: @unchecked Sendable {
    static let shared = GhosttyShellActivitySessions()

    private let lock = NSLock()
    private var sessions: [Int: TerminalShellActivityTracker.Session] = [:]

    private init() {}

    func insert(_ session: TerminalShellActivityTracker.Session, token: Int) {
        lock.withLock {
            sessions[token] = session
        }
    }

    func session(token: Int) -> TerminalShellActivityTracker.Session? {
        lock.withLock { sessions[token] }
    }

    func remove(_ token: Int) -> TerminalShellActivityTracker.Session? {
        lock.withLock {
            sessions.removeValue(forKey: token)
        }
    }
}

private let ghosttyRawOutputCallback: @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafePointer<UInt8>?,
    UInt
) -> Void = { userdata, pointer, length in
    guard let userdata, let pointer, length > 0 else { return }
    let token = Int(bitPattern: userdata)
    let data = Data(bytes: pointer, count: Int(length))
    GhosttyShellActivitySessions.shared.session(token: token)?.recordOutput(data)
    DispatchQueue.main.async {
        MainActor.assumeIsolated {
            GhosttyRawOutputHandlers.shared.deliver(data, token: token)
        }
    }
}
