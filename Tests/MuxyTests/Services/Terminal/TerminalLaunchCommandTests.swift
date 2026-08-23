import Foundation
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

    @Test("Remote tmux initial launch wraps the remote login command with its exact target")
    func remoteTmuxInitialLaunchWrapsRemoteLoginCommand() {
        let identifier = UUID(uuidString: "01234567-89AB-CDEF-0123-456789ABCDEF")!
        let destination = SSHDestination(host: "prod", remoteSessionMode: .tmux)
        let session = RemoteTmuxSession(id: identifier, destination: destination)
        let command = TerminalLaunchCommand.remoteShellCommand(
            destination: destination,
            workingDirectory: "~/code/api",
            startupCommand: "npm run dev",
            interactive: true,
            keepsShellOpen: false,
            tmuxSession: session
        )
        #expect(command.contains("tmux new-session -d -s muxy-0123456789abcdef0123456789abcdef"))
        #expect(command.contains("tmux has-session -t '\\''=muxy-0123456789abcdef0123456789abcdef'\\''"))
        #expect(command.contains("__muxy_shell_name=${__muxy_shell##*/}"))
        #expect(command.contains("MUXY_STARTUP_COMMAND"))
    }

    @Test("Remote tmux recovery attaches without creating or restarting the startup command")
    func remoteTmuxRecoveryOnlyAttaches() {
        let identifier = UUID(uuidString: "01234567-89AB-CDEF-0123-456789ABCDEF")!
        let destination = SSHDestination(host: "prod", remoteSessionMode: .tmux)
        let command = TerminalLaunchCommand.remoteShellCommand(
            destination: destination,
            workingDirectory: "~/code/api",
            startupCommand: "npm run dev",
            interactive: true,
            keepsShellOpen: false,
            tmuxSession: RemoteTmuxSession(id: identifier, destination: destination),
            createsTmuxSessionIfMissing: false
        )

        #expect(command.contains("attach-session"))
        #expect(command.contains("muxy-0123456789abcdef0123456789abcdef"))
        #expect(!command.contains("new-session"))
        #expect(!command.contains("npm run dev"))
    }

    @Test("Remote tmux paths bypass tmux format expansion")
    func remoteTmuxPathBypassesTmuxFormats() {
        let identifier = UUID(uuidString: "01234567-89AB-CDEF-0123-456789ABCDEF")!
        let destination = SSHDestination(host: "prod", remoteSessionMode: .tmux)
        let command = TerminalLaunchCommand.remoteShellCommand(
            destination: destination,
            workingDirectory: "/srv/#(touch /tmp/pwn)",
            startupCommand: nil,
            interactive: true,
            keepsShellOpen: false,
            tmuxSession: RemoteTmuxSession(id: identifier, destination: destination)
        )

        #expect(!command.contains("new-session -d -s muxy-0123456789abcdef0123456789abcdef -c"))
        #expect(command.contains("#(touch /tmp/pwn)"))
    }

    @Test("Remote tmux preserves outer TERM without overriding its child TERM")
    func remoteTmuxOwnsChildTerm() {
        let identifier = UUID(uuidString: "01234567-89AB-CDEF-0123-456789ABCDEF")!
        let destination = SSHDestination(
            host: "prod",
            environment: ["TERM": "xterm-256color", "TMUX_TMPDIR": "/tmp/custom"],
            remoteSessionMode: .tmux
        )
        let command = TerminalLaunchCommand.remoteShellCommand(
            destination: destination,
            workingDirectory: "~/code",
            startupCommand: nil,
            interactive: true,
            keepsShellOpen: false,
            tmuxSession: RemoteTmuxSession(id: identifier, destination: destination)
        )

        #expect(command.components(separatedBy: "export TERM=xterm-256color").count == 2)
        #expect(command.contains("export TMUX_TMPDIR=/tmp/custom"))
    }
}
