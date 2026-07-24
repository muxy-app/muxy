import Testing

@testable import Muxy

@Suite("Terminal process display name")
struct TerminalProcessDisplayNameTests {
    @Test("active command replaces a shell foreground name")
    func activeCommandReplacesShellName() {
        #expect(TerminalProcessDisplayName.resolve(
            foregroundName: "zsh",
            activeCommand: "npm run dev"
        ) == "npm")
        #expect(TerminalProcessDisplayName.resolve(
            foregroundName: "zsh",
            activeCommand: "php artisan serve"
        ) == "php")
    }

    @Test("non-shell foreground process remains authoritative")
    func foregroundProcessRemainsAuthoritative() {
        #expect(TerminalProcessDisplayName.resolve(
            foregroundName: "nvim",
            activeCommand: "npm run dev"
        ) == "nvim")
    }

    @Test("shell name returns after the active command finishes")
    func shellNameReturnsWithoutActiveCommand() {
        #expect(TerminalProcessDisplayName.resolve(
            foregroundName: "zsh",
            activeCommand: nil
        ) == "zsh")
    }

    @Test("environment assignments are skipped")
    func environmentAssignmentsAreSkipped() {
        #expect(TerminalProcessDisplayName.commandExecutableName(
            "NODE_ENV=development /usr/local/bin/npm run dev"
        ) == "npm")
    }
}
