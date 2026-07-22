import AppKit
import Carbon.HIToolbox
import Testing

@testable import Muxy

@Suite("QuickTerminalShortcutService")
@MainActor
struct QuickTerminalShortcutServiceTests {
    @Test("starts the stored backend and delivers triggers")
    func startAndTrigger() throws {
        let persistence = ServiceShortcutPersistence(shortcut: .doubleShift)
        let store = makeStore(persistence: persistence)
        let backend = TestShortcutBackend(state: .localOnly)
        let service = makeService(store: store, doubleShiftBackend: backend)
        var triggerCount = 0
        service.onTrigger = { triggerCount += 1 }

        try service.start()
        backend.sendTrigger()

        #expect(backend.startCount == 1)
        #expect(service.monitoringState == .localOnly)
        #expect(triggerCount == 1)
    }

    @Test("replacement starts before the previous registration stops")
    func replacementOrdering() throws {
        let recorder = ShortcutBackendRecorder()
        let store = makeStore(persistence: ServiceShortcutPersistence(shortcut: .doubleShift))
        let previous = TestShortcutBackend(name: "previous", state: .localOnly, recorder: recorder)
        let replacement = TestShortcutBackend(name: "replacement", state: .carbonHotKey, recorder: recorder)
        let service = makeService(
            store: store,
            doubleShiftBackend: previous,
            carbonHotKeyBackend: replacement
        )
        try service.start()

        try service.updateShortcut(.keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49))

        #expect(recorder.events == ["start previous", "start replacement", "stop previous"])
        #expect(service.shortcut == .keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49))
        #expect(service.monitoringState == .carbonHotKey)
    }

    @Test("failed replacement keeps the previous registration working")
    func failedReplacementKeepsPrevious() throws {
        let recorder = ShortcutBackendRecorder()
        let persistence = ServiceShortcutPersistence(shortcut: .doubleShift)
        let store = makeStore(persistence: persistence)
        let previous = TestShortcutBackend(name: "previous", state: .localOnly, recorder: recorder)
        let replacement = TestShortcutBackend(
            name: "replacement",
            state: .carbonHotKey,
            startError: .registrationFailed,
            recorder: recorder
        )
        let service = makeService(
            store: store,
            doubleShiftBackend: previous,
            carbonHotKeyBackend: replacement
        )
        try service.start()

        #expect(throws: ShortcutBackendTestError.registrationFailed) {
            try service.updateShortcut(.keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49))
        }

        #expect(previous.stopCount == 0)
        #expect(service.shortcut == .doubleShift)
        #expect(service.monitoringState == .localOnly)
        #expect(recorder.events == ["start previous", "start replacement"])
        #expect(persistence.shortcut == .doubleShift)
        #expect(service.errorMessage == ShortcutBackendTestError.registrationFailed.localizedDescription)
    }

    @Test("persistence failure stops the replacement and keeps the previous registration")
    func persistenceFailureKeepsPrevious() throws {
        let persistence = ServiceShortcutPersistence(shortcut: .doubleShift)
        let store = makeStore(persistence: persistence)
        let previous = TestShortcutBackend(state: .localOnly)
        let replacement = TestShortcutBackend(state: .carbonHotKey)
        let service = makeService(
            store: store,
            doubleShiftBackend: previous,
            carbonHotKeyBackend: replacement
        )
        try service.start()
        persistence.saveError = .persistenceFailed

        #expect(throws: ShortcutBackendTestError.persistenceFailed) {
            try service.updateShortcut(.keyCombo(
                KeyCombo(key: "space", command: true),
                virtualKeyCode: 49
            ))
        }

        #expect(previous.stopCount == 0)
        #expect(replacement.stopCount == 1)
        #expect(service.shortcut == .doubleShift)
        #expect(service.monitoringState == .localOnly)
    }

    @Test("changing shortcut recovers after initial registration fails")
    func changeRecoversAfterInitialFailure() throws {
        let initialShortcut = QuickTerminalShortcut.keyCombo(
            KeyCombo(key: "space", command: true),
            virtualKeyCode: 49
        )
        let store = makeStore(persistence: ServiceShortcutPersistence(shortcut: initialShortcut))
        let doubleShift = TestShortcutBackend(state: .localOnly)
        let failingCarbon = TestShortcutBackend(
            state: .carbonHotKey,
            startError: .registrationFailed
        )
        let service = makeService(
            store: store,
            doubleShiftBackend: doubleShift,
            carbonHotKeyBackend: failingCarbon
        )

        #expect(throws: ShortcutBackendTestError.registrationFailed) {
            try service.start()
        }
        try service.updateShortcut(.doubleShift)

        #expect(doubleShift.startCount == 1)
        #expect(service.shortcut == .doubleShift)
        #expect(service.monitoringState == .localOnly)
        #expect(service.errorMessage == nil)
    }

    @Test("selecting the stored shortcut retries an inactive registration")
    func sameShortcutRetriesInactiveRegistration() throws {
        let shortcut = QuickTerminalShortcut.keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49)
        let store = makeStore(persistence: ServiceShortcutPersistence(shortcut: shortcut))
        let failingCarbon = TestShortcutBackend(
            state: .carbonHotKey,
            startError: .registrationFailed
        )
        let workingCarbon = TestShortcutBackend(state: .carbonHotKey)
        var carbonBackends = [failingCarbon, workingCarbon]
        let service = QuickTerminalShortcutService(
            store: store,
            doubleShiftBackendFactory: { TestShortcutBackend(state: .localOnly) },
            carbonHotKeyBackendFactory: { _, _ in carbonBackends.removeFirst() },
            inputMonitoringAccessRequester: { false }
        )

        #expect(throws: ShortcutBackendTestError.registrationFailed) {
            try service.start()
        }
        try service.updateShortcut(shortcut)

        #expect(workingCarbon.startCount == 1)
        #expect(service.monitoringState == .carbonHotKey)
        #expect(service.errorMessage == nil)
    }

    @Test("explicit permission request upgrades double Shift monitoring")
    func explicitPermissionRequestUpgradesMonitoring() throws {
        let store = makeStore(persistence: ServiceShortcutPersistence(shortcut: .doubleShift))
        let backend = TestShortcutBackend(state: .localOnly, enabledState: .systemWide)
        var requestCount = 0
        let service = makeService(
            store: store,
            doubleShiftBackend: backend,
            requestAccess: {
                requestCount += 1
                return true
            }
        )
        try service.start()

        let granted = service.requestInputMonitoringAccess()

        #expect(granted)
        #expect(requestCount == 1)
        #expect(backend.enableCount == 1)
        #expect(service.monitoringState == .systemWide)
    }

    @Test("permission refresh never requests access")
    func permissionRefreshDoesNotRequestAccess() throws {
        let store = makeStore(persistence: ServiceShortcutPersistence(shortcut: .doubleShift))
        let backend = TestShortcutBackend(state: .localOnly, enabledState: .systemWide)
        var requestCount = 0
        let service = makeService(
            store: store,
            doubleShiftBackend: backend,
            requestAccess: {
                requestCount += 1
                return true
            }
        )
        try service.start()

        let enabled = service.refreshInputMonitoringAccess()

        #expect(enabled)
        #expect(requestCount == 0)
        #expect(service.monitoringState == .systemWide)
    }

    @Test("Carbon modifier mapping covers supported modifiers")
    func carbonModifierMapping() {
        let flags: NSEvent.ModifierFlags = [.command, .shift, .control, .option]
        let expected = UInt32(cmdKey | shiftKey | controlKey | optionKey)

        #expect(CarbonHotKeyBackend.carbonModifiers(for: flags) == expected)
    }

    @Test("service forwards the recorded virtual key code to Carbon")
    func recordedVirtualKeyCodeIsForwarded() throws {
        let key = try #require(KeyCombo.key(forVirtualKeyCode: 0))
        let shortcut = QuickTerminalShortcut.keyCombo(
            KeyCombo(key: key, command: true),
            virtualKeyCode: 0
        )
        let store = makeStore(persistence: ServiceShortcutPersistence(shortcut: shortcut))
        let backend = TestShortcutBackend(state: .carbonHotKey)
        var receivedCombo: KeyCombo?
        var receivedKeyCode: UInt16?
        let service = QuickTerminalShortcutService(
            store: store,
            doubleShiftBackendFactory: { TestShortcutBackend(state: .localOnly) },
            carbonHotKeyBackendFactory: { combo, virtualKeyCode in
                receivedCombo = combo
                receivedKeyCode = virtualKeyCode
                return backend
            },
            inputMonitoringAccessRequester: { false }
        )

        try service.start()

        #expect(receivedCombo == KeyCombo(key: key, command: true))
        #expect(receivedKeyCode == 0)
    }

    @Test("keyboard layout refresh preserves an unchanged Carbon registration")
    func keyboardLayoutRefreshPreservesRegistration() throws {
        var resolvedKey = "a"
        let shortcut = QuickTerminalShortcut.keyCombo(
            KeyCombo(key: resolvedKey, command: true),
            virtualKeyCode: 0
        )
        let persistence = ServiceShortcutPersistence(shortcut: shortcut)
        let store = QuickTerminalShortcutStore(
            persistence: persistence,
            settingsSynchronizer: {},
            canonicalizer: {
                $0.canonicalized(keyResolver: { _ in resolvedKey })
            }
        )
        let backend = TestShortcutBackend(state: .carbonHotKey)
        var backendCreationCount = 0
        let service = QuickTerminalShortcutService(
            store: store,
            doubleShiftBackendFactory: { TestShortcutBackend(state: .localOnly) },
            carbonHotKeyBackendFactory: { _, _ in
                backendCreationCount += 1
                return backend
            },
            inputMonitoringAccessRequester: { false }
        )
        try service.start()

        resolvedKey = "q"
        try service.refreshKeyboardLayout()

        let expected = QuickTerminalShortcut.keyCombo(
            KeyCombo(key: "q", command: true),
            virtualKeyCode: 0
        )
        #expect(backendCreationCount == 1)
        #expect(backend.startCount == 1)
        #expect(backend.stopCount == 0)
        #expect(service.shortcut == expected)
        #expect(persistence.shortcut == expected)
    }

    @Test("shortcut changes while stopped persist without restarting monitoring")
    func stoppedShortcutChangesWaitForStart() throws {
        let persistence = ServiceShortcutPersistence(shortcut: .doubleShift)
        let store = makeStore(persistence: persistence)
        let doubleShift = TestShortcutBackend(state: .localOnly)
        let carbon = TestShortcutBackend(state: .carbonHotKey)
        let service = makeService(
            store: store,
            doubleShiftBackend: doubleShift,
            carbonHotKeyBackend: carbon
        )
        let replacement = QuickTerminalShortcut.keyCombo(
            KeyCombo(key: "space", command: true),
            virtualKeyCode: 49
        )
        try service.start()
        service.stop()

        try service.updateShortcut(.doubleShift)
        try service.updateShortcut(replacement)

        #expect(doubleShift.startCount == 1)
        #expect(doubleShift.stopCount == 1)
        #expect(carbon.startCount == 0)
        #expect(service.monitoringState == .stopped)
        #expect(persistence.shortcut == replacement)

        try service.start()

        #expect(carbon.startCount == 1)
        #expect(service.monitoringState == .carbonHotKey)
    }

    @Test("stopped monitoring never requests Input Monitoring access")
    func stoppedMonitoringDoesNotRequestAccess() throws {
        let store = makeStore(persistence: ServiceShortcutPersistence(shortcut: .doubleShift))
        let backend = TestShortcutBackend(state: .localOnly)
        var requestCount = 0
        let service = makeService(
            store: store,
            doubleShiftBackend: backend,
            requestAccess: {
                requestCount += 1
                return true
            }
        )
        try service.start()
        service.stop()

        #expect(!service.requestInputMonitoringAccess())
        #expect(requestCount == 0)
    }

    @Test("service deinitialization stops its active backend")
    func serviceDeinitializationStopsBackend() throws {
        let store = makeStore(persistence: ServiceShortcutPersistence(shortcut: .doubleShift))
        let backend = TestShortcutBackend(state: .localOnly)
        var service: QuickTerminalShortcutService? = makeService(
            store: store,
            doubleShiftBackend: backend
        )
        try service?.start()

        service = nil

        #expect(backend.stopCount == 1)
    }

    @Test("unassigned shortcut starts without installing a backend")
    func unassignedStartsWithoutBackend() throws {
        let store = makeStore(persistence: ServiceShortcutPersistence())
        let doubleShift = TestShortcutBackend(state: .localOnly)
        let carbon = TestShortcutBackend(state: .carbonHotKey)
        let service = makeService(
            store: store,
            doubleShiftBackend: doubleShift,
            carbonHotKeyBackend: carbon
        )

        try service.start()

        #expect(service.shortcut == .unassigned)
        #expect(service.monitoringState == .stopped)
        #expect(doubleShift.startCount == 0)
        #expect(carbon.startCount == 0)
    }

    @Test("assigning a shortcut starts monitoring from an unassigned state")
    func assignFromUnassignedStartsBackend() throws {
        let persistence = ServiceShortcutPersistence()
        let store = makeStore(persistence: persistence)
        let backend = TestShortcutBackend(state: .localOnly)
        let service = makeService(store: store, doubleShiftBackend: backend)
        try service.start()

        try service.updateShortcut(.doubleShift)

        #expect(service.shortcut == .doubleShift)
        #expect(service.monitoringState == .localOnly)
        #expect(backend.startCount == 1)
        #expect(persistence.shortcut == .doubleShift)
    }

    @Test("unassigning persists before stopping the active backend")
    func unassignStopsActiveBackend() throws {
        let recorder = ShortcutBackendRecorder()
        let persistence = ServiceShortcutPersistence(shortcut: .doubleShift)
        let store = makeStore(persistence: persistence)
        let backend = TestShortcutBackend(name: "double shift", state: .localOnly, recorder: recorder)
        let service = makeService(store: store, doubleShiftBackend: backend)
        try service.start()

        try service.updateShortcut(.unassigned)

        #expect(service.shortcut == .unassigned)
        #expect(service.monitoringState == .stopped)
        #expect(persistence.shortcut == .unassigned)
        #expect(recorder.events == ["start double shift", "stop double shift"])
    }

    @Test("failed unassignment persistence keeps the active backend")
    func failedUnassignmentKeepsActiveBackend() throws {
        let persistence = ServiceShortcutPersistence(shortcut: .doubleShift)
        let store = makeStore(persistence: persistence)
        let backend = TestShortcutBackend(state: .localOnly)
        let service = makeService(store: store, doubleShiftBackend: backend)
        try service.start()
        persistence.saveError = .persistenceFailed

        #expect(throws: ShortcutBackendTestError.persistenceFailed) {
            try service.updateShortcut(.unassigned)
        }

        #expect(service.shortcut == .doubleShift)
        #expect(service.monitoringState == .localOnly)
        #expect(backend.stopCount == 0)
    }

    private func makeStore(persistence: ServiceShortcutPersistence) -> QuickTerminalShortcutStore {
        QuickTerminalShortcutStore(persistence: persistence, settingsSynchronizer: {})
    }

    private func makeService(
        store: QuickTerminalShortcutStore,
        doubleShiftBackend: TestShortcutBackend,
        carbonHotKeyBackend: TestShortcutBackend = TestShortcutBackend(state: .carbonHotKey),
        requestAccess: @escaping @MainActor () -> Bool = { false }
    ) -> QuickTerminalShortcutService {
        QuickTerminalShortcutService(
            store: store,
            doubleShiftBackendFactory: { doubleShiftBackend },
            carbonHotKeyBackendFactory: { _, _ in carbonHotKeyBackend },
            inputMonitoringAccessRequester: requestAccess
        )
    }
}

private enum ShortcutBackendTestError: Error {
    case registrationFailed
    case persistenceFailed
}

@MainActor
private final class ShortcutBackendRecorder {
    var events: [String] = []
}

@MainActor
private final class TestShortcutBackend: QuickTerminalShortcutBackend {
    let name: String
    private(set) var monitoringState: QuickTerminalShortcutMonitoringState
    private let enabledState: QuickTerminalShortcutMonitoringState
    private let startError: ShortcutBackendTestError?
    private let recorder: ShortcutBackendRecorder?
    private var trigger: (@MainActor () -> Void)?
    private(set) var startCount = 0
    private(set) var stopCount = 0
    private(set) var enableCount = 0

    init(
        name: String = "backend",
        state: QuickTerminalShortcutMonitoringState,
        enabledState: QuickTerminalShortcutMonitoringState? = nil,
        startError: ShortcutBackendTestError? = nil,
        recorder: ShortcutBackendRecorder? = nil
    ) {
        self.name = name
        monitoringState = state
        self.enabledState = enabledState ?? state
        self.startError = startError
        self.recorder = recorder
    }

    func start(trigger: @escaping @MainActor () -> Void) throws {
        startCount += 1
        recorder?.events.append("start \(name)")
        if let startError {
            throw startError
        }
        self.trigger = trigger
    }

    func stop() {
        stopCount += 1
        recorder?.events.append("stop \(name)")
        trigger = nil
        monitoringState = .stopped
    }

    func enableSystemWideMonitoringIfAuthorized() -> Bool {
        enableCount += 1
        monitoringState = enabledState
        return monitoringState == .systemWide
    }

    func sendTrigger() {
        trigger?()
    }
}

private final class ServiceShortcutPersistence: QuickTerminalShortcutPersisting {
    var shortcut: QuickTerminalShortcut
    var saveError: ShortcutBackendTestError?

    init(shortcut: QuickTerminalShortcut = .default) {
        self.shortcut = shortcut
    }

    func loadShortcut() throws -> QuickTerminalShortcut {
        shortcut
    }

    func saveShortcut(_ shortcut: QuickTerminalShortcut) throws {
        if let saveError {
            throw saveError
        }
        self.shortcut = shortcut
    }
}
