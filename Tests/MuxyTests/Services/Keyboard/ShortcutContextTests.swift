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

    private func globalWorkspaceWindow() -> NSWindow {
        let window = NSWindow()
        window.identifier = ShortcutContext.globalWorkspaceWindowIdentifier
        return window
    }

    @Test("non-workspace window only exposes the global scope")
    func nonWorkspaceWindowScopes() {
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

    @Test("main window with a focused browser includes the browser scope")
    func mainWindowWithBrowser() {
        let scopes = ShortcutContext.activeScopes(
            for: mainWindow(),
            isTerminalFocused: false,
            isBrowserFocused: true
        )
        #expect(scopes == [.global, .mainWindow, .browser])
    }

    @Test("global workspace exposes main workspace shortcut scopes")
    func globalWorkspaceScopes() {
        let scopes = ShortcutContext.activeScopes(
            for: globalWorkspaceWindow(),
            isTerminalFocused: true,
            isBrowserFocused: true
        )
        #expect(scopes == [.global, .mainWindow, .terminal, .browser])
    }

    @Test("global workspace identity remains distinguishable")
    func globalWorkspaceIdentity() {
        let window = globalWorkspaceWindow()
        #expect(ShortcutContext.isMainWindow(window))
        #expect(ShortcutContext.isGlobalWorkspaceWindow(window))
        #expect(!ShortcutContext.isGlobalWorkspaceWindow(mainWindow()))
    }

    @Test("find action is gated to the terminal scope")
    func findActionScope() {
        #expect(ShortcutAction.findInTerminal.scope == .terminal)
    }

    @Test("inspect element action is gated to the browser scope")
    func inspectElementActionScope() {
        #expect(ShortcutAction.inspectElement.scope == .browser)
    }
}
