import Foundation
import Testing

@testable import Muxy

@Suite("TerminalLaunchCommand")
struct TerminalLaunchCommandTests {
    private let recoveryToken = UUID(uuidString: "06D7D8FB-BAA8-4465-991B-8CBEEFD79E94")!

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
            configuration: remoteConfiguration()
        )
        #expect(command.hasPrefix("/bin/sh -c "))
        #expect(command.contains("/usr/bin/ssh "))
        #expect(command.contains("-tt"))
        #expect(command.contains("export TERM=xterm-256color; cd ~/code/api && exec \"${SHELL:-/bin/sh}\" -l -i"))
    }

    @Test("Remote shell retries transport failures with bounded backoff")
    func remoteShellRetriesTransportFailures() {
        let command = TerminalLaunchCommand.remoteShellCommand(
            destination: SSHDestination(host: "prod"),
            workingDirectory: "~",
            startupCommand: nil,
            configuration: remoteConfiguration()
        )

        #expect(command.contains("[ \"$muxy_status\" -eq 255 ]"))
        #expect(command.contains("[ \"$muxy_attempt\" -gt 5 ]"))
        #expect(command.contains("muxy_elapsed\" -lt 60"))
        #expect(command.contains("1) muxy_delay=1"))
        #expect(command.contains("2) muxy_delay=2"))
        #expect(command.contains("3) muxy_delay=4"))
        #expect(command.contains("*) muxy_delay=8"))
        #expect(command.contains("SSH connection lost. Reconnecting"))
    }

    @Test("Remote shell waits for manual retry after automatic recovery is exhausted")
    func remoteShellWaitsForManualRetry() {
        let script = TerminalLaunchCommand.remoteReconnectScript(
            initialSSH: "exit 255",
            reconnectSSH: "exit 255",
            recoveryToken: recoveryToken
        )

        let recoveryTitle = TerminalLaunchCommand.remoteReconnectRequiredTitle(recoveryToken: recoveryToken)
        #expect(script.contains(recoveryTitle))
        #expect(script.contains("while :; do sleep 3600; done"))
    }

    @Test("Remote recovery state notifies the pane")
    @MainActor
    func remoteRecoveryStateNotifiesPane() {
        let recoveryToken = UUID()
        let view = GhosttyTerminalNSView(
            workingDirectory: "~",
            workspaceContext: .ssh(SSHDestination(host: "prod")),
            remoteRecoveryToken: recoveryToken
        )
        var events: [String] = []
        view.onSearchEnd = { events.append("search-ended") }
        view.onRemoteSessionRecoveryFailed = { events.append("recovery-\($0)") }

        let forgedTitle = TerminalLaunchCommand.remoteReconnectRequiredTitle(recoveryToken: UUID())
        let validTitle = TerminalLaunchCommand.remoteReconnectRequiredTitle(recoveryToken: recoveryToken)

        #expect(!view.handleRemoteSessionRecoveryTitle(forgedTitle))
        #expect(!view.processExitHandled)
        #expect(view.handleRemoteSessionRecoveryTitle(validTitle))
        #expect(events == ["search-ended", "recovery-true"])
        #expect(view.isRemoteSessionRecoveryFailed)
        #expect(view.processExitHandled)

        view.retryRemoteSession()

        #expect(events == ["search-ended", "recovery-true", "recovery-false"])
        #expect(!view.isRemoteSessionRecoveryFailed)
        #expect(!view.processExitHandled)
        #expect(!view.handleRemoteSessionRecoveryTitle(validTitle))
    }

    @Test("Remote reconnect wrapper is valid POSIX shell")
    func remoteReconnectWrapperIsValidShell() throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sh")
        process.arguments = [
            "-n",
            "-c",
            TerminalLaunchCommand.remoteReconnectScript(
                initialSSH: "exit 0",
                reconnectSSH: "exit 0",
                recoveryToken: recoveryToken
            ),
        ]

        try process.run()
        process.waitUntilExit()

        #expect(process.terminationStatus == 0)
    }

    @Test("Immediate SSH failures wait for manual recovery without automatic retries")
    func immediateSSHFailuresWaitForManualRecovery() throws {
        let script = executableRecoveryScript(TerminalLaunchCommand.remoteReconnectScript(
            initialSSH: "/bin/sh -c 'printf [INITIAL]; exit 255'",
            reconnectSSH: "/bin/sh -c 'printf [RECONNECT]; exit 255'",
            recoveryToken: recoveryToken
        ))

        let result = try runShell(script)

        #expect(result.status == 75)
        #expect(occurrences(of: "[INITIAL]", in: result.output) == 1)
        #expect(!result.output.contains("[RECONNECT]"))
        #expect(result.output.contains(TerminalLaunchCommand.remoteReconnectRequiredTitle(recoveryToken: recoveryToken)))
    }

    @Test("Established SSH failures consume the bounded automatic retry budget")
    func establishedSSHFailuresRetryWithinBudget() throws {
        let script = executableRecoveryScript(TerminalLaunchCommand.remoteReconnectScript(
            initialSSH: "/bin/sh -c 'printf [INITIAL]; exit 255'",
            reconnectSSH: "/bin/sh -c 'printf [RECONNECT]; exit 255'",
            recoveryToken: recoveryToken,
            attemptLimit: 2,
            minimumSessionDuration: 0
        ))

        let result = try runShell(script)

        #expect(result.status == 75)
        #expect(occurrences(of: "[INITIAL]", in: result.output) == 1)
        #expect(occurrences(of: "[RECONNECT]", in: result.output) == 2)
    }

    @Test("Non-transport SSH exits preserve status without retrying")
    func nonTransportSSHExitsWithoutRetrying() throws {
        let script = TerminalLaunchCommand.remoteReconnectScript(
            initialSSH: "/bin/sh -c 'printf [INITIAL]; exit 42'",
            reconnectSSH: "/bin/sh -c 'printf [RECONNECT]; exit 255'",
            recoveryToken: recoveryToken
        )

        let result = try runShell(script)

        #expect(result.status == 42)
        #expect(occurrences(of: "[INITIAL]", in: result.output) == 1)
        #expect(!result.output.contains("[RECONNECT]"))
        #expect(!result.output.contains(TerminalLaunchCommand.remoteReconnectRequiredTitle(recoveryToken: recoveryToken)))
    }

    @Test("Remote command status 255 is reserved for SSH transport failures")
    func remoteCommandStatus255IsRemapped() throws {
        let command = TerminalLaunchCommand.remoteCommandReservingTransportFailureStatus("exit 255")

        let result = try runShell(command)

        #expect(result.status == 254)
    }

    @Test("Local cancellation does not trigger SSH recovery")
    func localCancellationDoesNotTriggerRecovery() throws {
        let script = TerminalLaunchCommand.remoteReconnectScript(
            initialSSH: "kill -INT $$",
            reconnectSSH: "printf [RECONNECT]",
            recoveryToken: recoveryToken
        )

        let result = try runShell(script)

        #expect(result.status == 130)
        #expect(!result.output.contains("[RECONNECT]"))
        #expect(!result.output.contains(TerminalLaunchCommand.remoteReconnectRequiredTitle(recoveryToken: recoveryToken)))
    }

    @Test("Remote shell does not replay startup command after reconnecting")
    func remoteShellDoesNotReplayStartupCommand() {
        let startupCommand = "printf MUXY_UNIQUE_STARTUP"
        let command = TerminalLaunchCommand.remoteShellCommand(
            destination: SSHDestination(host: "prod"),
            workingDirectory: "~",
            startupCommand: startupCommand,
            configuration: remoteConfiguration(keepsShellOpen: true)
        )

        #expect(command.components(separatedBy: startupCommand).count == 2)
        #expect(command.contains("muxy_initial"))
    }

    @Test("Remote shell escapes an injected startup command so it cannot break out")
    func remoteShellNeutralizesStartupCommand() {
        let payload = "x'; touch /tmp/pwn; '"
        let command = TerminalLaunchCommand.remoteShellCommand(
            destination: SSHDestination(host: "prod"),
            workingDirectory: "~",
            startupCommand: payload,
            configuration: remoteConfiguration(interactive: false)
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
            configuration: remoteConfiguration(keepsShellOpen: true)
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
            configuration: remoteConfiguration()
        )
        #expect(command.contains("export LANG=C.UTF-8; export TERM=screen-256color; cd ~ && exec \"${SHELL:-/bin/sh}\" -l -i"))
    }

    private func executableRecoveryScript(_ script: String) -> String {
        precondition(script.contains("sleep \"$muxy_delay\""))
        precondition(script.contains("while :; do sleep 3600; done"))
        return script
            .replacingOccurrences(of: "sleep \"$muxy_delay\"", with: ":")
            .replacingOccurrences(of: "while :; do sleep 3600; done", with: "exit 75")
    }

    private func remoteConfiguration(
        interactive: Bool = true,
        keepsShellOpen: Bool = false
    ) -> TerminalLaunchCommand.RemoteShellConfiguration {
        TerminalLaunchCommand.RemoteShellConfiguration(
            interactive: interactive,
            keepsShellOpen: keepsShellOpen,
            recoveryToken: recoveryToken
        )
    }

    private func runShell(_ script: String) throws -> (status: Int32, output: String) {
        let process = Process()
        let output = Pipe()
        process.executableURL = URL(fileURLWithPath: "/bin/sh")
        process.arguments = ["-c", script]
        process.standardOutput = output
        process.standardError = output

        try process.run()
        process.waitUntilExit()

        let data = output.fileHandleForReading.readDataToEndOfFile()
        return (process.terminationStatus, String(decoding: data, as: UTF8.self))
    }

    private func occurrences(of needle: String, in haystack: String) -> Int {
        haystack.components(separatedBy: needle).count - 1
    }
}
