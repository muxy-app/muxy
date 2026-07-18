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

    @Test("action resolves Cmd+Backtick to toggle extension console")
    func actionResolvesToggleExtensionConsole() throws {
        let store = KeyBindingStore(persistence: StubKeyBindingPersistence(bindings: KeyBinding.defaults))
        let event = try keyEvent(
            characters: "`",
            charactersIgnoringModifiers: "`",
            keyCode: 50,
            modifiers: [.command]
        )

        #expect(store.action(for: event, scopes: [.mainWindow]) == .toggleExtensionConsole)
    }

    @Test("action resolves inspect element only in browser scope")
    func actionResolvesInspectElementOnlyInBrowserScope() throws {
        let store = KeyBindingStore(persistence: StubKeyBindingPersistence(bindings: KeyBinding.defaults))
        let event = try keyEvent(
            characters: "i",
            charactersIgnoringModifiers: "i",
            keyCode: 34,
            modifiers: [.command, .option]
        )

        #expect(store.action(for: event, scopes: [.global, .mainWindow]) == nil)
        #expect(store.action(for: event, scopes: [.global, .mainWindow, .browser]) == .inspectElement)
    }

    @Test("action can be assigned and reset")
    func actionCanBeAssignedAndReset() {
        let persistence = StubKeyBindingPersistence(bindings: KeyBinding.defaults)
        let store = KeyBindingStore(persistence: persistence)
        let combo = KeyCombo(key: "u", command: true)

        #expect(store.combo(for: .refreshWorktrees) == KeyCombo(key: "r", command: true, option: true))

        store.updateBinding(action: .refreshWorktrees, combo: combo)

        #expect(store.combo(for: .refreshWorktrees) == combo)

        store.resetBinding(action: .refreshWorktrees)

        #expect(store.combo(for: .refreshWorktrees) == KeyCombo(key: "r", command: true, option: true))
    }

    @Test("action can be unassigned")
    func actionCanBeUnassigned() throws {
        let persistence = StubKeyBindingPersistence(bindings: KeyBinding.defaults)
        let store = KeyBindingStore(persistence: persistence)
        let combo = KeyCombo(key: "", modifiers: 0)
        let event = try keyEvent(
            characters: "r",
            charactersIgnoringModifiers: "r",
            keyCode: 15,
            modifiers: [.command, .option]
        )

        store.updateBinding(action: .refreshWorktrees, combo: combo)

        #expect(store.combo(for: .refreshWorktrees) == combo)
        #expect(store.action(for: event, scopes: [.mainWindow]) == nil)
    }

    @Test("saved custom bindings gain new default actions")
    func savedCustomBindingsGainNewDefaultActions() {
        let customOpenProject = KeyBinding(action: .openProject, combo: KeyCombo(key: "j", command: true, option: true))
        let persistence = StubKeyBindingPersistence(bindings: [customOpenProject])
        let store = KeyBindingStore(persistence: persistence)

        #expect(store.combo(for: .openProject) == customOpenProject.combo)
        #expect(store.combo(for: .refreshWorktrees) == KeyCombo(key: "r", command: true, option: true))
        #expect(store.combo(for: .recentlyRemovedProjects).isAssigned == false)
    }

    @Test("Recently Removed Projects can be assigned by the user")
    func recentlyRemovedProjectsCanBeAssigned() throws {
        let store = KeyBindingStore(persistence: StubKeyBindingPersistence(bindings: KeyBinding.defaults))
        let combo = KeyCombo(key: "u", command: true, shift: true, option: true)
        let keyCode = try #require(KeyCombo.keyCode(for: "u"))
        let event = try keyEvent(
            characters: "U",
            charactersIgnoringModifiers: "u",
            keyCode: keyCode,
            modifiers: [.command, .option, .shift]
        )

        #expect(store.action(for: event, scopes: [.mainWindow]) == nil)

        store.updateBinding(action: .recentlyRemovedProjects, combo: combo)

        #expect(store.action(for: event, scopes: [.mainWindow]) == .recentlyRemovedProjects)
    }

    @Test("new default shortcuts do not shadow saved custom bindings")
    func newDefaultShortcutsDoNotShadowSavedCustomBindings() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("keybindings-\(UUID().uuidString).json")
        let savedCombo = KeyCombo(key: "n", command: true, option: true)
        let savedBinding = KeyBinding(action: .terminalOmniboxCommands, combo: savedCombo)
        let data = try JSONEncoder().encode([savedBinding])
        try data.write(to: url, options: .atomic)
        defer { try? FileManager.default.removeItem(at: url) }
        let store = KeyBindingStore(persistence: FileKeyBindingPersistence(fileURL: url))
        let keyCode = try #require(KeyCombo.keyCode(for: "n"))
        let event = try keyEvent(
            characters: "n",
            charactersIgnoringModifiers: "n",
            keyCode: keyCode,
            modifiers: [.command, .option]
        )

        #expect(store.combo(for: .terminalOmniboxCommands) == savedCombo)
        #expect(store.combo(for: .createWorktree).isAssigned == false)
        #expect(store.action(for: event, scopes: [.mainWindow]) == .terminalOmniboxCommands)
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
