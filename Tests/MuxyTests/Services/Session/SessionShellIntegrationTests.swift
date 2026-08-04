import MuxySessionProtocol
import Testing

@Suite("SessionShellIntegration")
struct SessionShellIntegrationTests {
    private let resources = "/Applications/Muxy.app/Contents/Resources/ghostty"
    private var integrationRoot: String { resources + "/shell-integration" }

    private func invocation(
        shell: String,
        command: String = "",
        resourcesDirectory: String? = nil,
        environment: [String: String] = [:]
    ) -> SessionShellInvocation {
        SessionShellIntegration.invocation(
            command: command,
            shell: shell,
            resourcesDirectory: resourcesDirectory ?? resources,
            environment: environment.map { SessionEnvironmentEntry(key: $0.key, value: $0.value) }
        )
    }

    private func value(_ key: String, in invocation: SessionShellInvocation) -> String? {
        invocation.environment.first { $0.key == key }?.value
    }

    @Test("runs the shell as a login shell")
    func runsLoginShell() {
        let result = invocation(shell: "/bin/zsh")
        #expect(result.executable == "/bin/zsh")
        #expect(result.arguments.first == "-zsh")
    }

    @Test("points zsh at the bundled ZDOTDIR")
    func injectsZsh() {
        let result = invocation(shell: "/bin/zsh")
        #expect(value("ZDOTDIR", in: result) == integrationRoot + "/zsh")
        #expect(value("GHOSTTY_RESOURCES_DIR", in: result) == resources)
        #expect(value("GHOSTTY_ZSH_ZDOTDIR", in: result) == nil)
    }

    @Test("preserves an existing ZDOTDIR so the integration can restore it")
    func preservesExistingZDOTDIR() {
        let result = invocation(shell: "/opt/homebrew/bin/zsh", environment: ["ZDOTDIR": "/Users/test/.config/zsh"])
        #expect(value("GHOSTTY_ZSH_ZDOTDIR", in: result) == "/Users/test/.config/zsh")
        #expect(value("ZDOTDIR", in: result) == integrationRoot + "/zsh")
    }

    @Test("starts bash in posix mode with ENV pointing at the integration")
    func injectsBash() {
        let result = invocation(shell: "/bin/bash", environment: ["HOME": "/Users/test"])
        #expect(result.arguments == ["-bash", "--posix"])
        #expect(value("ENV", in: result) == integrationRoot + "/bash/ghostty.bash")
        #expect(value("GHOSTTY_BASH_INJECT", in: result) == "1")
        #expect(value("HISTFILE", in: result) == "/Users/test/.bash_history")
        #expect(value("GHOSTTY_BASH_UNEXPORT_HISTFILE", in: result) == "1")
    }

    @Test("preserves an existing bash ENV and HISTFILE")
    func preservesExistingBashState() {
        let result = invocation(
            shell: "/bin/bash",
            environment: ["ENV": "/Users/test/.env", "HISTFILE": "/Users/test/.history", "HOME": "/Users/test"]
        )
        #expect(value("GHOSTTY_BASH_ENV", in: result) == "/Users/test/.env")
        #expect(value("ENV", in: result) == integrationRoot + "/bash/ghostty.bash")
        #expect(value("HISTFILE", in: result) == "/Users/test/.history")
        #expect(value("GHOSTTY_BASH_UNEXPORT_HISTFILE", in: result) == nil)
    }

    @Test("leaves HISTFILE alone when HOME is unknown")
    func skipsHistfileWithoutHome() {
        let result = invocation(shell: "/bin/bash")
        #expect(value("HISTFILE", in: result) == nil)
        #expect(value("GHOSTTY_BASH_UNEXPORT_HISTFILE", in: result) == nil)
    }

    @Test("prepends the integration root to XDG_DATA_DIRS for fish, elvish and nushell")
    func injectsXDGShells() {
        for shell in ["/opt/homebrew/bin/fish", "/usr/local/bin/elvish", "/opt/homebrew/bin/nu"] {
            let result = invocation(shell: shell, environment: ["XDG_DATA_DIRS": "/usr/share"])
            #expect(value("XDG_DATA_DIRS", in: result) == integrationRoot + ":/usr/share")
            #expect(value("GHOSTTY_SHELL_INTEGRATION_XDG_DIR", in: result) == integrationRoot)
        }
    }

    @Test("keeps the XDG defaults when the variable is unset")
    func usesXDGDefaults() {
        let result = invocation(shell: "/opt/homebrew/bin/fish")
        #expect(value("XDG_DATA_DIRS", in: result) == integrationRoot + ":/usr/local/share:/usr/share")
    }

    @Test("does not add the integration root to XDG_DATA_DIRS twice")
    func doesNotDuplicateXDGEntry() {
        let result = invocation(
            shell: "/opt/homebrew/bin/fish",
            environment: ["XDG_DATA_DIRS": integrationRoot + ":/usr/share"]
        )
        #expect(value("XDG_DATA_DIRS", in: result) == integrationRoot + ":/usr/share")
    }

    @Test("leaves unknown shells untouched apart from the resources directory")
    func leavesUnknownShellsAlone() {
        let result = invocation(shell: "/usr/local/bin/xonsh")
        #expect(result.arguments == ["-xonsh"])
        #expect(value("ZDOTDIR", in: result) == nil)
        #expect(value("XDG_DATA_DIRS", in: result) == nil)
        #expect(value("ENV", in: result) == nil)
    }

    @Test("skips every injection when no resources directory is known")
    func skipsWithoutResources() {
        let result = invocation(shell: "/bin/zsh", resourcesDirectory: "")
        #expect(result.arguments == ["-zsh"])
        #expect(value("ZDOTDIR", in: result) == nil)
        #expect(value("GHOSTTY_RESOURCES_DIR", in: result) == nil)
    }

    @Test("runs a startup command through sh -c so the wrapper keeps its shell syntax")
    func runsStartupCommand() {
        let command = "/bin/zsh -l -c 'eval \"$MUXY_STARTUP_COMMAND\"' /bin/zsh"
        let result = invocation(shell: "/bin/zsh", command: command)
        #expect(result.executable == "/bin/sh")
        #expect(result.arguments == ["/bin/sh", "-c", "exec " + command])
        #expect(value("ZDOTDIR", in: result) == integrationRoot + "/zsh")
    }

    @Test("injects bash integration for startup commands without changing the outer shell")
    func injectsBashForCommands() {
        let result = invocation(shell: "/bin/bash", command: "echo hi", environment: ["HOME": "/Users/test"])
        #expect(result.executable == "/bin/sh")
        #expect(result.arguments == ["/bin/sh", "-c", "exec echo hi"])
        #expect(value("ENV", in: result) == integrationRoot + "/bash/ghostty.bash")
        #expect(value("GHOSTTY_BASH_INJECT", in: result) == "1")
    }

    @Test("falls back to zsh when the shell is unknown")
    func fallsBackToDefaultShell() {
        let result = invocation(shell: "")
        #expect(result.executable == SessionShellIntegration.defaultShell)
        #expect(result.arguments == ["-zsh"])
    }

    @Test("derives the shell name from the last path component")
    func derivesShellName() {
        #expect(SessionShellIntegration.shellName("/bin/zsh") == "zsh")
        #expect(SessionShellIntegration.shellName("zsh") == "zsh")
        #expect(SessionShellIntegration.shellName("/opt/homebrew/bin/fish") == "fish")
    }

    @Test("keeps the inherited environment")
    func keepsInheritedEnvironment() {
        let result = invocation(shell: "/bin/zsh", environment: ["TERM": "xterm-ghostty", "MUXY_PANE_ID": "abc"])
        #expect(value("TERM", in: result) == "xterm-ghostty")
        #expect(value("MUXY_PANE_ID", in: result) == "abc")
    }
}
