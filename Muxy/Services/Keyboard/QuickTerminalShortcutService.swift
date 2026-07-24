import Observation

enum QuickTerminalShortcutMonitoringState: Equatable {
    case stopped
    case localOnly
    case systemWide
    case carbonHotKey
}

@MainActor
protocol QuickTerminalShortcutBackend: AnyObject {
    var monitoringState: QuickTerminalShortcutMonitoringState { get }
    func start(trigger: @escaping @MainActor () -> Void) throws
    func stop()
    func enableSystemWideMonitoringIfAuthorized() -> Bool
}

extension QuickTerminalShortcutBackend {
    func enableSystemWideMonitoringIfAuthorized() -> Bool {
        false
    }
}

@MainActor
@Observable
final class QuickTerminalShortcutService {
    typealias DoubleShiftBackendFactory = @MainActor () -> any QuickTerminalShortcutBackend
    typealias CarbonHotKeyBackendFactory = @MainActor (KeyCombo, UInt16) -> any QuickTerminalShortcutBackend
    typealias InputMonitoringAccessRequester = @MainActor () -> Bool

    static let shared = QuickTerminalShortcutService()

    let store: QuickTerminalShortcutStore
    private(set) var monitoringState = QuickTerminalShortcutMonitoringState.stopped
    private(set) var errorMessage: String?
    @ObservationIgnored var onTrigger: (@MainActor () -> Void)?
    @ObservationIgnored private let doubleShiftBackendFactory: DoubleShiftBackendFactory
    @ObservationIgnored private let carbonHotKeyBackendFactory: CarbonHotKeyBackendFactory
    @ObservationIgnored private let inputMonitoringAccessRequester: InputMonitoringAccessRequester
    @ObservationIgnored private var activeBackend: (any QuickTerminalShortcutBackend)?
    @ObservationIgnored private var isMonitoringRequested = false
    @ObservationIgnored private var registrationGeneration: UInt64 = 0
    @ObservationIgnored private var activeRegistrationGeneration: UInt64?

    init(
        store: QuickTerminalShortcutStore = .shared,
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

    var shortcut: QuickTerminalShortcut {
        store.shortcut
    }

    var needsInputMonitoringAccess: Bool {
        shortcut == .doubleShift && monitoringState == .localOnly
    }

    func start() throws {
        isMonitoringRequested = true
        guard activeBackend == nil else { return }
        guard let backend = makeBackend(for: store.shortcut) else {
            monitoringState = .stopped
            errorMessage = nil
            return
        }
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
        isMonitoringRequested = false
        activeRegistrationGeneration = nil
        activeBackend?.stop()
        activeBackend = nil
        monitoringState = .stopped
        errorMessage = nil
    }

    func updateShortcut(_ shortcut: QuickTerminalShortcut) throws {
        guard let shortcut = store.canonicalized(shortcut) else {
            errorMessage = QuickTerminalShortcutError.invalidShortcut.localizedDescription
            throw QuickTerminalShortcutError.invalidShortcut
        }
        if shortcut == store.shortcut, activeBackend == nil, isMonitoringRequested {
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

    func refreshKeyboardLayout() throws {
        let currentShortcut = shortcut
        guard let refreshedShortcut = store.canonicalized(currentShortcut),
              refreshedShortcut != currentShortcut
        else { return }
        try updateShortcut(refreshedShortcut)
    }

    @discardableResult
    func requestInputMonitoringAccess() -> Bool {
        guard isMonitoringRequested, shortcut == .doubleShift else { return false }
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
        with shortcut: QuickTerminalShortcut,
        persistenceCommit: QuickTerminalShortcutStore.PersistenceCommit
    ) throws {
        guard isMonitoringRequested else {
            try persistenceCommit()
            errorMessage = nil
            return
        }
        if activeBackend != nil,
           shortcut.hasSameRegistrationIdentity(as: store.shortcut)
        {
            try persistenceCommit()
            return
        }
        guard let replacementBackend = makeBackend(for: shortcut) else {
            try persistenceCommit()
            activeRegistrationGeneration = nil
            activeBackend?.stop()
            activeBackend = nil
            monitoringState = .stopped
            return
        }
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

    private func makeBackend(for shortcut: QuickTerminalShortcut) -> (any QuickTerminalShortcutBackend)? {
        switch shortcut {
        case .unassigned:
            nil
        case .doubleShift:
            doubleShiftBackendFactory()
        case let .keyCombo(combo, virtualKeyCode):
            carbonHotKeyBackendFactory(combo, virtualKeyCode)
        }
    }

    private func start(_ backend: any QuickTerminalShortcutBackend) throws -> UInt64 {
        registrationGeneration &+= 1
        let generation = registrationGeneration
        try backend.start { [weak self] in
            guard self?.activeRegistrationGeneration == generation else { return }
            self?.onTrigger?()
        }
        return generation
    }
}
