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

        let project = TerminalOmniboxProjectItem(projectID: projectID, name: "Muxy", path: "/repo/muxy")
        let recentlyRemovedProject = TerminalOmniboxRecentlyRemovedProjectItem(
            projectID: projectID,
            name: "Muxy",
            path: "/repo/muxy",
            icon: "shippingbox"
        )
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
            command: "npm run dev",
            projectName: "Muxy",
            worktreeName: "main",
            worktreeBranch: "main"
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

        #expect(TerminalOmniboxItem.recentlyRemovedProject(recentlyRemovedProject).id ==
            "recent-project-\(projectID.uuidString)")
        #expect(TerminalOmniboxItem.recentlyRemovedProject(recentlyRemovedProject).title == "Muxy")
        #expect(TerminalOmniboxItem.recentlyRemovedProject(recentlyRemovedProject).subtitle == "/repo/muxy")
        #expect(TerminalOmniboxItem.recentlyRemovedProject(recentlyRemovedProject).sectionTitle == "Recently Removed")
        #expect(TerminalOmniboxItem.recentlyRemovedProject(recentlyRemovedProject).symbol == "shippingbox")
        #expect(TerminalOmniboxItem.recentlyRemovedProject(recentlyRemovedProject).searchKey ==
            "Muxy /repo/muxy recently removed project")

        #expect(TerminalOmniboxItem.worktree(primaryWorktree).subtitle == "(main) /repo/muxy")
        #expect(TerminalOmniboxItem.worktree(primaryWorktree).symbol == "folder.badge.gearshape")
        #expect(TerminalOmniboxItem.worktree(secondaryWorktree).subtitle == "/repo/muxy-feature")
        #expect(TerminalOmniboxItem.worktree(secondaryWorktree).symbol == "arrow.triangle.branch")
        #expect(TerminalOmniboxItem.worktree(primaryWorktree).sectionTitle == "Worktrees")

        #expect(TerminalOmniboxItem.openTab(openTab).id == "open-\(areaID.uuidString)-\(tabID.uuidString)")
        #expect(TerminalOmniboxItem.openTab(openTab).subtitle == "npm run dev")
        #expect(TerminalOmniboxItem.openTab(openTab).sectionTitle == "Open Tabs — Muxy (main)")
        #expect(TerminalOmniboxItem.openTab(openTab).symbol == "terminal")

        let namedWorkspace = TerminalOmniboxWorkspaceItem(groupID: UUID(), name: "Backend", projectCount: 3)
        let allWorkspaces = TerminalOmniboxWorkspaceItem(groupID: nil, name: "All Projects", projectCount: 7)
        #expect(TerminalOmniboxItem.workspace(namedWorkspace).title == "Backend")
        #expect(TerminalOmniboxItem.workspace(namedWorkspace).subtitle == "3 projects")
        #expect(TerminalOmniboxItem.workspace(namedWorkspace).symbol == "square.stack.3d.up")
        #expect(TerminalOmniboxItem.workspace(namedWorkspace).sectionTitle == "Workspaces")
        #expect(TerminalOmniboxItem.workspace(allWorkspaces).id == "workspace-all")
        #expect(TerminalOmniboxItem.workspace(allWorkspaces).subtitle == "All projects")
        #expect(TerminalOmniboxItem.workspace(allWorkspaces).symbol == "square.grid.2x2")
        #expect(TerminalOmniboxItem
            .workspace(TerminalOmniboxWorkspaceItem(groupID: UUID(), name: "Solo", projectCount: 1)).subtitle == "1 project")

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
        let recentlyRemovedProject = TerminalOmniboxRecentlyRemovedProjectItem(
            projectID: otherProjectID,
            name: "Removed",
            path: "/repo/removed",
            icon: nil
        )
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
            command: nil,
            projectName: "Muxy",
            worktreeName: "main",
            worktreeBranch: "main"
        )
        let otherOpenTab = OpenTerminalTabItem(
            projectID: activeProjectID,
            worktreeID: otherWorktreeID,
            areaID: UUID(),
            tabID: UUID(),
            title: "Other",
            workingDirectory: "/repo/muxy-other",
            command: nil,
            projectName: "Muxy",
            worktreeName: "other",
            worktreeBranch: nil
        )
        let shortcut = CommandShortcut(name: "Build", command: "swift build")
        let emptyShortcut = CommandShortcut(name: "Empty", command: "   ")
        let context = TerminalOmniboxItemContext(
            projects: [project],
            recentlyRemovedProjects: [recentlyRemovedProject],
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
            commandShortcuts: [shortcut, emptyShortcut],
            activeProjectID: activeProjectID,
            activeWorktreeID: activeWorktreeID,
            commandProjectIDs: [activeProjectID]
        )

        #expect(TerminalOmniboxItemResolver.items(in: context, launchScope: .projects) == [.project(project)])
        #expect(TerminalOmniboxItemResolver.items(in: context, launchScope: .recentlyRemovedProjects) ==
            [.recentlyRemovedProject(recentlyRemovedProject)])
        #expect(TerminalOmniboxItemResolver.items(in: context, launchScope: .worktrees) == [.worktree(activeWorktree)])
        #expect(TerminalOmniboxItemResolver.items(in: context, launchScope: .openTabs) ==
            [.openTab(activeOpenTab), .openTab(otherOpenTab)])
        #expect(TerminalOmniboxItemResolver.items(in: context, launchScope: .commandShortcuts) == [.commandShortcut(shortcut)])

        let inactiveContext = TerminalOmniboxItemContext(
            projects: [project],
            worktrees: [activeWorktree],
            openTabs: [activeOpenTab],
            commandShortcuts: [shortcut],
            activeProjectID: nil,
            activeWorktreeID: nil,
            commandProjectIDs: [activeProjectID]
        )

        #expect(TerminalOmniboxItemResolver.items(in: inactiveContext, launchScope: .worktrees).isEmpty)
        #expect(TerminalOmniboxItemResolver.items(in: inactiveContext, launchScope: .openTabs) == [.openTab(activeOpenTab)])
        #expect(TerminalOmniboxItemResolver.items(in: inactiveContext, launchScope: .commandShortcuts).isEmpty)
    }

    @Test("Recently Removed Projects scope preserves stored order and excludes unrelated items")
    func recentlyRemovedProjectsScopePreservesOrder() {
        let first = TerminalOmniboxRecentlyRemovedProjectItem(
            projectID: UUID(),
            name: "Newest",
            path: "/repo/newest",
            icon: nil
        )
        let second = TerminalOmniboxRecentlyRemovedProjectItem(
            projectID: UUID(),
            name: "Older",
            path: "/repo/older",
            icon: "archivebox"
        )
        let context = TerminalOmniboxItemContext(
            projects: [TerminalOmniboxProjectItem(projectID: UUID(), name: "Active", path: "/repo/active")],
            recentlyRemovedProjects: [first, second],
            worktrees: [],
            openTabs: [],
            commandShortcuts: [],
            activeProjectID: nil,
            activeWorktreeID: nil,
            commandProjectIDs: []
        )

        let items = TerminalOmniboxItemResolver.items(in: context, launchScope: .recentlyRemovedProjects)

        #expect(items == [.recentlyRemovedProject(first), .recentlyRemovedProject(second)])
    }

    @Test("Open tabs scope includes every project and worktree with the active one first")
    func openTabsScopeIncludesAllProjectsActiveFirst() {
        let activeProjectID = UUID()
        let otherProjectID = UUID()
        let activeWorktreeID = UUID()
        let otherWorktreeID = UUID()

        func tab(project: UUID, worktree: UUID, name: String) -> OpenTerminalTabItem {
            OpenTerminalTabItem(
                projectID: project,
                worktreeID: worktree,
                areaID: UUID(),
                tabID: UUID(),
                title: name,
                workingDirectory: nil,
                command: nil,
                projectName: name,
                worktreeName: name,
                worktreeBranch: name
            )
        }

        let otherProjectTab = tab(project: otherProjectID, worktree: otherWorktreeID, name: "other")
        let activeWorktreeTab = tab(project: activeProjectID, worktree: activeWorktreeID, name: "active")
        let otherWorktreeTab = tab(project: activeProjectID, worktree: otherWorktreeID, name: "secondary")

        let context = TerminalOmniboxItemContext(
            projects: [],
            worktrees: [],
            openTabs: [otherProjectTab, otherWorktreeTab, activeWorktreeTab],
            commandShortcuts: [],
            activeProjectID: activeProjectID,
            activeWorktreeID: activeWorktreeID,
            commandProjectIDs: []
        )

        let items = TerminalOmniboxItemResolver.items(in: context, launchScope: .openTabs)
        #expect(items.first == .openTab(activeWorktreeTab))
        #expect(items.count == 3)
        #expect(items.contains(.openTab(otherProjectTab)))
        #expect(items.contains(.openTab(otherWorktreeTab)))
    }

    @Test("Workspaces scope returns every workspace item")
    func workspacesScopeReturnsAllWorkspaces() {
        let allProjects = TerminalOmniboxWorkspaceItem(groupID: nil, name: "All Projects", projectCount: 5)
        let backend = TerminalOmniboxWorkspaceItem(groupID: UUID(), name: "Backend", projectCount: 2)
        let context = TerminalOmniboxItemContext(
            projects: [],
            worktrees: [],
            workspaces: [allProjects, backend],
            openTabs: [],
            commandShortcuts: [],
            activeProjectID: nil,
            activeWorktreeID: nil,
            commandProjectIDs: []
        )

        let items = TerminalOmniboxItemResolver.items(in: context, launchScope: .workspaces)
        #expect(items == [.workspace(allProjects), .workspace(backend)])
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
