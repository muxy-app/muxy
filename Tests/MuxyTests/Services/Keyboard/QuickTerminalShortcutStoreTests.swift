import Foundation
import Testing

@testable import Muxy

@Suite("QuickTerminalShortcutStore")
@MainActor
struct QuickTerminalShortcutStoreTests {
    @Test("loads the persisted shortcut")
    func loadsPersistedShortcut() {
        let shortcut = QuickTerminalShortcut.keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49)
        let persistence = InMemoryQuickTerminalShortcutPersistence(shortcut: shortcut)
        let store = makeStore(persistence: persistence)

        #expect(store.shortcut == shortcut)
    }

    @Test("normalizes a persisted display key to its registration identity")
    func normalizesPersistedDisplayKey() {
        let persisted = QuickTerminalShortcut.keyCombo(KeyCombo(key: "q", command: true), virtualKeyCode: 49)
        let expected = QuickTerminalShortcut.keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49)
        let persistence = InMemoryQuickTerminalShortcutPersistence(shortcut: persisted)
        let store = makeStore(persistence: persistence)

        #expect(store.shortcut == expected)
        #expect(persistence.savedShortcuts == [expected])
    }

    @Test("invalid persisted shortcut falls back to double Shift")
    func invalidPersistedShortcutFallsBack() {
        let shortcut = QuickTerminalShortcut.keyCombo(KeyCombo(key: "space", modifiers: 0), virtualKeyCode: 49)
        let persistence = InMemoryQuickTerminalShortcutPersistence(shortcut: shortcut)
        let store = makeStore(persistence: persistence)

        #expect(store.shortcut == .doubleShift)
    }

    @Test("update persists and runs the change handler")
    func updatePersistsAndNotifies() throws {
        let persistence = InMemoryQuickTerminalShortcutPersistence()
        var syncCount = 0
        let store = makeStore(persistence: persistence) { syncCount += 1 }
        let shortcut = QuickTerminalShortcut.keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49)
        var changedShortcut: QuickTerminalShortcut?
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
        let persistence = InMemoryQuickTerminalShortcutPersistence()
        let store = makeStore(persistence: persistence)
        let replacement = QuickTerminalShortcut.keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49)
        store.setChangeHandler { _, _ in throw QuickTerminalShortcutTestError.registrationFailed }

        #expect(throws: QuickTerminalShortcutTestError.registrationFailed) {
            try store.updateShortcut(replacement)
        }
        #expect(store.shortcut == .doubleShift)
        #expect(persistence.savedShortcuts.isEmpty)
    }

    @Test("invalid update is rejected before persistence")
    func invalidUpdateIsRejected() {
        let persistence = InMemoryQuickTerminalShortcutPersistence()
        let store = makeStore(persistence: persistence)

        #expect(throws: QuickTerminalShortcutError.invalidShortcut) {
            try store.updateShortcut(.keyCombo(KeyCombo(key: "space", modifiers: 0), virtualKeyCode: 49))
        }
        #expect(persistence.savedShortcuts.isEmpty)
    }

    @Test("persistence failure leaves in-memory state unchanged")
    func persistenceFailureLeavesStateUnchanged() {
        let persistence = InMemoryQuickTerminalShortcutPersistence()
        persistence.saveError = QuickTerminalShortcutTestError.persistenceFailed
        let store = makeStore(persistence: persistence)

        #expect(throws: QuickTerminalShortcutTestError.persistenceFailed) {
            try store.updateShortcut(.keyCombo(
                KeyCombo(key: "space", command: true),
                virtualKeyCode: 49
            ))
        }
        #expect(store.shortcut == .doubleShift)
    }

    private func makeStore(
        persistence: InMemoryQuickTerminalShortcutPersistence,
        settingsSynchronizer: @escaping @MainActor () -> Void = {}
    ) -> QuickTerminalShortcutStore {
        QuickTerminalShortcutStore(
            persistence: persistence,
            settingsSynchronizer: settingsSynchronizer
        )
    }
}

private enum QuickTerminalShortcutTestError: Error {
    case registrationFailed
    case persistenceFailed
}

private final class InMemoryQuickTerminalShortcutPersistence: QuickTerminalShortcutPersisting {
    var shortcut: QuickTerminalShortcut
    var savedShortcuts: [QuickTerminalShortcut] = []
    var saveError: QuickTerminalShortcutTestError?

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
        savedShortcuts.append(shortcut)
        self.shortcut = shortcut
    }
}
