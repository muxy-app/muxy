import AppKit
import os
import SwiftUI

private let deepLinkLogger = Logger(subsystem: "app.muxy", category: "DeepLink")

@main
struct MuxyApp: App {
    nonisolated static let launchDate = Date()

    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @State private var appState: AppState
    @State private var projectStore: ProjectStore
    @State private var worktreeStore: WorktreeStore
    @State private var projectGroupStore: ProjectGroupStore
    @State private var remoteDeviceStore: RemoteDeviceStore
    @State private var worktreeAutoRefresher: VCSWorktreeAutoRefresher?
    @State private var didStartDeferredServices = false

    init() {
        UserDefaults.standard.register(defaults: ["NSInitialToolTipDelay": 500])
        LaunchArgumentGuard.terminateIfNeeded()
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
        let remoteDeviceStore = RemoteDeviceStore(
            persistence: environment.remoteDevicePersistence
        )
        let projectGroupStore = ProjectGroupStore(
            persistence: environment.projectGroupPersistence,
            remoteDeviceStore: remoteDeviceStore
        )
        appState.restoreSelection(
            projects: projectStore.projects,
            worktrees: worktreeStore.worktrees,
            skippingProjectIDs: projectGroupStore.activeRemoteProjectIDs
        )
        _appState = State(initialValue: appState)
        _projectStore = State(initialValue: projectStore)
        _worktreeStore = State(initialValue: worktreeStore)
        _projectGroupStore = State(initialValue: projectGroupStore)
        _remoteDeviceStore = State(initialValue: remoteDeviceStore)
    }

    var body: some Scene {
        WindowGroup {
            MainWindow()
                .environment(appState)
                .environment(projectStore)
                .environment(worktreeStore)
                .environment(projectGroupStore)
                .environment(remoteDeviceStore)
                .environment(SSHConnectionService.shared)
                .environment(GhosttyService.shared)
                .environment(MuxyConfig.shared)
                .environment(ThemeService.shared)
                .environment(ExtensionStore.shared)
                .environment(ExtensionSettingsStore.shared)
                .preferredColorScheme(MuxyTheme.colorScheme)
                .onAppear {
                    startDeferredServicesIfNeeded()
                    startWorktreeAutoRefreshIfNeeded()
                    NotificationStore.shared.appState = appState
                    NotificationStore.shared.worktreeStore = worktreeStore
                    NotificationStore.shared.markAllAsRead()
                    DesktopNotificationService.shared.start(appState: appState)
                    MemoryDiagnostics.shared.configure(appState: appState)
                    TerminalProgressStore.shared.appState = appState
                    appDelegate.onTerminate = { [appState] in
                        appState.saveWorkspaces()
                    }
                    appDelegate.settingsContent = { [projectGroupStore, remoteDeviceStore] in
                        AnyView(
                            SettingsView()
                                .environment(projectGroupStore)
                                .environment(remoteDeviceStore)
                                .environment(SSHConnectionService.shared)
                        )
                    }
                    appDelegate.openProjectFromPath = { [appState, projectStore, worktreeStore, projectGroupStore] path in
                        CLIAccessor.openProjectFromPath(
                            path,
                            appState: appState,
                            projectStore: projectStore,
                            worktreeStore: worktreeStore,
                            projectGroupStore: projectGroupStore
                        )
                    }
                    appDelegate.flushPendingOpens()
                    NotificationSocketServer.shared.commandHandler = { [
                        appState,
                        projectStore,
                        worktreeStore,
                        projectGroupStore
                    ] message, context in
                        await SocketCommandHandler.handleRequest(
                            message,
                            appState: appState,
                            projectStore: projectStore,
                            worktreeStore: worktreeStore,
                            projectGroupStore: projectGroupStore,
                            clientContext: context
                        )
                    }
                    MobileServerService.shared.configure { server in
                        let delegate = RemoteServerDelegate(
                            appState: appState,
                            projectStore: projectStore,
                            worktreeStore: worktreeStore,
                            projectGroupStore: projectGroupStore
                        )
                        delegate.server = server
                        return delegate
                    }
                    appState.onProjectsEmptied = { [projectStore, worktreeStore] projectIDs in
                        for id in projectIDs where id != Project.homeID {
                            guard let project = projectStore.projects.first(where: { $0.id == id }) else {
                                worktreeStore.removeProject(id)
                                continue
                            }
                            let knownWorktrees = worktreeStore.list(for: id)
                            Task {
                                do {
                                    try await WorktreeStore.cleanupOnDisk(
                                        for: project,
                                        knownWorktrees: knownWorktrees
                                    )
                                    projectStore.remove(id: id)
                                    worktreeStore.removeProject(id)
                                } catch {
                                    ToastState.shared.show("Could not remove \(project.name): \(error.localizedDescription)")
                                }
                            }
                        }
                    }
                    projectStore.onProjectRemoved = { [projectGroupStore] projectID in
                        projectGroupStore.removeProjectFromAllGroups(projectID: projectID)
                    }
                    appState.onProjectSelected = { [projectStore] projectID in
                        projectStore.markActive(id: projectID)
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
    }

    private func startDeferredServicesIfNeeded() {
        guard !didStartDeferredServices else { return }
        didStartDeferredServices = true
        Task { @MainActor in
            await Task.yield()
            AIProviderRegistry.shared.prepareForInstallation()
            MuxyFileStorage.removeFile(named: "terminal-sessions.json")
            SettingsJSONStore.beginAutomaticUserSettingsSync()
            try? await Task.sleep(for: .seconds(2))
            UpdateService.shared.start()
            TerminalOfflineService.shared.start()
            await AIProviderRegistry.shared.installAll()
            await NotificationSocketServer.shared.awaitReady()
            ExtensionStore.shared.startAll()
            await ExtensionStore.shared.checkForUpdates()
            appDelegate.presentWhatsNewIfNeeded()
        }
    }

    private func startWorktreeAutoRefreshIfNeeded() {
        guard worktreeAutoRefresher == nil else { return }
        worktreeAutoRefresher = VCSWorktreeAutoRefresher(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    var onTerminate: (() -> Void)?
    var openProjectFromPath: ((String) -> Void)?
    var settingsContent: (() -> AnyView)?

    private var pendingOpenPaths: [String] = []
    private var pendingInstallName: String?
    private var isReadyForModals = false
    private var systemAppearanceObserver: NSObjectProtocol?
    private var settingsObserver: NSObjectProtocol?
    private var extensionsObserver: NSObjectProtocol?
    private var whatsNewObserver: NSObjectProtocol?
    private var modalThemeObserver: NSObjectProtocol?
    private weak var settingsWindow: NSWindow?
    private weak var extensionsWindow: NSWindow?
    private weak var whatsNewWindow: NSWindow?

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
        isReadyForModals = true
        if let name = pendingInstallName {
            pendingInstallName = nil
            presentExtensionsModal(installName: name)
        }
    }

    @MainActor
    func handleInstallExtension(name: String) {
        guard isReadyForModals else {
            pendingInstallName = name
            return
        }
        presentExtensionsModal(installName: name)
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

    nonisolated static func resolveInstallName(from url: URL) -> String? {
        guard url.scheme == "muxy", url.host == "extensions" else { return nil }
        let segments = url.pathComponents.filter { $0 != "/" }
        guard segments.first == "install" else { return nil }
        guard let raw = segments.dropFirst().first?.removingPercentEncoding, !raw.isEmpty else { return nil }
        guard (try? ExtensionManifestLoader.validate(name: raw)) != nil else { return nil }
        return raw
    }

    @MainActor
    func application(_ application: NSApplication, open urls: [URL]) {
        for url in urls {
            handleIncomingURL(url)
        }
    }

    @MainActor
    func handleIncomingURL(_ url: URL) {
        deepLinkLogger.log("incoming url: \(url.absoluteString, privacy: .public)")
        if let name = Self.resolveInstallName(from: url) {
            handleInstallExtension(name: name)
            return
        }
        guard let path = Self.resolveProjectPath(from: url) else { return }
        handleOpenProjectPath(path)
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
    private func registerURLEventHandler() {
        NSAppleEventManager.shared().setEventHandler(
            self,
            andSelector: #selector(handleURLEvent(_:withReplyEvent:)),
            forEventClass: AEEventClass(kInternetEventClass),
            andEventID: AEEventID(kAEGetURL)
        )
    }

    @MainActor
    func applicationWillFinishLaunching(_ notification: Notification) {
        registerURLEventHandler()
    }

    @MainActor
    @objc
    private func handleURLEvent(_ event: NSAppleEventDescriptor, withReplyEvent _: NSAppleEventDescriptor) {
        guard let string = event.paramDescriptor(forKeyword: AEKeyword(keyDirectObject))?.stringValue,
              let url = URL(string: string)
        else { return }
        handleIncomingURL(url)
    }

    @MainActor
    func applicationDidFinishLaunching(_ notification: Notification) {
        UserDefaults.standard.register(defaults: ["ApplePressAndHoldEnabled": false])
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
        DesktopNotificationService.shared.prepare()
        NotificationSocketServer.shared.openProjectHandler = { [weak self] path in
            Task { @MainActor [weak self] in
                self?.handleOpenProjectPath(path)
            }
        }
        NotificationSocketServer.shared.installExtensionHandler = { [weak self] name in
            Task { @MainActor [weak self] in
                self?.handleInstallExtension(name: name)
            }
        }
        NotificationSocketServer.shared.start()
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

    @MainActor
    @discardableResult
    static func activateMainWindowOnCurrentSpace() -> NSWindow? {
        NSApp.activate(ignoringOtherApps: true)
        guard let window = mainAppWindow() else { return nil }
        let previousBehavior = window.collectionBehavior
        window.collectionBehavior.insert(.moveToActiveSpace)
        window.makeKeyAndOrderFront(nil)
        window.collectionBehavior = previousBehavior
        return window
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard !AppRelaunch.isRelaunching else { return .terminateNow }
        return confirmQuitIfNeeded()
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

    func applicationWillTerminate(_ notification: Notification) {
        if let observer = systemAppearanceObserver {
            DistributedNotificationCenter.default().removeObserver(observer)
            systemAppearanceObserver = nil
        }
        if let settingsObserver {
            NotificationCenter.default.removeObserver(settingsObserver)
            self.settingsObserver = nil
        }
        if let extensionsObserver {
            NotificationCenter.default.removeObserver(extensionsObserver)
            self.extensionsObserver = nil
        }
        if let whatsNewObserver {
            NotificationCenter.default.removeObserver(whatsNewObserver)
            self.whatsNewObserver = nil
        }
        if let modalThemeObserver {
            NotificationCenter.default.removeObserver(modalThemeObserver)
            self.modalThemeObserver = nil
        }
        persistUserStateForTermination()
        NotificationStore.shared.saveToDisk()
        NotificationSocketServer.shared.stop()
        MainActor.assumeIsolated {
            MobileServerService.shared.stopForTermination()
            ExtensionStore.shared.stopAll()
        }
    }

    func persistUserStateForTermination() {
        guard !AppRelaunch.isRelaunching else { return }
        onTerminate?()
        RichInputDraftStore.shared.flush()
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
        extensionsObserver = NotificationCenter.default.addObserver(
            forName: .openExtensionsModal,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.presentExtensionsModal()
            }
        }
        whatsNewObserver = NotificationCenter.default.addObserver(
            forName: .openWhatsNewModal,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.presentWhatsNewModal()
            }
        }
        modalThemeObserver = NotificationCenter.default.addObserver(
            forName: .themeDidChange,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.settingsWindow?.backgroundColor = MuxyTheme.nsBg
                self?.extensionsWindow?.backgroundColor = MuxyTheme.nsBg
                self?.whatsNewWindow?.backgroundColor = MuxyTheme.nsBg
            }
        }
    }

    @MainActor
    private func presentSettingsModal() {
        let config = AppModalConfig(
            title: "Settings",
            size: CGSize(width: 980, height: 680),
            existing: settingsWindow,
            delegate: self,
            onClosed: { [weak self] in self?.settingsWindow = nil }
        )
        settingsWindow = AppModalPresenter.present(config) {
            settingsContent?() ?? AnyView(SettingsView())
        }
    }

    @MainActor
    private func presentExtensionsModal(installName: String? = nil) {
        let config = AppModalConfig(
            title: "Extensions",
            size: CGSize(width: 880, height: 620),
            existing: extensionsWindow,
            delegate: self,
            onClosed: { [weak self] in self?.extensionsWindow = nil }
        )
        extensionsWindow = AppModalPresenter.present(config) {
            ExtensionsView(installName: installName)
        }
        if let installName {
            NotificationCenter.default.post(
                name: .openExtensionInstall,
                object: nil,
                userInfo: [ExtensionInstallUserInfoKey.name: installName]
            )
        }
    }

    @MainActor
    private func presentWhatsNewModal(preloadedMarkdown: String? = nil) {
        guard let version = WhatsNewPreferences.currentVersion else { return }
        if preloadedMarkdown != nil {
            whatsNewWindow?.close()
            whatsNewWindow = nil
        }
        let config = AppModalConfig(
            title: "What's New",
            size: CGSize(width: 806, height: 560),
            existing: whatsNewWindow,
            delegate: self,
            onClosed: { [weak self] in self?.whatsNewWindow = nil }
        )
        whatsNewWindow = AppModalPresenter.present(config) {
            WhatsNewView(version: version, preloadedMarkdown: preloadedMarkdown)
        }
    }

    @MainActor
    func presentWhatsNewIfNeeded() {
        guard WhatsNewPreferences.shouldAutoShow,
              let version = WhatsNewPreferences.currentVersion
        else { return }
        Task { @MainActor in
            guard let markdown = try? await WhatsNewService.fetchReleaseNotes(version: version)
            else { return }
            presentWhatsNewModal(preloadedMarkdown: markdown)
            WhatsNewPreferences.markCurrentVersionViewed()
        }
    }

    func windowWillClose(_ notification: Notification) {
        let closed = notification.object as? NSWindow
        if closed === settingsWindow { settingsWindow = nil }
        if closed === extensionsWindow { extensionsWindow = nil }
        if closed === whatsNewWindow { whatsNewWindow = nil }
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
