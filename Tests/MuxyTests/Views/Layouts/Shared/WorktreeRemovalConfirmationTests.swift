import Foundation
import Testing
@testable import Muxy

@MainActor
struct WorktreeRemovalConfirmationTests {
    @Test
    func cleanWorktreeConfirmationWarnsAboutDiskRemoval() {
        let worktree = Worktree(name: "feature", path: "/tmp/muxy-feature", branch: "feature", isPrimary: false)

        let confirmation = WorktreeRemovalConfirmation(
            worktree: worktree,
            hasUncommittedChanges: false
        )

        #expect(confirmation.title == "Remove worktree \"feature\"?")
        #expect(confirmation.message == "This will remove the worktree from Muxy and delete its files on disk.")
    }

    @Test
    func dirtyWorktreeConfirmationWarnsAboutDiscardedChanges() {
        let worktree = Worktree(name: "feature", path: "/tmp/muxy-feature", branch: "feature", isPrimary: false)

        let confirmation = WorktreeRemovalConfirmation(
            worktree: worktree,
            hasUncommittedChanges: true
        )

        #expect(confirmation.title == "Remove worktree \"feature\"?")
        #expect(confirmation.message == "This worktree has uncommitted changes. Removing it will permanently discard them.")
    }

    @Test
    func inspectionStartsOnlyWithoutPendingRemovalWork() {
        #expect(WorktreeRemovalRequestPolicy.canStartInspection(
            hasPendingConfirmation: false,
            isInspecting: false,
            isRemoving: false
        ))
        #expect(!WorktreeRemovalRequestPolicy.canStartInspection(
            hasPendingConfirmation: true,
            isInspecting: false,
            isRemoving: false
        ))
        #expect(!WorktreeRemovalRequestPolicy.canStartInspection(
            hasPendingConfirmation: false,
            isInspecting: true,
            isRemoving: false
        ))
        #expect(!WorktreeRemovalRequestPolicy.canStartInspection(
            hasPendingConfirmation: false,
            isInspecting: false,
            isRemoving: true
        ))
    }

    @Test
    func confirmationRequiresTheCurrentRegisteredWorktree() {
        let projectID = UUID()
        let expected = WorktreeKey(projectID: projectID, worktreeID: UUID())
        let other = WorktreeKey(projectID: projectID, worktreeID: UUID())

        #expect(WorktreeRemovalRequestPolicy.canPresentConfirmation(.init(
            expected: expected,
            current: expected,
            isRegistered: true,
            isPreparing: true,
            isRemoving: false,
            hasPendingConfirmation: false
        )))
        #expect(!WorktreeRemovalRequestPolicy.canPresentConfirmation(.init(
            expected: expected,
            current: other,
            isRegistered: true,
            isPreparing: true,
            isRemoving: false,
            hasPendingConfirmation: false
        )))
        #expect(!WorktreeRemovalRequestPolicy.canPresentConfirmation(.init(
            expected: expected,
            current: expected,
            isRegistered: false,
            isPreparing: true,
            isRemoving: false,
            hasPendingConfirmation: false
        )))
    }

    @Test
    func confirmationRejectsConcurrentRemovalWork() {
        let expected = WorktreeKey(projectID: UUID(), worktreeID: UUID())

        #expect(!WorktreeRemovalRequestPolicy.canPresentConfirmation(.init(
            expected: expected,
            current: expected,
            isRegistered: true,
            isPreparing: true,
            isRemoving: true,
            hasPendingConfirmation: false
        )))
        #expect(!WorktreeRemovalRequestPolicy.canPresentConfirmation(.init(
            expected: expected,
            current: expected,
            isRegistered: true,
            isPreparing: true,
            isRemoving: false,
            hasPendingConfirmation: true
        )))
        #expect(!WorktreeRemovalRequestPolicy.canPresentConfirmation(.init(
            expected: expected,
            current: expected,
            isRegistered: true,
            isPreparing: false,
            isRemoving: false,
            hasPendingConfirmation: false
        )))
    }
}
