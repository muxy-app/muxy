import Foundation
import Testing

@testable import Muxy

@Suite("TerminalOmniboxItemResolver")
struct TerminalOmniboxItemResolverTests {
    @Test("Worktree scope only includes current project worktrees")
    func worktreeScopeUsesActiveProject() {
        let activeProjectID = UUID()
        let otherProjectID = UUID()
        let activeWorktreeID = UUID()
        let otherWorktreeID = UUID()

        let items = TerminalOmniboxItemResolver.items(
            in: TerminalOmniboxItemContext(
                projects: [],
                worktrees: [
                    TerminalOmniboxWorktreeItem(
                        projectID: activeProjectID,
                        worktreeID: activeWorktreeID,
                        name: "main",
                        path: "/tmp/active",
                        branch: "main",
                        isPrimary: true
                    ),
                    TerminalOmniboxWorktreeItem(
                        projectID: otherProjectID,
                        worktreeID: otherWorktreeID,
                        name: "other",
                        path: "/tmp/other",
                        branch: "feature",
                        isPrimary: false
                    ),
                ],
                openTabs: [],
                closedTabs: [],
                commandShortcuts: [],
                activeProjectID: activeProjectID,
                activeWorktreeID: activeWorktreeID,
                commandProjectIDs: []
            ),
            launchScope: .worktrees
        )

        #expect(items == [
            .worktree(TerminalOmniboxWorktreeItem(
                projectID: activeProjectID,
                worktreeID: activeWorktreeID,
                name: "main",
                path: "/tmp/active",
                branch: "main",
                isPrimary: true
            )),
        ])
    }
}
