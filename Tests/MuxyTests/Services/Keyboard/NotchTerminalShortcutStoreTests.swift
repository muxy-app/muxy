import Foundation
import Testing

@testable import Muxy

@Suite("NotchTerminalShortcutStore")
@MainActor
struct NotchTerminalShortcutStoreTests {
    @Test("loads the persisted shortcut")
    func loadsPersistedShortcut() {
        let shortcut = NotchTerminalShortcut.keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49)
        let persistence = InMemoryNotchTerminalShortcutPersistence(shortcut: shortcut)
        let store = makeStore(persistence: persistence)

        #expect(store.shortcut == shortcut)
    }

    @Test("normalizes a persisted display key to its registration identity")
    func normalizesPersistedDisplayKey() {
        let persisted = NotchTerminalShortcut.keyCombo(KeyCombo(key: "q", command: true), virtualKeyCode: 49)
        let expected = NotchTerminalShortcut.keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49)
        let persistence = InMemoryNotchTerminalShortcutPersistence(shortcut: persisted)
        let store = makeStore(persistence: persistence)

        #expect(store.shortcut == expected)
        #expect(persistence.savedShortcuts == [expected])
    }

    @Test("invalid persisted shortcut falls back to double Shift")
    func invalidPersistedShortcutFallsBack() {
        let shortcut = NotchTerminalShortcut.keyCombo(KeyCombo(key: "space", modifiers: 0), virtualKeyCode: 49)
        let persistence = InMemoryNotchTerminalShortcutPersistence(shortcut: shortcut)
        let store = makeStore(persistence: persistence)

        #expect(store.shortcut == .doubleShift)
    }

    @Test("update persists and runs the change handler")
    func updatePersistsAndNotifies() throws {
        let persistence = InMemoryNotchTerminalShortcutPersistence()
        var syncCount = 0
        let store = makeStore(persistence: persistence) { syncCount += 1 }
        let shortcut = NotchTerminalShortcut.keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49)
        var changedShortcut: NotchTerminalShortcut?
        store.setChangeHandler { shortcut, persistenceCommit in
            changedShortcut = shortcut
            try persistenceCommit()
        }

        try store.updateShortcut(shortcut)

        #expect(store.shortcut == shortcut)
        #expect(persistence.savedShortcuts == [shortcut])
        #expect(changedShortcut == shortcut)
        #expect(syncCount == 1)
    }

    @Test("failed change never persists the rejected shortcut")
    func failedChangeNeverPersists() {
        let persistence = InMemoryNotchTerminalShortcutPersistence()
        let store = makeStore(persistence: persistence)
        let replacement = NotchTerminalShortcut.keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49)
        store.setChangeHandler { _, _ in throw NotchTerminalShortcutTestError.registrationFailed }

        #expect(throws: NotchTerminalShortcutTestError.registrationFailed) {
            try store.updateShortcut(replacement)
        }
        #expect(store.shortcut == .doubleShift)
        #expect(persistence.savedShortcuts.isEmpty)
    }

    @Test("invalid update is rejected before persistence")
    func invalidUpdateIsRejected() {
        let persistence = InMemoryNotchTerminalShortcutPersistence()
        let store = makeStore(persistence: persistence)

        #expect(throws: NotchTerminalShortcutError.invalidShortcut) {
            try store.updateShortcut(.keyCombo(KeyCombo(key: "space", modifiers: 0), virtualKeyCode: 49))
        }
        #expect(persistence.savedShortcuts.isEmpty)
    }

    @Test("persistence failure leaves in-memory state unchanged")
    func persistenceFailureLeavesStateUnchanged() {
        let persistence = InMemoryNotchTerminalShortcutPersistence()
        persistence.saveError = NotchTerminalShortcutTestError.persistenceFailed
        let store = makeStore(persistence: persistence)

        #expect(throws: NotchTerminalShortcutTestError.persistenceFailed) {
            try store.updateShortcut(.keyCombo(
                KeyCombo(key: "space", command: true),
                virtualKeyCode: 49
            ))
        }
        #expect(store.shortcut == .doubleShift)
    }

    private func makeStore(
        persistence: InMemoryNotchTerminalShortcutPersistence,
        settingsSynchronizer: @escaping @MainActor () -> Void = {}
    ) -> NotchTerminalShortcutStore {
        NotchTerminalShortcutStore(
            persistence: persistence,
            settingsSynchronizer: settingsSynchronizer
        )
    }
}

private enum NotchTerminalShortcutTestError: Error {
    case registrationFailed
    case persistenceFailed
}

private final class InMemoryNotchTerminalShortcutPersistence: NotchTerminalShortcutPersisting {
    var shortcut: NotchTerminalShortcut
    var savedShortcuts: [NotchTerminalShortcut] = []
    var saveError: NotchTerminalShortcutTestError?

    init(shortcut: NotchTerminalShortcut = .default) {
        self.shortcut = shortcut
    }

    func loadShortcut() throws -> NotchTerminalShortcut {
        shortcut
    }

    func saveShortcut(_ shortcut: NotchTerminalShortcut) throws {
        if let saveError {
            throw saveError
        }
        savedShortcuts.append(shortcut)
        self.shortcut = shortcut
    }
}
