import AppKit
import Testing

@testable import Muxy

@MainActor
@Suite("ShortcutContext")
struct ShortcutContextTests {
    private func mainWindow() -> NSWindow {
        let window = NSWindow()
        window.identifier = ShortcutContext.mainWindowIdentifier
        return window
    }

    @Test("non-main window only exposes the global scope")
    func nonMainWindowScopes() {
        let scopes = ShortcutContext.activeScopes(for: NSWindow(), isTerminalFocused: true)
        #expect(scopes == [.global])
    }

    @Test("main window without a focused terminal omits the terminal scope")
    func mainWindowWithoutTerminal() {
        let scopes = ShortcutContext.activeScopes(for: mainWindow(), isTerminalFocused: false)
        #expect(scopes == [.global, .mainWindow])
    }

    @Test("main window with a focused terminal includes the terminal scope")
    func mainWindowWithTerminal() {
        let scopes = ShortcutContext.activeScopes(for: mainWindow(), isTerminalFocused: true)
        #expect(scopes == [.global, .mainWindow, .terminal])
    }

    @Test("find action is gated to the terminal scope")
    func findActionScope() {
        #expect(ShortcutAction.findInTerminal.scope == .terminal)
    }
}
