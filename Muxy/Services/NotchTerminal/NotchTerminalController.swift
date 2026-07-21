import AppKit

@MainActor
final class NotchTerminalController: NSObject {
    typealias ShortcutLabelProvider = @MainActor () -> String
    typealias SettingsHandler = @MainActor () -> Void
    typealias SizeProvider = @MainActor () -> NSSize
    typealias AppearanceProvider = @MainActor () -> NotchTerminalAppearance

    private static let showDuration: TimeInterval = 0.34
    private static let hideDuration: TimeInterval = 0.18

    private let session: NotchTerminalSession
    private let hoverController: NotchHoverController
    private let shortcutLabelProvider: ShortcutLabelProvider
    private let settingsHandler: SettingsHandler
    private let shortcutService: NotchTerminalShortcutService
    private let screenProvider: @MainActor () -> NSScreen?
    private let sizeProvider: SizeProvider
    private let appearanceProvider: AppearanceProvider
    private let reduceMotionProvider: @MainActor () -> Bool
    private let reduceTransparencyProvider: @MainActor () -> Bool
    private let notificationCenter: NotificationCenter
    private let workspaceNotificationCenter: NotificationCenter
    private var panel: NotchTerminalPanel?
    private var contentView: NotchTerminalContentView?
    private var presentation = NotchTerminalPresentationState()
    private var completionWorkItem: DispatchWorkItem?
    private var focusSnapshot: NotchTerminalFocusSnapshot?
    private var isTerminated = false

    init(
        shortcutLabelProvider: @escaping ShortcutLabelProvider,
        onOpenSettings: @escaping SettingsHandler,
        shortcutService: NotchTerminalShortcutService = .shared,
        session: NotchTerminalSession = NotchTerminalSession(),
        hoverController: NotchHoverController = NotchHoverController(),
        screenProvider: @escaping @MainActor () -> NSScreen? = { NotchTerminalScreenResolver.activeScreen() },
        sizeProvider: @escaping SizeProvider = { NotchTerminalSizePreferences.size() },
        appearanceProvider: @escaping AppearanceProvider = { NotchTerminalAppearancePreferences.appearance() },
        reduceMotionProvider: @escaping @MainActor () -> Bool = {
            NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        },
        reduceTransparencyProvider: @escaping @MainActor () -> Bool = {
            NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency
                || NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast
        },
        notificationCenter: NotificationCenter = .default,
        workspaceNotificationCenter: NotificationCenter = NSWorkspace.shared.notificationCenter
    ) {
        self.shortcutLabelProvider = shortcutLabelProvider
        settingsHandler = onOpenSettings
        self.shortcutService = shortcutService
        self.session = session
        self.hoverController = hoverController
        self.screenProvider = screenProvider
        self.sizeProvider = sizeProvider
        self.appearanceProvider = appearanceProvider
        self.reduceMotionProvider = reduceMotionProvider
        self.reduceTransparencyProvider = reduceTransparencyProvider
        self.notificationCenter = notificationCenter
        self.workspaceNotificationCenter = workspaceNotificationCenter
        super.init()
        session.onProcessExit = { [weak self] in
            self?.handleProcessExit()
        }
        hoverController.onOpenRequested = { [weak self] in
            self?.show()
        }
        notificationCenter.addObserver(
            self,
            selector: #selector(handleApplicationWillTerminate),
            name: NSApplication.willTerminateNotification,
            object: nil
        )
        notificationCenter.addObserver(
            self,
            selector: #selector(handleScreenParametersDidChange),
            name: NSApplication.didChangeScreenParametersNotification,
            object: nil
        )
        notificationCenter.addObserver(
            self,
            selector: #selector(handleGhosttyConfigurationDidChange),
            name: .ghosttyConfigurationDidChange,
            object: nil
        )
        workspaceNotificationCenter.addObserver(
            self,
            selector: #selector(handleAccessibilityDisplayOptionsDidChange),
            name: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification,
            object: nil
        )
    }

    override convenience init() {
        self.init(shortcutLabelProvider: { "⇧ ⇧" }, onOpenSettings: {})
    }

    deinit {
        notificationCenter.removeObserver(self)
        workspaceNotificationCenter.removeObserver(self)
    }

    var isVisible: Bool { presentation.targetIsVisible }

    func toggle() {
        setVisible(!presentation.targetIsVisible, restoresFocus: true)
    }

    func show() {
        setVisible(true, restoresFocus: true)
    }

    func hide() {
        setVisible(false, restoresFocus: true)
    }

    func startHoverZones() {
        hoverController.start()
    }

    func applicationWillTerminate() {
        guard !isTerminated else { return }
        isTerminated = true
        completionWorkItem?.cancel()
        completionWorkItem = nil
        hoverController.tearDown()
        panel?.orderOut(nil)
        session.terminate()
        contentView?.clearTerminal(status: "Closed")
        focusSnapshot = nil
    }

    private func setVisible(_ visible: Bool, restoresFocus: Bool) {
        guard !isTerminated,
              let transition = presentation.requestVisibility(visible)
        else { return }
        completionWorkItem?.cancel()
        completionWorkItem = nil
        if visible {
            present(transition)
        } else {
            dismiss(transition, restoresFocus: restoresFocus)
        }
    }

    private func present(_ transition: NotchTerminalPresentationTransition) {
        let panel = makePanelIfNeeded()
        guard let surface = session.surfaceForPresentation(), let screen = screenProvider() else {
            _ = presentation.complete(transition)
            return
        }
        if focusSnapshot == nil {
            focusSnapshot = NotchTerminalFocusSnapshot.capture(excluding: panel)
        }
        let frame = NotchTerminalGeometry.frame(
            screenFrame: screen.frame,
            visibleFrame: screen.visibleFrame,
            preferredSize: sizeProvider()
        )
        panel.setFrame(frame, display: true)
        contentView?.frame = NSRect(origin: .zero, size: frame.size)
        contentView?.setCollapsedNotchRect(collapsedNotchRect(screen: screen, panelFrame: frame))
        contentView?.attach(surface: surface)
        applyCurrentAppearance()
        contentView?.setShortcutLabel(shortcutLabelProvider())
        session.markVisible(true)
        hoverController.notifyOpened()

        let duration = reduceMotionProvider() ? 0 : Self.showDuration
        if !panel.isVisible {
            contentView?.setRevealProgress(false)
            panel.orderFrontRegardless()
        }
        panel.makeKey()
        panel.makeFirstResponder(surface.notchTerminalView)
        contentView?.animateReveal(true, duration: duration)
        scheduleCompletion(transition, duration: duration)
    }

    private func dismiss(_ transition: NotchTerminalPresentationTransition, restoresFocus: Bool) {
        session.markVisible(false)
        contentView?.hideShortcutSettings()
        let duration = reduceMotionProvider() ? 0 : Self.hideDuration
        contentView?.animateReveal(false, duration: duration)
        scheduleCompletion(transition, duration: duration) { [weak self] in
            guard let self else { return }
            self.panel?.orderOut(nil)
            if restoresFocus {
                self.focusSnapshot?.restore()
            }
            self.focusSnapshot = nil
            self.hoverController.notifyClosed()
        }
    }

    private func collapsedNotchRect(screen: NSScreen, panelFrame: NSRect) -> NSRect? {
        guard let notchRect = NotchTerminalNotchGeometry.notchRect(for: screen) else { return nil }
        return NotchTerminalNotchGeometry.collapsedRect(notchRect: notchRect, panelFrame: panelFrame)
    }

    private func scheduleCompletion(
        _ transition: NotchTerminalPresentationTransition,
        duration: TimeInterval,
        completion: (@MainActor () -> Void)? = nil
    ) {
        guard duration > 0 else {
            guard presentation.complete(transition) else { return }
            completion?()
            return
        }
        let workItem = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated {
                guard let self, self.presentation.complete(transition) else { return }
                self.completionWorkItem = nil
                completion?()
            }
        }
        completionWorkItem = workItem
        DispatchQueue.main.asyncAfter(deadline: .now() + duration, execute: workItem)
    }

    private func makePanelIfNeeded() -> NotchTerminalPanel {
        if let panel {
            return panel
        }
        let contentView = NotchTerminalContentView(frame: .zero)
        let panel = NotchTerminalPanel(contentRect: .zero)
        panel.contentView = contentView
        panel.onKeyDown = { [weak contentView] event in
            contentView?.handleKeyDown(event) ?? false
        }
        contentView.onClose = { [weak self] in
            self?.hide()
        }
        contentView.onOpenSettings = { [weak self] in
            guard let self else { return }
            self.setVisible(false, restoresFocus: false)
            self.settingsHandler()
        }
        contentView.shortcutSettingsProvider = { [weak shortcutService] in
            guard let shortcutService else {
                return NotchTerminalShortcutSettingsSnapshot(
                    shortcut: .default,
                    monitoringState: .stopped,
                    errorMessage: nil
                )
            }
            return NotchTerminalShortcutSettingsSnapshot(
                shortcut: shortcutService.shortcut,
                monitoringState: shortcutService.monitoringState,
                errorMessage: shortcutService.errorMessage
            )
        }
        contentView.onShortcutChange = { [weak self] shortcut in
            self?.updateShortcut(shortcut)
        }
        contentView.onRequestInputMonitoringAccess = { [weak shortcutService] in
            shortcutService?.requestInputMonitoringAccess() ?? false
        }
        self.panel = panel
        self.contentView = contentView
        return panel
    }

    private func updateShortcut(_ shortcut: NotchTerminalShortcut) -> String? {
        if case let .keyCombo(combo, _) = shortcut,
           let conflict = NotchTerminalShortcutConflictResolver.conflictMessage(for: combo)
        {
            return conflict
        }
        do {
            try shortcutService.updateShortcut(shortcut)
            contentView?.setShortcutLabel(shortcut.displayString)
            return nil
        } catch {
            return error.localizedDescription
        }
    }

    private func applyCurrentAppearance() {
        let appearance = appearanceProvider().resolvingReduceTransparency(reduceTransparencyProvider())
        contentView?.applyAppearance(appearance)
        session.applyAppearance(appearance)
    }

    private func handleProcessExit() {
        contentView?.clearTerminal(status: "Shell exited")
        guard presentation.targetIsVisible else { return }
        setVisible(false, restoresFocus: true)
    }

    @objc
    private func handleApplicationWillTerminate() {
        applicationWillTerminate()
    }

    @objc
    private func handleScreenParametersDidChange() {
        hoverController.refreshForScreenChange()
        guard presentation.targetIsVisible, let panel, let screen = screenProvider() else { return }
        let frame = NotchTerminalGeometry.frame(
            screenFrame: screen.frame,
            visibleFrame: screen.visibleFrame,
            preferredSize: sizeProvider()
        )
        panel.setFrame(frame, display: true)
        contentView?.frame = NSRect(origin: .zero, size: frame.size)
        contentView?.setCollapsedNotchRect(collapsedNotchRect(screen: screen, panelFrame: frame))
    }

    @objc
    private func handleGhosttyConfigurationDidChange() {
        guard !isTerminated else { return }
        applyCurrentAppearance()
    }

    @objc
    private func handleAccessibilityDisplayOptionsDidChange() {
        guard !isTerminated else { return }
        applyCurrentAppearance()
    }
}

@MainActor
private final class NotchTerminalFocusSnapshot {
    private weak var window: NSWindow?
    private let application: NSRunningApplication?

    private init(window: NSWindow?, application: NSRunningApplication?) {
        self.window = window
        self.application = application
    }

    static func capture(excluding panel: NSPanel) -> NotchTerminalFocusSnapshot {
        let window = NSApp.keyWindow === panel ? nil : NSApp.keyWindow
        return NotchTerminalFocusSnapshot(
            window: window,
            application: NSWorkspace.shared.frontmostApplication
        )
    }

    func restore() {
        guard let application, !application.isTerminated else { return }
        if application.processIdentifier == ProcessInfo.processInfo.processIdentifier {
            guard let window, window.isVisible else { return }
            window.makeKeyAndOrderFront(nil)
            return
        }
        application.activate(options: [])
    }
}
