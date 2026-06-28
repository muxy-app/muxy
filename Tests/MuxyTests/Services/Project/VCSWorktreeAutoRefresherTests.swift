import Foundation
import Testing

@testable import Muxy

@Suite("VCS worktree auto refresher head changes")
@MainActor
struct VCSWorktreeAutoRefresherTests {
    @Test("worktree.headChanged is gated by worktrees:read")
    func eventIsGated() {
        #expect(MuxyAPI.Permissions.required(forEvent: ExtensionEventName.worktreeHeadChanged) == .worktreesRead)
    }

    @Test("a changed branch is reported")
    func reportsChangedBranch() {
        let id = UUID()
        let changes = VCSWorktreeAutoRefresher.headChanges(before: [id: "main"], after: [id: "feature"])
        #expect(changes == [VCSWorktreeAutoRefresher.HeadChange(worktreeID: id, branch: "feature")])
    }

    @Test("an unchanged branch is not reported")
    func ignoresUnchangedBranch() {
        let id = UUID()
        let changes = VCSWorktreeAutoRefresher.headChanges(before: [id: "main"], after: [id: "main"])
        #expect(changes.isEmpty)
    }

    @Test("a newly seen worktree does not emit on first observation")
    func ignoresFirstObservation() {
        let id = UUID()
        let changes = VCSWorktreeAutoRefresher.headChanges(before: [:], after: [id: "main"])
        #expect(changes.isEmpty)
    }

    @Test("a removed worktree does not emit a change")
    func ignoresRemovedWorktree() {
        let id = UUID()
        let changes = VCSWorktreeAutoRefresher.headChanges(before: [id: "main"], after: [:])
        #expect(changes.isEmpty)
    }
}
