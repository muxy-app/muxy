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

    @Test("Builds fish startup commands without changing interactive shell behavior")
    func buildsFishStartupCommandsWithoutChangingInteractiveShellBehavior() {
        let closingCommand = TerminalLaunchCommand.shellCommand(
            interactive: true,
            shell: "/opt/homebrew/bin/fish"
        )
        let persistentCommand = TerminalLaunchCommand.shellCommand(
            interactive: true,
            keepsShellOpen: true,
            shell: "/opt/homebrew/bin/fish"
        )
        let nonInteractiveCommand = TerminalLaunchCommand.shellCommand(
            interactive: false,
            shell: "/opt/homebrew/bin/fish"
        )

        #expect(closingCommand.hasPrefix("/opt/homebrew/bin/fish -l -i -c 'eval \"$MUXY_STARTUP_COMMAND\"; set muxy_status $status;"))
        #expect(closingCommand.contains("if test $muxy_status -ne 0"))
        #expect(closingCommand.contains("exec \"$argv[1]\" -l"))
        #expect(closingCommand.contains("else exit $muxy_status"))
        #expect(!closingCommand.contains("muxy_status=$?"))
        #expect(!closingCommand.contains("then exec \"$0\" -l"))
        #expect(persistentCommand.contains("else exec \"$argv[1]\" -l"))
        #expect(nonInteractiveCommand.hasPrefix("/opt/homebrew/bin/fish -l -c 'eval \"$MUXY_STARTUP_COMMAND\"; set muxy_status $status;"))
        #expect(!nonInteractiveCommand.hasPrefix("/opt/homebrew/bin/fish -l -i -c"))
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

    @Test("Remote shell folds the working directory and targets the host")
    func remoteShellFoldsWorkingDirectory() {
        let command = TerminalLaunchCommand.remoteShellCommand(
            destination: SSHDestination(host: "prod"),
            workingDirectory: "~/code/api",
            startupCommand: nil,
            interactive: true,
            keepsShellOpen: false
        )
        #expect(command.hasPrefix("/usr/bin/ssh "))
        #expect(command.contains("-tt"))
        #expect(command.contains("'export TERM=xterm-256color; cd ~/code/api && exec \"${SHELL:-/bin/sh}\" -l -i'"))
    }

    @Test("Remote shell escapes an injected startup command so it cannot break out")
    func remoteShellNeutralizesStartupCommand() {
        let payload = "x'; touch /tmp/pwn; '"
        let command = TerminalLaunchCommand.remoteShellCommand(
            destination: SSHDestination(host: "prod"),
            workingDirectory: "~",
            startupCommand: payload,
            interactive: false,
            keepsShellOpen: false
        )
        #expect(command.contains("export MUXY_STARTUP_COMMAND="))
        #expect(command.contains("export TERM=xterm-256color"))
        #expect(!command.contains(payload))
        #expect(command.contains("'\\''"))
    }

    @Test("Remote startup commands select fish syntax at execution time")
    func remoteStartupCommandSelectsFishSyntax() {
        let command = TerminalLaunchCommand.remoteShellCommand(
            destination: SSHDestination(host: "prod"),
            workingDirectory: "~",
            startupCommand: "printf REMOTE_FISH",
            interactive: true,
            keepsShellOpen: true
        )

        #expect(command.contains("exec /bin/sh -c"))
        #expect(command.contains("__muxy_shell_name=${__muxy_shell##*/}"))
        #expect(command.contains("case \"$__muxy_shell_name\" in fish)"))
        #expect(command.contains("fish) exec \"$__muxy_shell\" -l -i -c"))
        #expect(command.contains("set muxy_status $status"))
        #expect(command.contains("else exec \"$argv[1]\" -l"))
        #expect(command.contains("*) exec \"$__muxy_shell\" -l -i -c"))
        #expect(command.contains("muxy_status=$?"))
    }

    @Test("Remote shell uses configured environment")
    func remoteShellUsesConfiguredEnvironment() {
        let command = TerminalLaunchCommand.remoteShellCommand(
            destination: SSHDestination(host: "prod", environment: ["TERM": "screen-256color", "LANG": "C.UTF-8"]),
            workingDirectory: "~",
            startupCommand: nil,
            interactive: true,
            keepsShellOpen: false
        )
        #expect(command.contains("'export LANG=C.UTF-8; export TERM=screen-256color; cd ~ && exec \"${SHELL:-/bin/sh}\" -l -i'"))
    }
}
