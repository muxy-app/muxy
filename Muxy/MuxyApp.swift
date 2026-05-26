import AppKit
import SwiftUI

@main
struct MuxyApp: App {
    nonisolated static let launchDate = Date()

    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @State private var appState: AppState
    @State private var projectStore: ProjectStore
    @State private var worktreeStore: WorktreeStore
    @State private var projectGroupStore: ProjectGroupStore
    @State private var vcsWorktreeAutoRefresher: VCSWorktreeAutoRefresher
    @State private var didStartDeferredServices = false

    init() {
        _ = MuxyApp.launchDate
        let environment = AppEnvironment.live
        let projectStore = ProjectStore(persistence: environment.projectPersistence)
        let worktreeStore = WorktreeStore(
            persistence: environment.worktreePersistence,
            projects: projectStore.projects
        )
        let appState = AppState(
            selectionStore: environment.selectionStore,
            terminalViews: environment.terminalViews,
            workspacePersistence: environment.workspacePersistence
        )
        appState.restoreSelection(
            projects: projectStore.projects,
            worktrees: worktreeStore.worktrees
        )
        let projectGroupStore = ProjectGroupStore(
            persistence: environment.projectGroupPersistence
        )
        let vcsWorktreeAutoRefresher = VCSWorktreeAutoRefresher(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore
        )
        _appState = State(initialValue: appState)
        _projectStore = State(initialValue: projectStore)
        _worktreeStore = State(initialValue: worktreeStore)
        _projectGroupStore = State(initialValue: projectGroupStore)
        _vcsWorktreeAutoRefresher = State(initialValue: vcsWorktreeAutoRefresher)
    }

    var body: some Scene {
        WindowGroup {
            MainWindow()
                .environment(appState)
                .environment(projectStore)
                .environment(worktreeStore)
                .environment(projectGroupStore)
                .environment(GhosttyService.shared)
                .environment(MuxyConfig.shared)
                .environment(ThemeService.shared)
                .preferredColorScheme(MuxyTheme.colorScheme)
                .onAppear {
                    startDeferredServicesIfNeeded()
                    NotificationStore.shared.appState = appState
                    NotificationStore.shared.worktreeStore = worktreeStore
                    NotificationStore.shared.markAllAsRead()
                    MemoryDiagnostics.shared.configure(appState: appState)
                    TerminalProgressStore.shared.appState = appState
                    appDelegate.onTerminate = { [appState] in
                        appState.saveTerminalSessions()
                        appState.saveWorkspaces()
                    }
                    appDelegate.hasUnsavedEditorTabs = { [appState] in
                        appState.unsavedEditorTabs()
                    }
                    appDelegate.openProjectFromPath = { [appState, projectStore, worktreeStore] path in
                        CLIAccessor.openProjectFromPath(
                            path,
                            appState: appState,
                            projectStore: projectStore,
                            worktreeStore: worktreeStore
                        )
                    }
                    appDelegate.flushPendingOpens()
                    NotificationSocketServer.shared.commandHandler = { [appState, projectStore, worktreeStore] message, context in
                        await SocketCommandHandler.handleRequest(
                            message,
                            appState: appState,
                            projectStore: projectStore,
                            worktreeStore: worktreeStore,
                            clientContext: context
                        )
                    }
                    MobileServerService.shared.configure { server in
                        let delegate = RemoteServerDelegate(
                            appState: appState,
                            projectStore: projectStore,
                            worktreeStore: worktreeStore
                        )
                        delegate.server = server
                        return delegate
                    }
                    appState.onProjectsEmptied = { [projectStore, worktreeStore] projectIDs in
                        for id in projectIDs {
                            if let project = projectStore.projects.first(where: { $0.id == id }) {
                                let knownWorktrees = worktreeStore.list(for: id)
                                Task.detached {
                                    await WorktreeStore.cleanupOnDisk(
                                        for: project,
                                        knownWorktrees: knownWorktrees
                                    )
                                }
                            }
                            projectStore.remove(id: id)
                            worktreeStore.removeProject(id)
                        }
                    }
                    projectStore.onProjectRemoved = { [projectGroupStore] projectID in
                        projectGroupStore.removeProjectFromAllGroups(projectID: projectID)
                    }
                }
        }
        .windowStyle(HiddenTitleBarWindowStyle())
        .defaultSize(width: 1200, height: 800)
        .commands {
            MuxyCommands(
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore,
                keyBindings: .shared,
                commandShortcuts: .shared,
                config: .shared,
                ghostty: .shared,
                updateService: .shared
            )
        }

        Window("Source Control", id: "vcs") {
            VCSWindowView()
                .environment(appState)
                .environment(projectStore)
                .environment(worktreeStore)
                .environment(projectGroupStore)
                .environment(GhosttyService.shared)
                .preferredColorScheme(MuxyTheme.colorScheme)
        }
        .defaultSize(width: 700, height: 600)

        Window("Muxy Help", id: "help") {
            HelpView()
                .preferredColorScheme(MuxyTheme.colorScheme)
        }
        .defaultSize(width: 820, height: 580)
    }

    private func startDeferredServicesIfNeeded() {
        guard !didStartDeferredServices else { return }
        didStartDeferredServices = true
        Task { @MainActor in
            await Task.yield()
            SettingsJSONStore.beginAutomaticUserSettingsSync()
            try? await Task.sleep(for: .seconds(2))
            UpdateService.shared.start()
            AIProviderRegistry.shared.installAll()
            ExtensionStore.shared.startAll()
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    var onTerminate: (() -> Void)?
    var hasUnsavedEditorTabs: (() -> [EditorTabState])?
    var openProjectFromPath: ((String) -> Void)?

    private var pendingOpenPaths: [String] = []
    private var systemAppearanceObserver: NSObjectProtocol?
    private var settingsObserver: NSObjectProtocol?
    private var settingsThemeObserver: NSObjectProtocol?
    private weak var settingsWindow: NSWindow?

    @MainActor
    func handleOpenProjectPath(_ path: String) {
        let standardized = URL(fileURLWithPath: path).standardizedFileURL.path
        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(atPath: standardized, isDirectory: &isDirectory),
              isDirectory.boolValue
        else { return }
        if let handler = openProjectFromPath {
            handler(standardized)
            return
        }
        pendingOpenPaths.append(standardized)
    }

    @MainActor
    func flushPendingOpens() {
        guard let handler = openProjectFromPath else { return }
        let queued = pendingOpenPaths
        pendingOpenPaths.removeAll()
        for path in queued {
            handler(path)
        }
    }

    nonisolated static func resolveProjectPath(from url: URL) -> String? {
        if url.isFileURL {
            let standardized = url.standardizedFileURL.path
            return standardized.isEmpty || standardized == "/" ? nil : standardized
        }
        guard url.scheme == "muxy" else { return nil }
        guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false) else {
            return nil
        }
        var raw: String?
        if let queryItems = components.queryItems,
           let pathItem = queryItems.first(where: { $0.name == "path" })?.value,
           !pathItem.isEmpty
        {
            raw = pathItem
        } else {
            var combined = ""
            if let host = components.host, !host.isEmpty {
                combined = host
            }
            if !components.path.isEmpty, components.path != "/" {
                combined += components.path
            }
            raw = combined.isEmpty ? nil : combined
        }
        guard var resolved = raw else { return nil }
        if let decoded = resolved.removingPercentEncoding {
            resolved = decoded
        }
        guard !resolved.isEmpty, resolved != "/" else { return nil }
        if !resolved.hasPrefix("/") {
            resolved = "/" + resolved
        }
        let standardized = URL(fileURLWithPath: resolved).standardizedFileURL.path
        guard !standardized.isEmpty, standardized != "/" else { return nil }
        return standardized
    }

    @MainActor
    func application(_ application: NSApplication, open urls: [URL]) {
        for url in urls {
            guard let path = Self.resolveProjectPath(from: url) else { continue }
            handleOpenProjectPath(path)
        }
    }

    func applicationShouldOpenUntitledFile(_ sender: NSApplication) -> Bool {
        false
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if let window = Self.mainAppWindow() {
            sender.activate(ignoringOtherApps: true)
            window.makeKeyAndOrderFront(nil)
        }
        return false
    }

    @MainActor
    func applicationDidFinishLaunching(_ notification: Notification) {
        SentryService.shared.start()
        NSWindow.allowsAutomaticWindowTabbing = false
        NSApp.setActivationPolicy(.regular)
        NSApp.activate()
        setAppIcon()
        _ = GhosttyService.shared
        GhosttyService.shared.applyInitialColorScheme()
        ThemeService.shared.applyDefaultThemeIfNeeded()
        ThemeService.shared.migrateToPairedThemeIfNeeded()
        observeSystemAppearanceChanges()
        ModifierKeyMonitor.shared.start()
        NotificationSocketServer.shared.openProjectHandler = { [weak self] path in
            Task { @MainActor [weak self] in
                self?.handleOpenProjectPath(path)
            }
        }
        NotificationSocketServer.shared.start()
        _ = AIUsageSettingsStore.isUsageEnabled()
        DiagnosticsMenuController.shared.install()
        observeSettingsRequests()
        consumeLaunchArguments()
    }

    @MainActor
    private func consumeLaunchArguments() {
        guard CommandLine.argc > 1 else { return }
        let candidate = CommandLine.arguments[1]
        guard candidate.hasPrefix("/") || candidate.hasPrefix("~") else { return }
        let expanded = (candidate as NSString).expandingTildeInPath
        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(atPath: expanded, isDirectory: &isDirectory),
              isDirectory.boolValue
        else { return }
        handleOpenProjectPath(expanded)
    }

    static func mainAppWindow(excluding excludedWindow: NSWindow? = nil) -> NSWindow? {
        NSApp.windows.first { window in
            window !== excludedWindow && window.identifier == ShortcutContext.mainWindowIdentifier
        }
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        let unsaved = hasUnsavedEditorTabs?() ?? []
        guard !unsaved.isEmpty else { return confirmQuitIfNeeded() }

        let alert = NSAlert()
        alert.messageText = unsaved.count == 1
            ? "You have unsaved changes in 1 file."
            : "You have unsaved changes in \(unsaved.count) files."
        alert.informativeText = "If you quit without saving, your changes will be lost."
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Save All")
        alert.addButton(withTitle: "Cancel")
        alert.addButton(withTitle: "Discard")
        alert.buttons[0].keyEquivalent = "\r"
        alert.buttons[1].keyEquivalent = "\u{1b}"

        let response = alert.runModal()
        switch response {
        case .alertFirstButtonReturn:
            Task { @MainActor in
                var failures: [String] = []
                for state in unsaved {
                    do {
                        try await state.saveFileAsync()
                    } catch {
                        failures.append("\(state.fileName): \(error.localizedDescription)")
                    }
                }
                if failures.isEmpty {
                    NSApp.reply(toApplicationShouldTerminate: true)
                    return
                }
                Self.presentSaveFailureAlert(failures: failures)
                NSApp.reply(toApplicationShouldTerminate: false)
            }
            return .terminateLater
        case .alertThirdButtonReturn:
            return .terminateNow
        default:
            return .terminateCancel
        }
    }

    @MainActor
    private func confirmQuitIfNeeded() -> NSApplication.TerminateReply {
        guard QuitConfirmationPreferences.confirmQuit else { return .terminateNow }

        let alert = NSAlert()
        alert.messageText = "Quit Muxy?"
        alert.informativeText = "Are you sure you want to quit Muxy?"
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Quit")
        alert.addButton(withTitle: "Cancel")
        alert.buttons[0].keyEquivalent = "\r"
        alert.buttons[1].keyEquivalent = "\u{1b}"
        alert.showsSuppressionButton = true
        alert.suppressionButton?.title = "Don't ask again"

        let response = alert.runModal()
        guard response == .alertFirstButtonReturn else { return .terminateCancel }
        if alert.suppressionButton?.state == .on {
            QuitConfirmationPreferences.confirmQuit = false
        }
        return .terminateNow
    }

    @MainActor
    private static func presentSaveFailureAlert(failures: [String]) {
        let alert = NSAlert()
        alert.messageText = failures.count == 1
            ? "Could Not Save File"
            : "Could Not Save \(failures.count) Files"
        alert.informativeText = failures.joined(separator: "\n")
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "OK")
        alert.buttons[0].keyEquivalent = "\r"
        alert.runModal()
    }

    func applicationWillTerminate(_ notification: Notification) {
        if let observer = systemAppearanceObserver {
            DistributedNotificationCenter.default().removeObserver(observer)
            systemAppearanceObserver = nil
        }
        if let settingsObserver {
            NotificationCenter.default.removeObserver(settingsObserver)
            self.settingsObserver = nil
        }
        if let settingsThemeObserver {
            NotificationCenter.default.removeObserver(settingsThemeObserver)
            self.settingsThemeObserver = nil
        }
        onTerminate?()
        NotificationStore.shared.saveToDisk()
        NotificationSocketServer.shared.stop()
        MainActor.assumeIsolated {
            MobileServerService.shared.stopForTermination()
            RichInputDraftStore.shared.flush()
            ExtensionStore.shared.stopAll()
        }
    }

    @MainActor
    private func observeSettingsRequests() {
        settingsObserver = NotificationCenter.default.addObserver(
            forName: .openSettingsModal,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.presentSettingsModal()
            }
        }
        settingsThemeObserver = NotificationCenter.default.addObserver(
            forName: .themeDidChange,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.settingsWindow?.backgroundColor = MuxyTheme.nsBg
            }
        }
    }

    @MainActor
    private func presentSettingsModal() {
        if let settingsWindow {
            settingsWindow.makeKeyAndOrderFront(nil)
            return
        }
        guard let parent = NSApp.keyWindow ?? NSApp.mainWindow else { return }
        let host = NSHostingController(
            rootView: SettingsView()
                .frame(width: 980, height: 680)
                .preferredColorScheme(MuxyTheme.colorScheme)
        )
        let window = SettingsModalWindow(contentViewController: host)
        window.title = "Settings"
        window.styleMask = [.titled, .closable]
        window.isOpaque = true
        window.backgroundColor = MuxyTheme.nsBg
        window.delegate = self
        settingsWindow = window
        parent.beginSheet(window) { [weak self, weak window] _ in
            guard self?.settingsWindow === window else { return }
            self?.settingsWindow = nil
        }
        window.startOutsideClickMonitor()
    }

    func windowWillClose(_ notification: Notification) {
        guard notification.object as? NSWindow === settingsWindow else { return }
        settingsWindow = nil
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        guard sender.identifier == ShortcutContext.mainWindowIdentifier else { return true }
        NSApp.terminate(nil)
        return false
    }

    @MainActor
    private func observeSystemAppearanceChanges() {
        if let observer = systemAppearanceObserver {
            DistributedNotificationCenter.default().removeObserver(observer)
            systemAppearanceObserver = nil
        }
        systemAppearanceObserver = DistributedNotificationCenter.default().addObserver(
            forName: Notification.Name("AppleInterfaceThemeChangedNotification"),
            object: nil,
            queue: .main
        ) { _ in
            Task { @MainActor in
                GhosttyService.shared.appearanceDidChange()
            }
        }
    }

    @MainActor
    private func setAppIcon() {
        guard let url = Bundle.appResources.url(forResource: "AppIcon", withExtension: "png") else {
            return
        }
        guard let image = NSImage(contentsOf: url) else { return }
        image.size = NSSize(width: 512, height: 512)
        NSApp.applicationIconImage = image
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}

private final class SettingsModalWindow: NSWindow {
    private var outsideClickMonitor: Any?

    func startOutsideClickMonitor() {
        stopOutsideClickMonitor()
        outsideClickMonitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] event in
            guard let self, let sheetParent, event.window === sheetParent else { return event }
            close()
            return nil
        }
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.modifierFlags.intersection(.deviceIndependentFlagsMask) == .command,
           event.charactersIgnoringModifiers?.lowercased() == "w"
        {
            close()
            return true
        }
        return super.performKeyEquivalent(with: event)
    }

    override func cancelOperation(_ sender: Any?) {
        close()
    }

    override func close() {
        stopOutsideClickMonitor()
        guard let sheetParent else {
            super.close()
            return
        }
        sheetParent.endSheet(self)
    }

    private func stopOutsideClickMonitor() {
        guard let outsideClickMonitor else { return }
        NSEvent.removeMonitor(outsideClickMonitor)
        self.outsideClickMonitor = nil
    }
}

struct WindowConfigurator: NSViewRepresentable {
    let configVersion: Int
    let uiScalePreset: UIScale.Preset

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> NSView {
        let v = NSView()
        DispatchQueue.main.async {
            guard let w = v.window else { return }
            w.identifier = ShortcutContext.mainWindowIdentifier
            if Self.closeDuplicateMainWindow(w) { return }
            w.titlebarAppearsTransparent = true
            w.titleVisibility = .hidden
            w.styleMask.insert(.fullSizeContentView)
            w.isMovable = false
            w.isMovableByWindowBackground = false
            Self.disableWindowTabbing(for: w)
            Self.applyWindowBackground(w)
            Self.repositionTrafficLights(in: w)
            Self.hideTitlebarDecorationView(in: w)
            Self.neutralizeSafeAreaInsets(in: w)
            Self.interceptCloseButton(in: w, coordinator: context.coordinator)
            context.coordinator.observe(window: w)
        }
        return v
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        guard let w = nsView.window else { return }
        Self.applyWindowBackground(w)
        Self.repositionTrafficLights(in: w)
    }

    private static func applyWindowBackground(_ window: NSWindow) {
        window.isOpaque = false
        window.backgroundColor = .clear
        window.contentView?.wantsLayer = true
        window.contentView?.layer?.backgroundColor = MuxyTheme.nsBg.cgColor
    }

    static func disableWindowTabbing(for window: NSWindow) {
        window.tabbingMode = .disallowed
    }

    static func closeDuplicateMainWindow(_ window: NSWindow) -> Bool {
        guard let existingWindow = AppDelegate.mainAppWindow(excluding: window) else { return false }
        existingWindow.makeKeyAndOrderFront(nil)
        window.close()
        return true
    }

    static func neutralizeSafeAreaInsets(in window: NSWindow) {
        if #available(macOS 26.0, *) {
            guard let contentView = window.contentView else { return }
            contentView.additionalSafeAreaInsets.top = 0
            let baseSafeAreaTop = contentView.safeAreaInsets.top
            contentView.additionalSafeAreaInsets.top = -baseSafeAreaTop
        }
    }

    static func hideTitlebarDecorationView(in window: NSWindow) {
        guard let themeFrame = window.contentView?.superview else { return }
        for view in themeFrame.subviews {
            let name = NSStringFromClass(type(of: view))
            guard name.contains("NSTitlebarContainerView") else { continue }

            view.wantsLayer = true
            view.layer?.backgroundColor = CGColor.clear
            view.layer?.isOpaque = false

            for child in view.subviews {
                let childName = NSStringFromClass(type(of: child))
                if childName.contains("NSTitlebarDecorationView") {
                    child.isHidden = true
                }
                if childName.contains("NSTitlebarView") {
                    child.wantsLayer = true
                    child.layer?.backgroundColor = CGColor.clear
                    child.layer?.isOpaque = false
                    for sub in child.subviews {
                        let subName = NSStringFromClass(type(of: sub))
                        if subName == "NSView" || subName.contains("Background") {
                            sub.isHidden = true
                        }
                    }
                }
            }
        }
    }

    static func interceptCloseButton(in window: NSWindow, coordinator: Coordinator) {
        guard let button = window.standardWindowButton(.closeButton) else { return }
        button.target = coordinator
        button.action = #selector(Coordinator.handleCloseButton(_:))
    }

    static let trafficLightY: CGFloat = 3.5
    static let baselineTitleBarHeight: CGFloat = 32

    static func desiredTrafficLightY() -> CGFloat {
        let scaledTitleBarHeight = UIMetrics.scaled(baselineTitleBarHeight)
        let extraVerticalSpace = scaledTitleBarHeight - baselineTitleBarHeight
        if #available(macOS 26.0, *) {
            let buttonHeight: CGFloat = 14
            return (baselineTitleBarHeight - buttonHeight - extraVerticalSpace) / 2
        }
        return trafficLightY - extraVerticalSpace / 2
    }

    static func repositionTrafficLights(in window: NSWindow) {
        let y = desiredTrafficLightY()
        for button in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
            guard let btn = window.standardWindowButton(button) else { continue }
            guard abs(btn.frame.origin.y - y) > 0.5 else { continue }
            var frame = btn.frame
            frame.origin.y = y
            btn.frame = frame
        }
    }

    final class Coordinator: NSObject {
        private var observations: [NSObjectProtocol] = []
        private var buttonFrameObservations: [NSObjectProtocol] = []

        @objc
        func handleCloseButton(_: Any?) {
            MainActor.assumeIsolated {
                NSApp.terminate(nil)
            }
        }

        func observe(window: NSWindow) {
            guard observations.isEmpty else { return }

            let names: [Notification.Name] = [
                NSWindow.didResizeNotification,
                NSWindow.didEndLiveResizeNotification,
                NSWindow.didChangeScreenNotification,
                NSWindow.didChangeBackingPropertiesNotification,
                NSWindow.didExitFullScreenNotification,
                NSWindow.didEnterFullScreenNotification,
                NSWindow.didUpdateNotification,
                NSWindow.didBecomeKeyNotification,
                NSWindow.didBecomeMainNotification,
            ]
            for name in names {
                let token = NotificationCenter.default.addObserver(
                    forName: name,
                    object: window,
                    queue: .main
                ) { notification in
                    guard let w = notification.object as? NSWindow else { return }
                    MainActor.assumeIsolated {
                        WindowConfigurator.repositionTrafficLights(in: w)
                        WindowConfigurator.hideTitlebarDecorationView(in: w)
                        if name == NSWindow.didChangeScreenNotification
                            || name == NSWindow.didChangeBackingPropertiesNotification
                        {
                            WindowConfigurator.neutralizeSafeAreaInsets(in: w)
                        }
                        if name == NSWindow.didEnterFullScreenNotification
                            || name == NSWindow.didExitFullScreenNotification
                        {
                            WindowConfigurator.neutralizeSafeAreaInsets(in: w)
                            let isFullScreen = w.styleMask.contains(.fullScreen)
                            NotificationCenter.default.post(
                                name: .windowFullScreenDidChange,
                                object: nil,
                                userInfo: ["isFullScreen": isFullScreen]
                            )
                        }
                    }
                }
                observations.append(token)
            }

            observeButtonFrames(window: window)
        }

        private func observeButtonFrames(window: NSWindow) {
            buttonFrameObservations.forEach { NotificationCenter.default.removeObserver($0) }
            buttonFrameObservations.removeAll()
            for type in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
                guard let button = MainActor.assumeIsolated({ window.standardWindowButton(type) }) else { continue }
                MainActor.assumeIsolated { button.postsFrameChangedNotifications = true }
                let token = NotificationCenter.default.addObserver(
                    forName: NSView.frameDidChangeNotification,
                    object: button,
                    queue: .main
                ) { [weak window] _ in
                    guard let window else { return }
                    DispatchQueue.main.async {
                        MainActor.assumeIsolated {
                            WindowConfigurator.repositionTrafficLights(in: window)
                        }
                    }
                }
                buttonFrameObservations.append(token)
            }
        }

        deinit {
            observations.forEach { NotificationCenter.default.removeObserver($0) }
            buttonFrameObservations.forEach { NotificationCenter.default.removeObserver($0) }
        }
    }
}
