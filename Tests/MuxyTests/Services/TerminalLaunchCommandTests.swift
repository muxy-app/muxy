import Testing

@testable import Muxy

@Suite("TerminalLaunchCommand")
struct TerminalLaunchCommandTests {
    @Test("Builds remote SSH CLI terminal command")
    func buildsRemoteSSHCLITerminalCommand() {
        let destination = SSHDestination(
            host: "prod",
            remoteRoot: "/srv/app",
            user: "deploy",
            identityFile: "~/.ssh/id_ed25519",
            authenticationMethod: .privateKey
        )

        let command = TerminalLaunchCommand.remoteShellCommand(
            destination: destination,
            workingDirectory: "/srv/app path",
            startupCommand: "echo hi",
            interactive: true,
            keepsShellOpen: true
        )

        #expect(command.hasPrefix("/usr/bin/ssh "))
        #expect(command.contains("-tt"))
        #expect(command.contains("deploy@prod"))
        #expect(command.contains("cd "))
        #expect(command.contains("srv/app path"))
        #expect(command.contains("MUXY_STARTUP_COMMAND="))
        #expect(command.contains("echo hi"))
    }

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

    @Test("Launch wrapper can keep shell open after successful command")
    func launchWrapperKeepsShellOpen() {
        let command = TerminalLaunchCommand.shellCommand(
            interactive: true,
            keepsShellOpen: true,
            shell: "/bin/zsh"
        )

        #expect(command.contains("else exec \"$0\" -l"))
        #expect(!command.contains("else exit $muxy_status"))
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
