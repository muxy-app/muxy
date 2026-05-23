import AppKit
import Testing

@testable import Muxy

@Suite("KeyBindingStore")
@MainActor
struct KeyBindingStoreTests {
    @Test("action resolves by keyCode regardless of input layout characters")
    func actionResolvesFromKeyCode() throws {
        let persistence = StubKeyBindingPersistence(bindings: [
            KeyBinding(action: .newTab, combo: KeyCombo(key: "q", command: true))
        ])
        let store = KeyBindingStore(persistence: persistence)
        let event = try keyEvent(
            characters: "й",
            charactersIgnoringModifiers: "й",
            keyCode: 12,
            modifiers: [.command]
        )

        let action = store.action(for: event, scopes: [.mainWindow])

        #expect(action == .newTab)
    }

    @Test("action respects shortcut scope filtering")
    func actionRespectsScopeFiltering() throws {
        let store = KeyBindingStore(persistence: StubKeyBindingPersistence(bindings: KeyBinding.defaults))
        let event = try keyEvent(
            characters: "R",
            charactersIgnoringModifiers: "r",
            keyCode: 15,
            modifiers: [.command, .shift]
        )

        #expect(store.action(for: event, scopes: [.global]) == .reloadConfig)
        #expect(store.action(for: event, scopes: [.mainWindow]) == nil)
    }

    @Test("action can be assigned and reset")
    func actionCanBeAssignedAndReset() {
        let persistence = StubKeyBindingPersistence(bindings: KeyBinding.defaults)
        let store = KeyBindingStore(persistence: persistence)
        let combo = KeyCombo(key: "u", command: true)

        #expect(store.combo(for: .openVCSTab) == KeyCombo(key: "y", command: true))
        #expect(store.combo(for: .openDiffViewerTab) == KeyCombo(key: "y", command: true, shift: true))

        store.updateBinding(action: .openVCSTab, combo: combo)

        #expect(store.combo(for: .openVCSTab) == combo)

        store.resetBinding(action: .openVCSTab)

        #expect(store.combo(for: .openVCSTab) == KeyCombo(key: "y", command: true))
    }

    private func keyEvent(
        characters: String,
        charactersIgnoringModifiers: String,
        keyCode: UInt16,
        modifiers: NSEvent.ModifierFlags
    ) throws -> NSEvent {
        guard let event = NSEvent.keyEvent(
            with: .keyDown,
            location: .zero,
            modifierFlags: modifiers,
            timestamp: 0,
            windowNumber: 0,
            context: nil,
            characters: characters,
            charactersIgnoringModifiers: charactersIgnoringModifiers,
            isARepeat: false,
            keyCode: keyCode
        ) else {
            throw EventCreationError()
        }
        return event
    }

    private final class StubKeyBindingPersistence: KeyBindingPersisting {
        private let storedBindings: [KeyBinding]

        init(bindings: [KeyBinding]) {
            storedBindings = bindings
        }

        func loadBindings() throws -> [KeyBinding] {
            storedBindings
        }

        func saveBindings(_: [KeyBinding]) throws {}
    }

    private struct EventCreationError: Error {}
}
