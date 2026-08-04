import Foundation
import MuxySessionProtocol
import Testing

@testable import Muxy

@Suite("TerminalSessionsStore")
struct TerminalSessionsStoreTests {
    private func descriptor(
        projectID: String? = nil,
        worktreeID: String? = nil,
        title: String? = nil,
        workingDirectory: String = "/Users/test/project",
        isAttached: Bool = false
    ) throws -> SessionDescriptor {
        var metadata: [SessionEnvironmentEntry] = []
        if let projectID {
            metadata.append(SessionEnvironmentEntry(key: SessionMetadataKey.project, value: projectID))
        }
        if let worktreeID {
            metadata.append(SessionEnvironmentEntry(key: SessionMetadataKey.worktree, value: worktreeID))
        }
        if let title {
            metadata.append(SessionEnvironmentEntry(key: SessionMetadataKey.title, value: title))
        }
        return SessionDescriptor(
            identifier: try #require(SessionIdentifier(uuidString: UUID().uuidString)),
            shellProcessID: 900,
            ttyDevice: 42,
            workingDirectory: workingDirectory,
            isAttached: isAttached,
            metadata: metadata
        )
    }

    @Test("keeps only sessions belonging to the given worktree")
    func filtersByWorktree() throws {
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())
        let other = WorktreeKey(projectID: UUID(), worktreeID: UUID())

        let matching = try descriptor(projectID: key.projectID.uuidString, worktreeID: key.worktreeID.uuidString)
        let otherWorktree = try descriptor(projectID: key.projectID.uuidString, worktreeID: other.worktreeID.uuidString)
        let otherProject = try descriptor(projectID: other.projectID.uuidString, worktreeID: key.worktreeID.uuidString)
        let untagged = try descriptor()

        let filtered = TerminalSessionsStore.filter(
            [matching, otherWorktree, otherProject, untagged],
            inWorktree: key
        )
        #expect(filtered == [matching])
    }

    @Test("returns nothing when no session carries metadata")
    func filtersOutUntaggedSessions() throws {
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())
        #expect(TerminalSessionsStore.filter([try descriptor()], inWorktree: key).isEmpty)
    }

    @Test("hides sessions a pane already owns")
    func hidesOwnedSessions() throws {
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())
        let owned = try descriptor(projectID: key.projectID.uuidString, worktreeID: key.worktreeID.uuidString)
        let detached = try descriptor(projectID: key.projectID.uuidString, worktreeID: key.worktreeID.uuidString)
        let ownedID = try #require(UUID(uuidString: owned.identifier.uuidString))

        let result = TerminalSessionsStore.detached(
            [owned, detached],
            inWorktree: key,
            ownedSessionIDs: [ownedID]
        )
        #expect(result == [detached])
    }

    @Test("matches ownership across the lowercase daemon identifier and uppercase pane UUID")
    func matchesOwnershipRegardlessOfCase() throws {
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())
        let session = try descriptor(projectID: key.projectID.uuidString, worktreeID: key.worktreeID.uuidString)
        let identifierText = session.identifier.uuidString
        #expect(identifierText == identifierText.lowercased())

        let paneSessionID = try #require(UUID(uuidString: identifierText))
        #expect(paneSessionID.uuidString == paneSessionID.uuidString.uppercased())

        let result = TerminalSessionsStore.detached([session], inWorktree: key, ownedSessionIDs: [paneSessionID])
        #expect(result.isEmpty)
    }

    @Test("keeps a session still marked attached by the daemon when no pane owns it")
    func keepsUnownedSessionRegardlessOfDaemonFlag() throws {
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())
        let session = try descriptor(
            projectID: key.projectID.uuidString,
            worktreeID: key.worktreeID.uuidString,
            isAttached: true
        )
        let result = TerminalSessionsStore.detached([session], inWorktree: key, ownedSessionIDs: [])
        #expect(result == [session])
    }

    @Test("keeps a session whose pane was freed to save memory out of the list")
    func hidesIdleFreedPane() throws {
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())
        let session = try descriptor(
            projectID: key.projectID.uuidString,
            worktreeID: key.worktreeID.uuidString,
            isAttached: false
        )
        let ownedID = try #require(UUID(uuidString: session.identifier.uuidString))
        #expect(TerminalSessionsStore.detached([session], inWorktree: key, ownedSessionIDs: [ownedID]).isEmpty)
    }

    @Test("prefers the recorded title for display")
    func prefersRecordedTitle() throws {
        let session = try descriptor(title: "Dev Server")
        #expect(TerminalSessionsStore.displayTitle(for: session) == "Dev Server")
    }

    @Test("falls back to the working directory name without a title")
    func fallsBackToDirectoryName() throws {
        let session = try descriptor(workingDirectory: "/Users/test/my-app")
        #expect(TerminalSessionsStore.displayTitle(for: session) == "my-app")
    }

    @Test("falls back to the default terminal title for a blank title and root directory")
    func fallsBackToDefaultTitle() throws {
        let session = try descriptor(title: "   ", workingDirectory: "/")
        #expect(TerminalSessionsStore.displayTitle(for: session) == TerminalPaneState.defaultTitle)
    }
}

@Suite("TerminalSessionAttachment")
@MainActor
struct TerminalSessionAttachmentTests {
    @Test("points a pane at another session")
    func repointsPane() {
        let pane = TerminalPaneState(projectPath: "/tmp")
        #expect(pane.sessionID == pane.id)

        let target = UUID()
        #expect(TerminalSessionAttachment.attach(sessionID: target, to: pane) == .attached)
        #expect(pane.sessionID == target)
    }

    @Test("reports an unchanged pane when it already owns the session")
    func detectsUnchangedPane() {
        let pane = TerminalPaneState(projectPath: "/tmp")
        #expect(TerminalSessionAttachment.attach(sessionID: pane.sessionID, to: pane) == .alreadyAttached)
        #expect(pane.sessionID == pane.id)
    }

    @Test("hands a previous owner back its own session so two panes never share one")
    func releasesPreviousOwner() {
        let shared = UUID()
        let previousOwner = TerminalPaneState(sessionID: shared, projectPath: "/tmp")
        let bystander = TerminalPaneState(projectPath: "/tmp")
        let adopting = TerminalPaneState(projectPath: "/tmp")

        let released = TerminalSessionAttachment.releaseOwners(
            of: shared,
            excluding: adopting.id,
            in: [previousOwner, bystander, adopting]
        )

        #expect(released.map(\.id) == [previousOwner.id])
        #expect(previousOwner.sessionID == previousOwner.id)
        #expect(bystander.sessionID == bystander.id)
    }

    @Test("leaves the adopting pane alone when it already owns the session")
    func skipsTheAdoptingPane() {
        let pane = TerminalPaneState(projectPath: "/tmp")
        let released = TerminalSessionAttachment.releaseOwners(
            of: pane.sessionID,
            excluding: pane.id,
            in: [pane]
        )
        #expect(released.isEmpty)
        #expect(pane.sessionID == pane.id)
    }
}

@Suite("TerminalPaneState session identity")
@MainActor
struct TerminalPaneSessionIdentityTests {
    @Test("defaults the session to the pane itself")
    func defaultsToPaneID() {
        let pane = TerminalPaneState(projectPath: "/tmp")
        #expect(pane.sessionID == pane.id)
    }

    @Test("keeps an adopted session distinct from the pane")
    func keepsAdoptedSession() {
        let adopted = UUID()
        let pane = TerminalPaneState(sessionID: adopted, projectPath: "/tmp")
        #expect(pane.sessionID == adopted)
        #expect(pane.id != adopted)
    }

    @Test("round-trips the session through a tab snapshot")
    func roundTripsThroughSnapshot() {
        let adopted = UUID()
        let tab = TerminalTab(pane: TerminalPaneState(sessionID: adopted, projectPath: "/tmp"))
        let restored = TerminalTab(restoring: tab.snapshot())
        #expect(restored.content.pane?.sessionID == adopted)
    }

    @Test("restores a legacy snapshot with the pane as its own session")
    func restoresLegacySnapshot() {
        let paneID = UUID()
        let snapshot = TerminalTabSnapshot(
            kind: .terminal,
            customTitle: nil,
            colorID: nil,
            isPinned: false,
            projectPath: "/tmp",
            paneTitle: nil,
            paneID: paneID
        )
        let restored = TerminalTab(restoring: snapshot)
        #expect(restored.content.pane?.sessionID == paneID)
    }
}
