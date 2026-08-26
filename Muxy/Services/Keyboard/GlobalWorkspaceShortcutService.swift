import Foundation
import Observation

@MainActor
@Observable
final class GlobalWorkspaceShortcutService {
    typealias DoubleModifierBackendFactory = @MainActor (
        DoubleModifierShortcutBackend.Modifier,
        TimeInterval
    ) -> any QuickTerminalShortcutBackend
    typealias CarbonHotKeyBackendFactory = @MainActor (KeyCombo, UInt16) -> any QuickTerminalShortcutBackend

    struct Configuration: Equatable {
        let isEnabled: Bool
        let trigger: GlobalWorkspaceTrigger
        let doubleTapInterval: TimeInterval
        let toggleToHide: Bool
        let customShortcut: QuickTerminalShortcut?

        static func current(defaults: UserDefaults = .standard) -> Configuration {
            Configuration(
                isEnabled: GlobalWorkspacePreferences.isEnabled(defaults: defaults),
                trigger: GlobalWorkspacePreferences.trigger(defaults: defaults),
                doubleTapInterval: GlobalWorkspacePreferences.doubleTapInterval(defaults: defaults),
                toggleToHide: GlobalWorkspacePreferences.toggleToHide(defaults: defaults),
                customShortcut: GlobalWorkspacePreferences.customShortcut(defaults: defaults)
            )
        }
    }

    static let shared = GlobalWorkspaceShortcutService()

    private(set) var monitoringState = QuickTerminalShortcutMonitoringState.stopped
    private(set) var configuration: Configuration
    private(set) var errorMessage: String?
    @ObservationIgnored var onTrigger: (@MainActor (_ toggleToHide: Bool) -> Void)?
    @ObservationIgnored private let doubleModifierBackendFactory: DoubleModifierBackendFactory
    @ObservationIgnored private let carbonHotKeyBackendFactory: CarbonHotKeyBackendFactory
    @ObservationIgnored private let defaults: UserDefaults
    @ObservationIgnored private var backend: (any QuickTerminalShortcutBackend)?
    @ObservationIgnored private var defaultsObserver: NSObjectProtocol?
    @ObservationIgnored private var isStarted = false

    init(
        defaults: UserDefaults = .standard,
        doubleModifierBackendFactory: @escaping DoubleModifierBackendFactory = { modifier, interval in
            DoubleModifierShortcutBackend(
                modifier: modifier,
                maximumTapDuration: 0.3,
                maximumInterval: interval
            )
        },
        carbonHotKeyBackendFactory: @escaping CarbonHotKeyBackendFactory = {
            CarbonHotKeyBackend(combo: $0, virtualKeyCode: $1)
        }
    ) {
        self.defaults = defaults
        self.doubleModifierBackendFactory = doubleModifierBackendFactory
        self.carbonHotKeyBackendFactory = carbonHotKeyBackendFactory
        configuration = Configuration.current(defaults: defaults)
    }

    deinit {
        MainActor.assumeIsolated {
            stop()
        }
    }

    var needsInputMonitoringAccess: Bool {
        configuration.isEnabled && configuration.trigger.modifier != nil && monitoringState == .localOnly
    }

    func start() {
        guard !isStarted else { return }
        isStarted = true
        defaultsObserver = NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification,
            object: defaults,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.refreshConfiguration()
            }
        }
        apply(Configuration.current(defaults: defaults))
    }

    func refresh() {
        refreshConfiguration()
    }

    func stop() {
        if let defaultsObserver {
            NotificationCenter.default.removeObserver(defaultsObserver)
        }
        defaultsObserver = nil
        backend?.stop()
        backend = nil
        monitoringState = .stopped
        errorMessage = nil
        isStarted = false
    }

    @discardableResult
    func requestInputMonitoringAccess() -> Bool {
        guard configuration.isEnabled, configuration.trigger.modifier != nil else { return false }
        let granted = DoubleModifierShortcutBackend.requestInputMonitoringAccess()
        guard granted, let backend else { return granted }
        let enabled = backend.enableSystemWideMonitoringIfAuthorized()
        monitoringState = backend.monitoringState
        return enabled
    }

    @discardableResult
    func refreshInputMonitoringAccess() -> Bool {
        guard configuration.isEnabled, configuration.trigger.modifier != nil, let backend else { return false }
        let enabled = backend.enableSystemWideMonitoringIfAuthorized()
        monitoringState = backend.monitoringState
        return enabled
    }

    func updateCustomShortcut(_ shortcut: QuickTerminalShortcut) throws {
        try GlobalWorkspacePreferences.setCustomShortcut(shortcut, defaults: defaults)
        refreshConfiguration()
        if let errorMessage {
            throw GlobalWorkspaceShortcutUpdateError.registrationFailed(errorMessage)
        }
    }

    private func refreshConfiguration() {
        let next = Configuration.current(defaults: defaults)
        guard next != configuration else { return }
        apply(next)
    }

    private func apply(_ next: Configuration) {
        let registrationChanged = next.isEnabled != configuration.isEnabled
            || next.trigger != configuration.trigger
            || next.doubleTapInterval != configuration.doubleTapInterval
            || next.customShortcut != configuration.customShortcut
        configuration = next

        guard isStarted else { return }
        guard next.isEnabled else {
            backend?.stop()
            backend = nil
            monitoringState = .stopped
            errorMessage = nil
            return
        }
        guard registrationChanged || backend == nil else { return }

        backend?.stop()
        guard let replacement = makeBackend(for: next) else {
            backend = nil
            monitoringState = .stopped
            errorMessage = nil
            return
        }
        do {
            try replacement.start { [weak self] in
                guard let self else { return }
                self.onTrigger?(self.configuration.toggleToHide)
            }
            backend = replacement
            monitoringState = replacement.monitoringState
            errorMessage = nil
        } catch {
            replacement.stop()
            backend = nil
            monitoringState = .stopped
            errorMessage = error.localizedDescription
        }
    }

    private func makeBackend(for configuration: Configuration) -> (any QuickTerminalShortcutBackend)? {
        if let modifier = configuration.trigger.modifier {
            return doubleModifierBackendFactory(modifier, configuration.doubleTapInterval)
        }
        guard case let .keyCombo(combo, virtualKeyCode) = configuration.customShortcut else { return nil }
        return carbonHotKeyBackendFactory(combo, virtualKeyCode)
    }
}

enum GlobalWorkspaceShortcutUpdateError: LocalizedError {
    case registrationFailed(String)

    var errorDescription: String? {
        switch self {
        case let .registrationFailed(message): message
        }
    }
}
