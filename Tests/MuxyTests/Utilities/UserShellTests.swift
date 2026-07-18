import Testing

@testable import Muxy

@Suite("UserShell")
struct UserShellTests {
    @Test("environment shell takes precedence")
    func environmentShellTakesPrecedence() {
        let shell = UserShell.path(
            environment: ["SHELL": "/bin/bash"],
            accountShell: { "/opt/homebrew/bin/fish" }
        )

        #expect(shell == "/bin/bash")
    }

    @Test("account shell is used when GUI environment omits SHELL")
    func accountShellUsedWithoutEnvironmentShell() {
        let shell = UserShell.path(
            environment: [:],
            accountShell: { "/opt/homebrew/bin/fish" }
        )

        #expect(shell == "/opt/homebrew/bin/fish")
    }

    @Test("zsh is the final fallback")
    func zshIsFinalFallback() {
        #expect(UserShell.path(environment: [:], accountShell: { nil }) == "/bin/zsh")
    }
}
