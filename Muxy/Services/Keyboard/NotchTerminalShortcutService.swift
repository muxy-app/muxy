import Observation

enum NotchTerminalShortcutMonitoringState: Equatable {
    case stopped
    case localOnly
    case systemWide
    case carbonHotKey
}

@MainActor
protocol NotchTerminalShortcutBackend: AnyObject {
    var monitoringState: NotchTerminalShortcutMonitoringState { get }
    func start(trigger: @escaping @MainActor () -> Void) throws
    func stop()
    func enableSystemWideMonitoringIfAuthorized() -> Bool
}

extension NotchTerminalShortcutBackend {
    func enableSystemWideMonitoringIfAuthorized() -> Bool {
        false
    }
}

@MainActor
@Observable
final class NotchTerminalShortcutService {
    typealias DoubleShiftBackendFactory = @MainActor () -> any NotchTerminalShortcutBackend
    typealias CarbonHotKeyBackendFactory = @MainActor (KeyCombo, UInt16) -> any NotchTerminalShortcutBackend
    typealias InputMonitoringAccessRequester = @MainActor () -> Bool

    static let shared = NotchTerminalShortcutService()

    let store: NotchTerminalShortcutStore
    private(set) var monitoringState = NotchTerminalShortcutMonitoringState.stopped
    private(set) var errorMessage: String?
    @ObservationIgnored var onTrigger: (@MainActor () -> Void)?
    @ObservationIgnored private let doubleShiftBackendFactory: DoubleShiftBackendFactory
    @ObservationIgnored private let carbonHotKeyBackendFactory: CarbonHotKeyBackendFactory
    @ObservationIgnored private let inputMonitoringAccessRequester: InputMonitoringAccessRequester
    @ObservationIgnored private var activeBackend: (any NotchTerminalShortcutBackend)?
    @ObservationIgnored private var registrationGeneration: UInt64 = 0
    @ObservationIgnored private var activeRegistrationGeneration: UInt64?

    init(
        store: NotchTerminalShortcutStore = .shared,
        doubleShiftBackendFactory: @escaping DoubleShiftBackendFactory = { DoubleShiftShortcutBackend() },
        carbonHotKeyBackendFactory: @escaping CarbonHotKeyBackendFactory = {
            CarbonHotKeyBackend(combo: $0, virtualKeyCode: $1)
        },
        inputMonitoringAccessRequester: @escaping InputMonitoringAccessRequester = {
            DoubleShiftShortcutBackend.requestInputMonitoringAccess()
        }
    ) {
        self.store = store
        self.doubleShiftBackendFactory = doubleShiftBackendFactory
        self.carbonHotKeyBackendFactory = carbonHotKeyBackendFactory
        self.inputMonitoringAccessRequester = inputMonitoringAccessRequester
        store.setChangeHandler { [weak self] shortcut, persistenceCommit in
            guard let self else {
                try persistenceCommit()
                return
            }
            try self.replaceRegistration(
                with: shortcut,
                persistenceCommit: persistenceCommit
            )
        }
    }

    deinit {
        MainActor.assumeIsolated {
            activeBackend?.stop()
        }
    }

    var shortcut: NotchTerminalShortcut {
        store.shortcut
    }

    var needsInputMonitoringAccess: Bool {
        shortcut == .doubleShift && monitoringState == .localOnly
    }

    func start() throws {
        guard activeBackend == nil else { return }
        let backend = makeBackend(for: store.shortcut)
        do {
            let generation = try start(backend)
            activeRegistrationGeneration = generation
        } catch {
            errorMessage = error.localizedDescription
            throw error
        }
        activeBackend = backend
        monitoringState = backend.monitoringState
        errorMessage = nil
    }

    func stop() {
        activeRegistrationGeneration = nil
        activeBackend?.stop()
        activeBackend = nil
        monitoringState = .stopped
    }

    func updateShortcut(_ shortcut: NotchTerminalShortcut) throws {
        if shortcut == store.shortcut, activeBackend == nil {
            try start()
            return
        }
        do {
            try store.updateShortcut(shortcut)
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
            throw error
        }
    }

    func resetShortcut() throws {
        try updateShortcut(.default)
    }

    @discardableResult
    func requestInputMonitoringAccess() -> Bool {
        guard shortcut == .doubleShift else { return false }
        let granted = inputMonitoringAccessRequester()
        guard granted, let activeBackend else { return granted }
        let enabled = activeBackend.enableSystemWideMonitoringIfAuthorized()
        monitoringState = activeBackend.monitoringState
        if enabled {
            errorMessage = nil
        }
        return enabled
    }

    @discardableResult
    func refreshInputMonitoringAccess() -> Bool {
        guard shortcut == .doubleShift, let activeBackend else { return false }
        let enabled = activeBackend.enableSystemWideMonitoringIfAuthorized()
        monitoringState = activeBackend.monitoringState
        if enabled {
            errorMessage = nil
        }
        return enabled
    }

    private func replaceRegistration(
        with shortcut: NotchTerminalShortcut,
        persistenceCommit: NotchTerminalShortcutStore.PersistenceCommit
    ) throws {
        let replacementBackend = makeBackend(for: shortcut)
        let replacementGeneration = try start(replacementBackend)
        do {
            try persistenceCommit()
        } catch {
            replacementBackend.stop()
            throw error
        }
        guard let previousBackend = activeBackend else {
            activeBackend = replacementBackend
            activeRegistrationGeneration = replacementGeneration
            monitoringState = replacementBackend.monitoringState
            return
        }
        previousBackend.stop()
        activeBackend = replacementBackend
        activeRegistrationGeneration = replacementGeneration
        monitoringState = replacementBackend.monitoringState
    }

    private func makeBackend(for shortcut: NotchTerminalShortcut) -> any NotchTerminalShortcutBackend {
        switch shortcut {
        case .doubleShift:
            doubleShiftBackendFactory()
        case let .keyCombo(combo, virtualKeyCode):
            carbonHotKeyBackendFactory(combo, virtualKeyCode)
        }
    }

    private func start(_ backend: any NotchTerminalShortcutBackend) throws -> UInt64 {
        registrationGeneration &+= 1
        let generation = registrationGeneration
        try backend.start { [weak self] in
            guard self?.activeRegistrationGeneration == generation else { return }
            self?.onTrigger?()
        }
        return generation
    }
}
