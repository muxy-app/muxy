import AppKit
import SwiftUI

@main
struct MuxyApp: App {
    static let launchDate = Date()

    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @State private var appState: AppState
    @State private var projectStore: ProjectStore
    @State private var worktreeStore: WorktreeStore
    private let updateService = UpdateService.shared

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
        _appState = State(initialValue: appState)
        _projectStore = State(initialValue: projectStore)
        _worktreeStore = State(initialValue: worktreeStore)
    }

    var body: some Scene {
        WindowGroup {
            MainWindow()
                .environment(appState)
                .environment(projectStore)
                .environment(worktreeStore)
                .environment(GhosttyService.shared)
                .environment(MuxyConfig.shared)
                .environment(ThemeService.shared)
                .preferredColorScheme(MuxyTheme.colorScheme)
                .onAppear {
                    NotificationStore.shared.appState = appState
                    NotificationStore.shared.worktreeStore = worktreeStore
                    NotificationStore.shared.markAllAsRead()
                    appDelegate.onTerminate = { [appState] in
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
                    NotificationSocketServer.shared.openProjectHandler = { [appDelegate] path in
                        Task { @MainActor in
                            appDelegate.handleOpenProjectPath(path)
                        }
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
                }
        }
        .windowStyle(HiddenTitleBarWindowStyle())
        .defaultSize(width: 1200, height: 800)
        .commands {
            MuxyCommands(
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
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
                .environment(GhosttyService.shared)
                .preferredColorScheme(MuxyTheme.colorScheme)
        }
        .defaultSize(width: 700, height: 600)

        Settings {
            SettingsView()
                .preferredColorScheme(MuxyTheme.colorScheme)
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    var onTerminate: (() -> Void)?
    var hasUnsavedEditorTabs: (() -> [EditorTabState])?
    var openProjectFromPath: ((String) -> Void)?

    private var pendingOpenPaths: [String] = []
    private var systemAppearanceObserver: NSObjectProtocol?

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

    @MainActor
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate()
        setAppIcon()
        _ = GhosttyService.shared
        GhosttyService.shared.applyInitialColorScheme()
        ThemeService.shared.applyDefaultThemeIfNeeded()
        ThemeService.shared.migrateToPairedThemeIfNeeded()
        observeSystemAppearanceChanges()
        UpdateService.shared.start()
        ModifierKeyMonitor.shared.start()
        NotificationSocketServer.shared.start()
        AIProviderRegistry.shared.installAll()
        _ = AIUsageSettingsStore.isUsageEnabled()

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

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        let unsaved = hasUnsavedEditorTabs?() ?? []
        guard !unsaved.isEmpty else { return .terminateNow }

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
        onTerminate?()
        NotificationStore.shared.saveToDisk()
        NotificationSocketServer.shared.stop()
        MainActor.assumeIsolated {
            MobileServerService.shared.stopForTermination()
        }
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

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> NSView {
        let v = NSView()
        DispatchQueue.main.async {
            guard let w = v.window else { return }
            w.identifier = ShortcutContext.mainWindowIdentifier
            w.titlebarAppearsTransparent = true
            w.titleVisibility = .hidden
            w.styleMask.insert(.fullSizeContentView)
            w.isMovable = false
            w.isMovableByWindowBackground = false
            Self.applyWindowBackground(w)
            Self.repositionTrafficLights(in: w)
            Self.hideTitlebarDecorationView(in: w)
            Self.neutralizeSafeAreaInsets(in: w)
            context.coordinator.observe(window: w)
        }
        return v
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        guard let w = nsView.window else { return }
        Self.applyWindowBackground(w)
    }

    private static func applyWindowBackground(_ window: NSWindow) {
        window.isOpaque = false
        window.backgroundColor = .clear
        window.contentView?.wantsLayer = true
        window.contentView?.layer?.backgroundColor = MuxyTheme.nsBg.cgColor
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

    static let trafficLightY: CGFloat = 3.5

    static func repositionTrafficLights(in window: NSWindow) {
        let y: CGFloat
        if #available(macOS 26.0, *) {
            let buttonHeight: CGFloat = 14
            y = (32 - buttonHeight) / 2
        } else {
            y = trafficLightY
        }
        for button in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
            guard let btn = window.standardWindowButton(button) else { continue }
            var frame = btn.frame
            frame.origin.y = y
            btn.frame = frame
        }
    }

    final class Coordinator: NSObject {
        private var observations: [NSObjectProtocol] = []

        func observe(window: NSWindow) {
            guard observations.isEmpty else { return }

            let names: [Notification.Name] = [
                NSWindow.didResizeNotification,
                NSWindow.didEndLiveResizeNotification,
                NSWindow.didChangeScreenNotification,
                NSWindow.didChangeBackingPropertiesNotification,
                NSWindow.didExitFullScreenNotification,
                NSWindow.didEnterFullScreenNotification,
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
        }

        deinit {
            observations.forEach { NotificationCenter.default.removeObserver($0) }
        }
    }
}
