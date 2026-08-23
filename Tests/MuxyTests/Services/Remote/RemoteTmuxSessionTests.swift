import Foundation
import Testing

@testable import Muxy

@Suite("Remote tmux sessions")
struct RemoteTmuxSessionTests {
    private let id = UUID(uuidString: "01234567-89AB-CDEF-0123-456789ABCDEF")!

    @Test("legacy SSH data defaults to direct sessions")
    func legacySSHDataDefaultsToDirect() throws {
        let data = """
        {"host":"example.com","remoteRoot":"~/code"}
        """.data(using: .utf8)!

        #expect(try JSONDecoder().decode(SSHWorkspaceData.self, from: data).remoteSessionMode == .direct)
        #expect(try JSONDecoder().decode(SSHDestination.self, from: data).remoteSessionMode == .direct)
    }

    @Test("tmux mode round trips through SSH workspace data")
    func tmuxModeRoundTrips() throws {
        let original = SSHWorkspaceData(host: "example.com", remoteSessionMode: .tmux)
        let decoded = try JSONDecoder().decode(SSHWorkspaceData.self, from: JSONEncoder().encode(original))

        #expect(decoded.remoteSessionMode == .tmux)
        #expect(decoded.destination.remoteSessionMode == .tmux)
    }

    @Test("session mode does not change SSH connection identity")
    func sessionModeDoesNotChangeConnectionIdentity() {
        let direct = SSHDestination(host: "example.com", remoteSessionMode: .direct)
        let tmux = SSHDestination(host: "example.com", remoteSessionMode: .tmux)

        #expect(direct.connectionKey == tmux.connectionKey)
    }

    @Test("session name and target are stable and host independent")
    func stableSessionName() {
        let first = RemoteTmuxSession(id: id, destination: SSHDestination(host: "one.example.com"))
        let second = RemoteTmuxSession(id: id, destination: SSHDestination(host: "two.example.com"))

        #expect(first.name == "muxy-0123456789abcdef0123456789abcdef")
        #expect(first.target == "=muxy-0123456789abcdef0123456789abcdef")
        #expect(first.name == second.name)
    }

    @Test("commands escape initial commands without tmux path formats")
    func commandEscaping() {
        let command = RemoteTmuxCommandBuilder.attachOrCreateCommand(
            for: session,
            initialCommand: "echo 'hello'; rm -rf /"
        )

        #expect(command.contains("'echo '\\''hello'\\''; rm -rf /'"))
        #expect(command.contains("-t '=muxy-0123456789abcdef0123456789abcdef'"))
        #expect(!command.contains(" -c "))
        #expect(command.contains("muxy-0123456789abcdef0123456789abcdef.closed"))
        #expect(command.contains("[ ! -e \"$__muxy_tmux_tombstone\" ]"))
    }

    @Test("startup command applies creation settings only while attaching exactly")
    func startupCommand() {
        let command = RemoteTmuxCommandBuilder.attachOrCreateCommand(
            for: session,
            initialCommand: "npm run dev"
        )

        #expect(command.contains("if ! tmux has-session -t '=muxy-0123456789abcdef0123456789abcdef'"))
        #expect(command.contains("tmux new-session -d -s muxy-0123456789abcdef0123456789abcdef 'npm run dev'"))
        #expect(command.contains("tmux has-session -t '=muxy-0123456789abcdef0123456789abcdef' 2>/dev/null || exit 1"))
        #expect(command.hasSuffix("exec tmux attach-session -d -t '=muxy-0123456789abcdef0123456789abcdef'"))
    }

    @Test("commands never perform broad tmux operations")
    func commandsAvoidBroadOperations() {
        let commands = [
            RemoteTmuxCommandBuilder.availabilityCommand(),
            RemoteTmuxCommandBuilder.hasSessionCommand(for: session),
            RemoteTmuxCommandBuilder.attachOrCreateCommand(for: session, initialCommand: "sh"),
            RemoteTmuxCommandBuilder.killSessionCommand(for: session),
        ].joined(separator: "\n")

        #expect(!commands.contains("kill-server"))
        #expect(!commands.contains(" detach-client"))
        #expect(!commands.contains(" -x"))
        #expect(!commands.contains("detach-session"))
        let killCommand = RemoteTmuxCommandBuilder.killSessionCommand(for: session)
        #expect(killCommand.contains("mkdir \"$__muxy_tmux_tombstone\""))
        #expect(killCommand.contains("[ ! -L \"$__muxy_tmux_tombstone\" ]"))
    }

    @Test("lookup and availability require explicit successful markers")
    func markerClassification() {
        #expect(RemoteTmuxSessionService.lookup(for: result(RemoteTmuxSessionService.presentMarker)) == .present)
        #expect(RemoteTmuxSessionService.lookup(for: result(RemoteTmuxSessionService.absentMarker)) == .absent)
        #expect(RemoteTmuxSessionService.lookup(for: result(RemoteTmuxSessionService.unavailableMarker)) == .unavailable)
        #expect(RemoteTmuxSessionService.lookup(for: result(RemoteTmuxSessionService.unknownMarker)) == .unknown)
        #expect(RemoteTmuxSessionService.lookup(for: result("shell output\n\(RemoteTmuxSessionService.absentMarker)")) == .unknown)
        #expect(RemoteTmuxSessionService.lookup(for: result(RemoteTmuxSessionService.absentMarker, status: 255)) == .unknown)
        #expect(RemoteTmuxSessionService.lookup(for: result(RemoteTmuxSessionService.absentMarker, truncated: true)) == .unknown)
        #expect(RemoteTmuxSessionService.availability(for: result(RemoteTmuxSessionService.availableMarker)) == .available)
        #expect(RemoteTmuxSessionService.availability(for: result(RemoteTmuxSessionService.unavailableMarker)) == .unavailable)
        #expect(RemoteTmuxSessionService.availability(for: result("unexpected")) == .unknown)
        let lookupCommand = RemoteTmuxCommandBuilder.hasSessionCommand(for: session)
        #expect(!lookupCommand.contains("no server running"))
        #expect(lookupCommand.contains(RemoteTmuxSessionService.unknownMarker))
    }

    private var session: RemoteTmuxSession {
        RemoteTmuxSession(id: id, destination: SSHDestination(host: "example.com"))
    }

    private func result(_ stdout: String, status: Int32 = 0, truncated: Bool = false) -> GitProcessResult {
        GitProcessResult(status: status, stdout: stdout, stdoutData: Data(), stderr: "", truncated: truncated)
    }
}
