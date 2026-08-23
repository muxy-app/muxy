import Foundation
import Testing

@testable import Muxy

@Suite("Terminal session backing")
struct TerminalSessionBackingTests {
    private let paneID = UUID(uuidString: "01234567-89AB-CDEF-0123-456789ABCDEF")!
    private let sessionID = UUID(uuidString: "FEDCBA98-7654-3210-FEDC-BA9876543210")!

    @Test("resolves local persistent sessions with the supplied session identifier")
    func resolvesLocalPersistentSession() {
        let backing = TerminalSessionBacking.resolve(
            paneID: paneID,
            sessionID: sessionID,
            workspaceContext: .local,
            usesLocalPersistentSession: true
        )

        #expect(backing == .local(sessionID))
        #expect(backing.localSessionID == sessionID)
    }

    @Test("resolves direct remote sessions without a local persistent identifier")
    func resolvesDirectRemoteSession() {
        let backing = TerminalSessionBacking.resolve(
            paneID: paneID,
            sessionID: sessionID,
            workspaceContext: .ssh(SSHDestination(host: "example.com", remoteSessionMode: .direct)),
            usesLocalPersistentSession: false
        )

        #expect(backing == .direct)
        #expect(backing.localSessionID == nil)
    }

    @Test("resolves tmux remote sessions without local persistent session semantics")
    func resolvesTmuxRemoteSession() {
        let destination = SSHDestination(host: "example.com", remoteSessionMode: .tmux)
        let backing = TerminalSessionBacking.resolve(
            paneID: paneID,
            sessionID: nil,
            workspaceContext: .ssh(destination),
            usesLocalPersistentSession: false
        )

        #expect(backing == .remoteTmux(RemoteTmuxSession(id: paneID, destination: destination)))
        #expect(backing.localSessionID == nil)
    }

    @Test("preserves the original tmux destination when device settings change")
    func preservesOriginalTmuxDestination() {
        let original = SSHDestination(host: "original.example.com", remoteSessionMode: .tmux)
        let current = SSHDestination(host: "current.example.com", remoteSessionMode: .direct)
        let backing = TerminalSessionBacking.resolve(
            paneID: paneID,
            sessionID: sessionID,
            workspaceContext: .ssh(current),
            usesLocalPersistentSession: false,
            remoteSessionMode: .tmux,
            remoteTmuxDestination: original
        )

        #expect(backing == .remoteTmux(RemoteTmuxSession(id: sessionID, destination: original)))
    }

    @MainActor
    @Test("pane snapshots preserve immutable tmux backing")
    func snapshotsPreserveTmuxBacking() throws {
        let destination = SSHDestination(host: "example.com", remoteSessionMode: .tmux)
        let pane = TerminalPaneState(
            sessionID: sessionID,
            projectPath: "~/code",
            remoteSessionMode: .tmux,
            remoteTmuxDestination: destination
        )
        let encoded = try JSONEncoder().encode(TerminalTab(pane: pane).snapshot())
        let snapshot = try JSONDecoder().decode(TerminalTabSnapshot.self, from: encoded)
        let restored = TerminalTab(restoring: snapshot)

        #expect(restored.content.pane?.remoteSessionMode == .tmux)
        #expect(restored.content.pane?.remoteTmuxDestination == destination)
        #expect(restored.content.pane?.sessionID == sessionID)
        #expect(restored.content.pane?.createsRemoteTmuxSessionIfMissing == false)
    }

    @MainActor
    @Test("unmaterialized pane snapshots preserve unresolved backing")
    func snapshotsPreserveUnresolvedBacking() throws {
        let pane = TerminalPaneState(projectPath: "~/code")
        let encoded = try JSONEncoder().encode(TerminalTab(pane: pane).snapshot())
        let snapshot = try JSONDecoder().decode(TerminalTabSnapshot.self, from: encoded)
        let restored = TerminalTab(restoring: snapshot)

        #expect(!snapshot.paneRemoteSessionModeResolved)
        #expect(restored.content.pane?.remoteSessionMode == nil)
        #expect(restored.content.pane?.createsRemoteTmuxSessionIfMissing == true)
    }

    @MainActor
    @Test("tmux surfaces launch and upload through their persisted destination")
    func tmuxSurfacesUsePersistedDestination() {
        let original = SSHDestination(host: "original.example.com", remoteSessionMode: .tmux)
        let current = SSHDestination(host: "current.example.com", remoteSessionMode: .direct)
        let session = RemoteTmuxSession(id: sessionID, destination: original)
        let surface = GhosttyTerminalNSView(
            workingDirectory: "~/code",
            workspaceContext: .ssh(current),
            sessionBacking: .remoteTmux(session)
        )

        #expect(surface.effectiveSSHDestination == original)
        #expect(surface.uploadDestination == original)
    }
}
