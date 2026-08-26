import AppKit
import CoreGraphics
import SwiftUI

@MainActor
private final class GlobalWorkspaceModel {
    let appState: AppState
    let projectStore: ProjectStore
    let worktreeStore: WorktreeStore
    let projectGroupStore: ProjectGroupStore
    let remoteDeviceStore: RemoteDeviceStore
    let browserProfileStore: BrowserProfileStore
    let browserHistoryStore: BrowserHistoryStore

    init(
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore,
        remoteDeviceStore: RemoteDeviceStore,
        browserProfileStore: BrowserProfileStore,
        browserHistoryStore: BrowserHistoryStore
    ) {
        let environment = AppEnvironment.live
        let appState = AppState(
            selectionStore: UserDefaultsActiveProjectSelectionStore(
                projectKey: "muxy.hotkey.activeProjectID",
                worktreesKey: "muxy.hotkey.activeWorktreeIDs"
            ),
            terminalViews: environment.terminalViews,
            workspacePersistence: FileWorkspacePersistence(
                fileURL: MuxyFileStorage.fileURL(filename: "hotkey-workspaces.json")
            )
        )
        appState.onWorkspaceSelected = { key in
            if projectStore.storedProjects.contains(where: { $0.id == key.projectID }) {
                projectStore.markActive(id: key.projectID)
            } else {
                projectGroupStore.markRemoteProjectActive(id: key.projectID)
            }
            worktreeStore.markActive(projectID: key.projectID, worktreeID: key.worktreeID)
        }
        appState.restoreSelection(
            projects: projectStore.projects,
            worktrees: worktreeStore.worktrees,
            skippingProjectIDs: projectGroupStore.activeRemoteProjectIDs
        )

        self.appState = appState
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
        self.projectGroupStore = projectGroupStore
        self.remoteDeviceStore = remoteDeviceStore
        self.browserProfileStore = browserProfileStore
        self.browserHistoryStore = browserHistoryStore
    }

    func ensureWorkspaceReady() {
        if appState.activeProjectID == nil {
            _ = HomeProjectService.openHomeTab(
                appState: appState,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore
            )
        }
        guard let projectID = appState.activeProjectID, !appState.hasTabs(for: projectID) else { return }
        appState.createTab(projectID: projectID)
    }

    func save() {
        appState.saveWorkspaces()
    }
}

private final class GlobalWorkspacePanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

@MainActor
final class GlobalWorkspaceController: NSObject, NSWindowDelegate {
    static let shared = GlobalWorkspaceController()

    private struct Dependencies {
        let projectStore: ProjectStore
        let worktreeStore: WorktreeStore
        let projectGroupStore: ProjectGroupStore
        let remoteDeviceStore: RemoteDeviceStore
        let browserProfileStore: BrowserProfileStore
        let browserHistoryStore: BrowserHistoryStore
    }

    private static let windowCollectionBehavior: NSWindow.CollectionBehavior = [
        .canJoinAllSpaces,
        .fullScreenAuxiliary,
        .transient,
        .ignoresCycle,
    ]
    private static let windowLevel = NSWindow.Level(rawValue: Int(CGWindowLevelForKey(.mainMenuWindow)) - 2)
    private static let fullScreenWindowLevel = NSWindow.Level(rawValue: Int(CGWindowLevelForKey(.mainMenuWindow)) + 1)

    private var dependencies: Dependencies?
    private var panel: GlobalWorkspacePanel?
    private var model: GlobalWorkspaceModel?
    private var previousApplication: NSRunningApplication?
    private var fullScreenShortcutMonitor: Any?
    private var terminationObserver: NSObjectProtocol?
    private var overlayRestoreFrame: NSRect?
    private var overlayRestoreStyleMask: NSWindow.StyleMask?
    private var overlayRestoreHasShadow = true
    private(set) var isPresented = false
    private(set) var isOverlayFullScreen = false

    override private init() {
        super.init()
        terminationObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.willTerminateNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.stop(restoresFocus: false)
            }
        }
    }

    deinit {
        MainActor.assumeIsolated {
            if let terminationObserver {
                NotificationCenter.default.removeObserver(terminationObserver)
            }
        }
    }

    func configure(
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore,
        remoteDeviceStore: RemoteDeviceStore,
        browserStores: (profiles: BrowserProfileStore, history: BrowserHistoryStore)
    ) {
        dependencies = Dependencies(
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            remoteDeviceStore: remoteDeviceStore,
            browserProfileStore: browserStores.profiles,
            browserHistoryStore: browserStores.history
        )
    }

    func toggle() {
        if isPresented {
            hide()
        } else {
            show()
        }
    }

    func show() {
        guard !isPresented else { return }
        createPanelIfNeeded()
        guard let panel, let model else { return }
        model.ensureWorkspaceReady()
        capturePreviousApplication()
        if !isOverlayFullScreen {
            applyWindowPresentation(panel)
            panel.setFrame(preferredFrame(), display: true)
        }
        isPresented = true
        panel.orderFrontRegardless()
        panel.makeKeyAndOrderFront(nil)
    }

    func hide() {
        guard isPresented else { return }
        model?.save()
        panel?.orderOut(nil)
        isPresented = false
        restorePreviousApplication()
    }

    func toggleOverlayFullScreen() {
        guard let panel else { return }
        if isOverlayFullScreen {
            exitOverlayFullScreen(panel)
        } else {
            enterOverlayFullScreen(panel)
        }
    }

    func stop(restoresFocus: Bool) {
        if restoresFocus, isPresented {
            restorePreviousApplication()
        } else {
            previousApplication = nil
        }
        model?.save()
        removeFullScreenShortcutMonitor()
        panel?.orderOut(nil)
        panel?.contentViewController = nil
        panel?.delegate = nil
        panel?.close()
        panel = nil
        model = nil
        isPresented = false
        isOverlayFullScreen = false
        overlayRestoreFrame = nil
        overlayRestoreStyleMask = nil
    }

    private func createPanelIfNeeded() {
        guard panel == nil, let dependencies else { return }
        let model = GlobalWorkspaceModel(
            projectStore: dependencies.projectStore,
            worktreeStore: dependencies.worktreeStore,
            projectGroupStore: dependencies.projectGroupStore,
            remoteDeviceStore: dependencies.remoteDeviceStore,
            browserProfileStore: dependencies.browserProfileStore,
            browserHistoryStore: dependencies.browserHistoryStore
        )
        let panel = GlobalWorkspacePanel(
            contentRect: preferredFrame(),
            styleMask: normalStyleMask,
            backing: .buffered,
            defer: false
        )
        panel.identifier = ShortcutContext.globalWorkspaceWindowIdentifier
        panel.delegate = self
        panel.isReleasedWhenClosed = false
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        panel.becomesKeyOnlyIfNeeded = false
        panel.titlebarAppearsTransparent = true
        panel.titleVisibility = .hidden
        panel.isMovable = false
        panel.isMovableByWindowBackground = false
        configureWindowButtons(panel)
        applyWindowPresentation(panel)
        installFullScreenShortcutMonitor(for: panel)

        let rootView = LocalizationEnvironment {
            MainWindow(windowIdentifier: ShortcutContext.globalWorkspaceWindowIdentifier)
                .environment(model.appState)
                .environment(model.projectStore)
                .environment(model.worktreeStore)
                .environment(model.projectGroupStore)
                .environment(model.remoteDeviceStore)
                .environment(model.browserProfileStore)
                .environment(model.browserHistoryStore)
                .environment(SSHConnectionService.shared)
                .environment(GhosttyService.shared)
                .environment(MuxyConfig.shared)
                .environment(ThemeService.shared)
                .environment(ExtensionStore.shared)
                .environment(ExtensionSettingsStore.shared)
                .preferredColorScheme(MuxyTheme.colorScheme)
        }
        panel.contentViewController = NSHostingController(rootView: rootView)
        panel.orderOut(nil)

        self.model = model
        self.panel = panel
    }

    private var normalStyleMask: NSWindow.StyleMask {
        [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView, .nonactivatingPanel]
    }

    private func configureWindowButtons(_ panel: GlobalWorkspacePanel) {
        if let closeButton = panel.standardWindowButton(.closeButton) {
            closeButton.target = self
            closeButton.action = #selector(handleCloseButton(_:))
        }
        if let zoomButton = panel.standardWindowButton(.zoomButton) {
            zoomButton.target = self
            zoomButton.action = #selector(handleFullScreenButton(_:))
        }
    }

    private func installFullScreenShortcutMonitor(for panel: NSWindow) {
        guard fullScreenShortcutMonitor == nil else { return }
        fullScreenShortcutMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self, weak panel] event in
            guard let self,
                  let panel,
                  panel.isKeyWindow,
                  ShortcutContext.isGlobalWorkspaceWindow(panel),
                  KeyBindingStore.shared.combo(for: .toggleFullScreen).matches(event: event)
            else { return event }
            self.toggleOverlayFullScreen()
            return nil
        }
    }

    private func removeFullScreenShortcutMonitor() {
        guard let fullScreenShortcutMonitor else { return }
        NSEvent.removeMonitor(fullScreenShortcutMonitor)
        self.fullScreenShortcutMonitor = nil
    }

    private func enterOverlayFullScreen(_ panel: GlobalWorkspacePanel) {
        guard !isOverlayFullScreen,
              let screen = panel.screen ?? screenUnderMouse() ?? NSScreen.main
        else { return }
        overlayRestoreFrame = panel.frame
        overlayRestoreStyleMask = panel.styleMask
        overlayRestoreHasShadow = panel.hasShadow
        isOverlayFullScreen = true
        panel.styleMask = [.borderless, .resizable, .nonactivatingPanel]
        panel.hasShadow = false
        panel.collectionBehavior = Self.windowCollectionBehavior
        panel.level = Self.fullScreenWindowLevel
        panel.setFrame(screen.frame, display: true, animate: true)
        WindowConfigurator.neutralizeSafeAreaInsets(in: panel)
        postFullScreenChange(true, panel: panel)
        panel.orderFrontRegardless()
        panel.makeKeyAndOrderFront(nil)
    }

    private func exitOverlayFullScreen(_ panel: GlobalWorkspacePanel) {
        guard isOverlayFullScreen else { return }
        let restoreFrame = overlayRestoreFrame ?? preferredFrame()
        let restoreStyleMask = overlayRestoreStyleMask ?? normalStyleMask
        isOverlayFullScreen = false
        overlayRestoreFrame = nil
        overlayRestoreStyleMask = nil
        postFullScreenChange(false, panel: panel)
        panel.styleMask = restoreStyleMask
        panel.hasShadow = overlayRestoreHasShadow
        panel.titlebarAppearsTransparent = true
        panel.titleVisibility = .hidden
        panel.isMovable = false
        panel.isMovableByWindowBackground = false
        configureWindowButtons(panel)
        applyWindowPresentation(panel)
        WindowConfigurator.neutralizeSafeAreaInsets(in: panel)
        panel.setFrame(restoreFrame, display: true, animate: true)
        panel.orderFrontRegardless()
        panel.makeKeyAndOrderFront(nil)
    }

    private func applyWindowPresentation(_ panel: NSWindow) {
        panel.collectionBehavior = Self.windowCollectionBehavior
        panel.level = Self.windowLevel
    }

    private func postFullScreenChange(_ isFullScreen: Bool, panel: NSWindow) {
        NotificationCenter.default.post(
            name: .windowFullScreenDidChange,
            object: panel,
            userInfo: ["isFullScreen": isFullScreen]
        )
    }

    private func preferredFrame() -> NSRect {
        guard let visibleFrame = (screenUnderMouse() ?? NSScreen.main ?? NSScreen.screens.first)?.visibleFrame else {
            return NSRect(x: 120, y: 100, width: 1200, height: 800)
        }
        let width = min(visibleFrame.width, max(900, visibleFrame.width * 0.86))
        let height = min(visibleFrame.height, max(600, visibleFrame.height * 0.82))
        return NSRect(
            x: visibleFrame.midX - width / 2,
            y: visibleFrame.midY - height / 2,
            width: width,
            height: height
        )
    }

    private func screenUnderMouse() -> NSScreen? {
        let location = NSEvent.mouseLocation
        return NSScreen.screens.first { NSMouseInRect(location, $0.frame, false) }
    }

    private func capturePreviousApplication() {
        guard let application = NSWorkspace.shared.frontmostApplication,
              application.processIdentifier != ProcessInfo.processInfo.processIdentifier
        else {
            previousApplication = nil
            return
        }
        previousApplication = application
    }

    private func restorePreviousApplication() {
        let application = previousApplication
        previousApplication = nil
        guard let application, !application.isTerminated else { return }
        DispatchQueue.main.async {
            application.activate(options: [.activateIgnoringOtherApps])
        }
    }

    @objc
    private func handleCloseButton(_: Any?) {
        hide()
    }

    @objc
    private func handleFullScreenButton(_: Any?) {
        toggleOverlayFullScreen()
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        hide()
        return false
    }
}
