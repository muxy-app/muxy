import Foundation
import Testing

@testable import Muxy

@Suite("TerminalOmniboxItemResolver")
struct TerminalOmniboxItemResolverTests {
    @Test("Item metadata reflects each omnibox variant")
    func itemMetadataReflectsVariant() {
        let projectID = UUID(uuidString: "00000000-0000-0000-0000-000000000001")!
        let worktreeID = UUID(uuidString: "00000000-0000-0000-0000-000000000002")!
        let areaID = UUID(uuidString: "00000000-0000-0000-0000-000000000003")!
        let tabID = UUID(uuidString: "00000000-0000-0000-0000-000000000004")!
        let shortcutID = UUID(uuidString: "00000000-0000-0000-0000-000000000005")!
        let closedID = UUID(uuidString: "00000000-0000-0000-0000-000000000006")!

        let project = TerminalOmniboxProjectItem(projectID: projectID, name: "Muxy", path: "/repo/muxy")
        let primaryWorktree = TerminalOmniboxWorktreeItem(
            projectID: projectID,
            worktreeID: worktreeID,
            name: "main",
            path: "/repo/muxy",
            branch: "main",
            isPrimary: true
        )
        let secondaryWorktree = TerminalOmniboxWorktreeItem(
            projectID: projectID,
            worktreeID: UUID(),
            name: "feature",
            path: "/repo/muxy-feature",
            branch: nil,
            isPrimary: false
        )
        let openTab = OpenTerminalTabItem(
            projectID: projectID,
            worktreeID: worktreeID,
            areaID: areaID,
            tabID: tabID,
            title: "Server",
            workingDirectory: "/repo/muxy",
            command: "npm run dev"
        )
        let closedTab = ClosedTerminalTabSnapshot(
            id: closedID,
            projectID: projectID,
            worktreeID: worktreeID,
            areaID: areaID,
            projectPath: "/repo/muxy",
            title: "Closed",
            customTitle: nil,
            colorID: nil,
            workingDirectory: "/repo/muxy",
            startupCommand: "swift test",
            lastSubmittedCommand: "swift build",
            closedSequence: 1,
            closedAt: Date(timeIntervalSince1970: 0)
        )
        let shortcut = CommandShortcut(
            id: shortcutID,
            name: "  Run Tests  ",
            command: " swift test ",
            combo: KeyCombo(key: "t", command: true)
        )

        #expect(TerminalOmniboxItem.project(project).id == "project-\(projectID.uuidString)")
        #expect(TerminalOmniboxItem.project(project).title == "Muxy")
        #expect(TerminalOmniboxItem.project(project).subtitle == "/repo/muxy")
        #expect(TerminalOmniboxItem.project(project).sectionTitle == "Projects")
        #expect(TerminalOmniboxItem.project(project).symbol == "folder")
        #expect(TerminalOmniboxItem.project(project).searchKey == "Muxy /repo/muxy project")

        #expect(TerminalOmniboxItem.worktree(primaryWorktree).subtitle == "(main) /repo/muxy")
        #expect(TerminalOmniboxItem.worktree(primaryWorktree).symbol == "folder.badge.gearshape")
        #expect(TerminalOmniboxItem.worktree(secondaryWorktree).subtitle == "/repo/muxy-feature")
        #expect(TerminalOmniboxItem.worktree(secondaryWorktree).symbol == "arrow.triangle.branch")
        #expect(TerminalOmniboxItem.worktree(primaryWorktree).sectionTitle == "Worktrees")

        #expect(TerminalOmniboxItem.openTab(openTab).id == "open-\(areaID.uuidString)-\(tabID.uuidString)")
        #expect(TerminalOmniboxItem.openTab(openTab).subtitle == "npm run dev")
        #expect(TerminalOmniboxItem.openTab(openTab).sectionTitle == "Open Tabs")
        #expect(TerminalOmniboxItem.openTab(openTab).symbol == "terminal")

        #expect(TerminalOmniboxItem.closedTab(closedTab).id == "closed-\(closedID.uuidString)")
        #expect(TerminalOmniboxItem.closedTab(closedTab).subtitle == "swift build")
        #expect(TerminalOmniboxItem.closedTab(closedTab).sectionTitle == "History")
        #expect(TerminalOmniboxItem.closedTab(closedTab).symbol == "clock.arrow.circlepath")
        #expect(TerminalOmniboxItem.closedTab(closedTab).searchKey == "Closed /repo/muxy swift build")

        #expect(TerminalOmniboxItem.commandShortcut(shortcut).id == "shortcut-\(shortcutID.uuidString)")
        #expect(TerminalOmniboxItem.commandShortcut(shortcut).title == "Run Tests")
        #expect(TerminalOmniboxItem.commandShortcut(shortcut).subtitle == "swift test")
        #expect(TerminalOmniboxItem.commandShortcut(shortcut).sectionTitle == "Custom Commands")
        #expect(TerminalOmniboxItem.commandShortcut(shortcut).symbol == "command")
    }

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

    @Test("Resolver returns expected items for every launch scope")
    func resolverReturnsItemsForEveryLaunchScope() {
        let activeProjectID = UUID()
        let otherProjectID = UUID()
        let activeWorktreeID = UUID()
        let otherWorktreeID = UUID()
        let project = TerminalOmniboxProjectItem(projectID: activeProjectID, name: "Muxy", path: "/repo/muxy")
        let activeWorktree = TerminalOmniboxWorktreeItem(
            projectID: activeProjectID,
            worktreeID: activeWorktreeID,
            name: "main",
            path: "/repo/muxy",
            branch: "main",
            isPrimary: true
        )
        let activeOpenTab = OpenTerminalTabItem(
            projectID: activeProjectID,
            worktreeID: activeWorktreeID,
            areaID: UUID(),
            tabID: UUID(),
            title: "Active",
            workingDirectory: "/repo/muxy",
            command: nil
        )
        let otherOpenTab = OpenTerminalTabItem(
            projectID: activeProjectID,
            worktreeID: otherWorktreeID,
            areaID: UUID(),
            tabID: UUID(),
            title: "Other",
            workingDirectory: "/repo/muxy-other",
            command: nil
        )
        let closedTab = ClosedTerminalTabSnapshot(
            id: UUID(),
            projectID: activeProjectID,
            worktreeID: activeWorktreeID,
            areaID: UUID(),
            projectPath: "/repo/muxy",
            title: "Closed",
            customTitle: nil,
            colorID: nil,
            workingDirectory: "/repo/muxy",
            startupCommand: nil,
            lastSubmittedCommand: nil,
            closedSequence: 1,
            closedAt: Date()
        )
        let shortcut = CommandShortcut(name: "Build", command: "swift build")
        let emptyShortcut = CommandShortcut(name: "Empty", command: "   ")
        let context = TerminalOmniboxItemContext(
            projects: [project],
            worktrees: [
                activeWorktree,
                TerminalOmniboxWorktreeItem(
                    projectID: otherProjectID,
                    worktreeID: UUID(),
                    name: "other",
                    path: "/repo/other",
                    branch: nil,
                    isPrimary: false
                ),
            ],
            openTabs: [activeOpenTab, otherOpenTab],
            closedTabs: [
                closedTab,
                ClosedTerminalTabSnapshot(
                    id: UUID(),
                    projectID: otherProjectID,
                    worktreeID: activeWorktreeID,
                    areaID: UUID(),
                    projectPath: "/repo/other",
                    title: "Other Closed",
                    customTitle: nil,
                    colorID: nil,
                    workingDirectory: "/repo/other",
                    startupCommand: nil,
                    lastSubmittedCommand: nil,
                    closedSequence: 2,
                    closedAt: Date()
                ),
            ],
            commandShortcuts: [shortcut, emptyShortcut],
            activeProjectID: activeProjectID,
            activeWorktreeID: activeWorktreeID,
            commandProjectIDs: [activeProjectID]
        )

        #expect(TerminalOmniboxItemResolver.items(in: context, launchScope: .projects) == [.project(project)])
        #expect(TerminalOmniboxItemResolver.items(in: context, launchScope: .worktrees) == [.worktree(activeWorktree)])
        #expect(TerminalOmniboxItemResolver.items(in: context, launchScope: .openTabs) == [.openTab(activeOpenTab)])
        #expect(TerminalOmniboxItemResolver.items(in: context, launchScope: .history) == [.closedTab(closedTab)])
        #expect(TerminalOmniboxItemResolver.items(in: context, launchScope: .commandShortcuts) == [.commandShortcut(shortcut)])

        let inactiveContext = TerminalOmniboxItemContext(
            projects: [project],
            worktrees: [activeWorktree],
            openTabs: [activeOpenTab],
            closedTabs: [closedTab],
            commandShortcuts: [shortcut],
            activeProjectID: nil,
            activeWorktreeID: nil,
            commandProjectIDs: [activeProjectID]
        )

        #expect(TerminalOmniboxItemResolver.items(in: inactiveContext, launchScope: .worktrees).isEmpty)
        #expect(TerminalOmniboxItemResolver.items(in: inactiveContext, launchScope: .openTabs).isEmpty)
        #expect(TerminalOmniboxItemResolver.items(in: inactiveContext, launchScope: .history).isEmpty)
        #expect(TerminalOmniboxItemResolver.items(in: inactiveContext, launchScope: .commandShortcuts).isEmpty)
    }

    @Test("commandShortcuts scope includes extension commands")
    func commandShortcutsScopeIncludesExtensionCommands() {
        let projectID = UUID()
        let shortcut = CommandShortcut(name: "Server", command: "npm run dev")
        let extensionItem = ExtensionPaletteItem(
            extensionID: "demo",
            extensionName: "Demo",
            command: ExtensionPaletteCommand(id: "do", title: "Do thing", subtitle: nil)
        )
        let context = TerminalOmniboxItemContext(
            projects: [],
            worktrees: [],
            openTabs: [],
            closedTabs: [],
            commandShortcuts: [shortcut],
            extensionCommands: [extensionItem],
            activeProjectID: projectID,
            activeWorktreeID: nil,
            commandProjectIDs: [projectID]
        )

        let items = TerminalOmniboxItemResolver.items(in: context, launchScope: .commandShortcuts)
        #expect(items == [.commandShortcut(shortcut), .extensionCommand(extensionItem)])
    }

    @Test("commandShortcuts scope returns extension commands even when project has no shortcuts allowed")
    func commandShortcutsScopeWithoutActiveProject() {
        let extensionItem = ExtensionPaletteItem(
            extensionID: "demo",
            extensionName: "Demo",
            command: ExtensionPaletteCommand(id: "do", title: "Do thing", subtitle: nil)
        )
        let context = TerminalOmniboxItemContext(
            projects: [],
            worktrees: [],
            openTabs: [],
            closedTabs: [],
            commandShortcuts: [CommandShortcut(name: "Server", command: "npm run dev")],
            extensionCommands: [extensionItem],
            activeProjectID: nil,
            activeWorktreeID: nil,
            commandProjectIDs: []
        )

        let items = TerminalOmniboxItemResolver.items(in: context, launchScope: .commandShortcuts)
        #expect(items == [.extensionCommand(extensionItem)])
    }

    @Test("extensionCommand exposes title, subtitle and section")
    func extensionCommandItemAccessors() {
        let item = ExtensionPaletteItem(
            extensionID: "demo",
            extensionName: "Demo",
            command: ExtensionPaletteCommand(id: "run", title: "Run", subtitle: "subtitle")
        )
        let omniboxItem = TerminalOmniboxItem.extensionCommand(item)

        #expect(omniboxItem.title == "Run")
        #expect(omniboxItem.subtitle == "subtitle")
        #expect(omniboxItem.sectionTitle == "Extension Commands")
        #expect(omniboxItem.symbol == "puzzlepiece.extension")
        #expect(omniboxItem.searchKey.contains("Demo"))
        #expect(omniboxItem.id == "ext-demo-run")
    }
}
