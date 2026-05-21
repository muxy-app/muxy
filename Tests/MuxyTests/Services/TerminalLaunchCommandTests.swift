import Testing

@testable import Muxy

@Suite("TerminalLaunchCommand")
struct TerminalLaunchCommandTests {
    @Test("Builds non-interactive login shell command")
    func buildsNonInteractiveLoginShellCommand() {
        let command = TerminalLaunchCommand.shellCommand(interactive: false, shell: "/bin/zsh")
        #expect(command.hasPrefix("/bin/zsh -l -c 'eval \"$MUXY_STARTUP_COMMAND\"; muxy_status=$?;"))
        #expect(command.contains("then exec \"$0\" -l"))
        #expect(command.hasSuffix("' /bin/zsh"))
    }

    @Test("Builds interactive login shell command")
    func buildsInteractiveLoginShellCommand() {
        let command = TerminalLaunchCommand.shellCommand(interactive: true, shell: "/bin/zsh")
        #expect(command.hasPrefix("/bin/zsh -l -i -c 'eval \"$MUXY_STARTUP_COMMAND\"; muxy_status=$?;"))
        #expect(command.contains("exit $muxy_status"))
        #expect(command.contains("then exec \"$0\" -l"))
        #expect(command.hasSuffix("' /bin/zsh"))
    }

    @Test("Launch wrapper does not embed user command")
    func launchWrapperDoesNotEmbedUserCommand() {
        let command = TerminalLaunchCommand.shellCommand(interactive: true, shell: "/bin/zsh")
        #expect(!command.contains("/Users/some user/Library/Application Support/some file.json"))
    }

    @Test("Escapes shell path in launch wrapper")
    func escapesShellPathInLaunchWrapper() {
        let command = TerminalLaunchCommand.shellCommand(interactive: false, shell: "/tmp/my shell;touch /tmp/pwn")
        #expect(command.hasPrefix("'/tmp/my shell;touch /tmp/pwn' -l -c 'eval \"$MUXY_STARTUP_COMMAND\""))
        #expect(command.contains("then exec \"$0\" -l"))
        #expect(command.hasSuffix("' '/tmp/my shell;touch /tmp/pwn'"))
    }
}
