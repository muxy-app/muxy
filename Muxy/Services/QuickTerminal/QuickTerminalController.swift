import AppKit

@MainActor
final class QuickTerminalController: NSObject {
    typealias ShortcutLabelProvider = @MainActor () -> String
    typealias SettingsHandler = @MainActor () -> Void
    typealias SizeProvider = @MainActor () -> NSSize
    typealias AppearanceProvider = @MainActor () -> QuickTerminalAppearance

    private static let showDuration: TimeInterval = 0.34
    private static let hideDuration: TimeInterval = 0.18

    private let session: QuickTerminalSession
    private let shortcutLabelProvider: ShortcutLabelProvider
    private let settingsHandler: SettingsHandler
    private let shortcutService: QuickTerminalShortcutService
    private let screenProvider: @MainActor () -> NSScreen?
    private let sizeProvider: SizeProvider
    private let appearanceProvider: AppearanceProvider
    private let reduceMotionProvider: @MainActor () -> Bool
    private let reduceTransparencyProvider: @MainActor () -> Bool
    private let notificationCenter: NotificationCenter
    private let workspaceNotificationCenter: NotificationCenter
    private var panel: QuickTerminalPanel?
    private var contentView: QuickTerminalContentView?
    private var presentation = QuickTerminalPresentationState()
    private var completionWorkItem: DispatchWorkItem?
    private var focusSnapshot: QuickTerminalFocusSnapshot?
    private var isTerminated = false

    init(
        shortcutLabelProvider: @escaping ShortcutLabelProvider,
        onOpenSettings: @escaping SettingsHandler,
        shortcutService: QuickTerminalShortcutService = .shared,
        session: QuickTerminalSession = QuickTerminalSession(),
        screenProvider: @escaping @MainActor () -> NSScreen? = { QuickTerminalScreenResolver.activeScreen() },
        sizeProvider: @escaping SizeProvider = { QuickTerminalSizePreferences.size() },
        appearanceProvider: @escaping AppearanceProvider = { QuickTerminalAppearancePreferences.appearance() },
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
        shortcutService.onTrigger = { [weak self] in
            self?.handleShortcutTrigger()
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
        self.init(shortcutLabelProvider: { QuickTerminalShortcut.default.controlLabel }, onOpenSettings: {})
    }

    deinit {
        notificationCenter.removeObserver(self)
        workspaceNotificationCenter.removeObserver(self)
    }

    var isVisible: Bool { presentation.targetIsVisible }

    private func handleShortcutTrigger() {
        setVisible(!presentation.targetIsVisible, restoresFocus: true)
    }

    private func hide() {
        setVisible(false, restoresFocus: true)
    }

    func applicationWillTerminate() {
        stop(restoresFocus: false)
    }

    func stop(restoresFocus: Bool) {
        guard !isTerminated else { return }
        isTerminated = true
        completionWorkItem?.cancel()
        completionWorkItem = nil
        let shouldRestoreFocus = QuickTerminalFocusRestorationPolicy.shouldRestore(
            requested: restoresFocus,
            panelIsKey: panel?.isKeyWindow == true
        )
        session.markVisible(false)
        contentView?.hideConfigurationOverlays()
        panel?.orderOut(nil)
        if shouldRestoreFocus {
            focusSnapshot?.restore()
        }
        contentView?.clearTerminal(status: "Closed")
        session.terminate()
        panel?.onKeyDown = nil
        panel?.contentView = nil
        panel?.close()
        panel = nil
        contentView = nil
        focusSnapshot = nil
        presentation = QuickTerminalPresentationState()
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

    private func present(_ transition: QuickTerminalPresentationTransition) {
        let panel = makePanelIfNeeded()
        guard let surface = session.surfaceForPresentation(), let screen = screenProvider() else {
            _ = presentation.complete(transition)
            return
        }
        if QuickTerminalFocusRestorationPolicy.shouldCapture(
            hasSnapshot: focusSnapshot != nil,
            panelIsKey: panel.isKeyWindow
        ) {
            focusSnapshot = QuickTerminalFocusSnapshot.capture(excluding: panel)
        }
        let frame = QuickTerminalGeometry.frame(
            screenFrame: screen.frame,
            visibleFrame: screen.visibleFrame,
            preferredSize: sizeProvider()
        )
        panel.setFrame(frame, display: true)
        contentView?.frame = NSRect(origin: .zero, size: frame.size)
        contentView?.setCollapsedCutoutRect(collapsedCutoutRect(screen: screen, panelFrame: frame))
        contentView?.attach(surface: surface)
        applyCurrentAppearance()
        contentView?.setShortcutLabel(shortcutLabelProvider())
        session.markVisible(true)

        let duration = reduceMotionProvider() ? 0 : Self.showDuration
        if !panel.isVisible {
            contentView?.setRevealProgress(false)
            panel.orderFrontRegardless()
        }
        panel.makeKey()
        panel.makeFirstResponder(surface.quickTerminalView)
        contentView?.animateReveal(true, duration: duration)
        scheduleCompletion(transition, duration: duration)
    }

    private func dismiss(_ transition: QuickTerminalPresentationTransition, restoresFocus: Bool) {
        session.markVisible(false)
        contentView?.hideConfigurationOverlays()
        let duration = reduceMotionProvider() ? 0 : Self.hideDuration
        contentView?.animateReveal(false, duration: duration)
        scheduleCompletion(transition, duration: duration) { [weak self] in
            guard let self else { return }
            let shouldRestoreFocus = QuickTerminalFocusRestorationPolicy.shouldRestore(
                requested: restoresFocus,
                panelIsKey: self.panel?.isKeyWindow == true
            )
            self.panel?.orderOut(nil)
            if shouldRestoreFocus {
                self.focusSnapshot?.restore()
            }
            self.focusSnapshot = nil
        }
    }

    private func collapsedCutoutRect(screen: NSScreen, panelFrame: NSRect) -> NSRect? {
        guard let cutoutRect = QuickTerminalCutoutGeometry.cutoutRect(for: screen) else { return nil }
        return QuickTerminalCutoutGeometry.collapsedRect(cutoutRect: cutoutRect, panelFrame: panelFrame)
    }

    private func scheduleCompletion(
        _ transition: QuickTerminalPresentationTransition,
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

    private func makePanelIfNeeded() -> QuickTerminalPanel {
        if let panel {
            return panel
        }
        let contentView = QuickTerminalContentView(frame: .zero)
        let panel = QuickTerminalPanel(contentRect: .zero)
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
                return QuickTerminalShortcutSettingsSnapshot(
                    shortcut: .default,
                    monitoringState: .stopped,
                    errorMessage: nil
                )
            }
            return QuickTerminalShortcutSettingsSnapshot(
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
        contentView.quickSettingsProvider = { [weak self] in
            guard let self else {
                return QuickTerminalQuickSettings(transparency: 0, blurIntensity: 0, width: 0, height: 0)
            }
            let appearance = self.appearanceProvider()
            let size = self.sizeProvider()
            return QuickTerminalQuickSettings(
                transparency: appearance.transparency,
                blurIntensity: appearance.blurIntensity,
                width: Int(size.width.rounded()),
                height: Int(size.height.rounded())
            )
        }
        contentView.onAppearanceSettingsChange = { [weak self] transparency, blurIntensity in
            self?.updateAppearanceSettings(transparency: transparency, blurIntensity: blurIntensity)
        }
        contentView.onSizeSettingsChange = { [weak self] width, height in
            self?.updateSizeSettings(width: width, height: height)
        }
        self.panel = panel
        self.contentView = contentView
        return panel
    }

    private func updateAppearanceSettings(transparency: Int, blurIntensity: Int) {
        QuickTerminalAppearancePreferences.setTransparency(transparency)
        QuickTerminalAppearancePreferences.setBlurIntensity(blurIntensity)
        applyCurrentAppearance()
    }

    private func updateSizeSettings(width: Int, height: Int) {
        QuickTerminalSizePreferences.setWidth(width)
        QuickTerminalSizePreferences.setHeight(height)
        applyPreferredSize()
    }

    private func applyPreferredSize() {
        guard let panel, let screen = screenProvider() else { return }
        let frame = QuickTerminalGeometry.frame(
            screenFrame: screen.frame,
            visibleFrame: screen.visibleFrame,
            preferredSize: sizeProvider()
        )
        panel.setFrame(frame, display: true)
        contentView?.frame = NSRect(origin: .zero, size: frame.size)
        contentView?.setCollapsedCutoutRect(collapsedCutoutRect(screen: screen, panelFrame: frame))
    }

    private func updateShortcut(_ shortcut: QuickTerminalShortcut) -> String? {
        if case let .keyCombo(combo, _) = shortcut,
           let conflict = QuickTerminalShortcutConflictResolver.conflictMessage(for: combo)
        {
            return conflict
        }
        do {
            try shortcutService.updateShortcut(shortcut)
            contentView?.setShortcutLabel(shortcut.controlLabel)
            return nil
        } catch {
            return error.localizedDescription
        }
    }

    private func applyCurrentAppearance() {
        let appearance = appearanceProvider().resolvingReduceTransparency(reduceTransparencyProvider())
        contentView?.applyAppearance(appearance)
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
        guard presentation.targetIsVisible else { return }
        applyPreferredSize()
    }

    @objc
    private func handleGhosttyConfigurationDidChange() {
        guard !isTerminated else { return }
        session.reloadConfiguration()
        applyCurrentAppearance()
    }

    @objc
    private func handleAccessibilityDisplayOptionsDidChange() {
        guard !isTerminated else { return }
        applyCurrentAppearance()
    }
}

enum QuickTerminalFocusRestorationPolicy {
    static func shouldCapture(hasSnapshot: Bool, panelIsKey: Bool) -> Bool {
        !hasSnapshot || !panelIsKey
    }

    static func shouldRestore(requested: Bool, panelIsKey: Bool) -> Bool {
        requested && panelIsKey
    }
}

@MainActor
private final class QuickTerminalFocusSnapshot {
    private weak var window: NSWindow?
    private let application: NSRunningApplication?

    private init(window: NSWindow?, application: NSRunningApplication?) {
        self.window = window
        self.application = application
    }

    static func capture(excluding panel: NSPanel) -> QuickTerminalFocusSnapshot {
        let window = NSApp.keyWindow === panel ? nil : NSApp.keyWindow
        return QuickTerminalFocusSnapshot(
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
