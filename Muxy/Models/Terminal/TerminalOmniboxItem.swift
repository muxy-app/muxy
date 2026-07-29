import Foundation

enum TerminalOmniboxLaunchScope: String {
    case projects
    case recentlyRemovedProjects
    case worktrees
    case workspaces
    case openTabs
    case commandShortcuts
}

struct OpenTerminalTabItem: Identifiable, Equatable {
    let projectID: UUID
    let worktreeID: UUID
    let areaID: UUID
    let tabID: UUID
    let title: String
    let workingDirectory: String?
    let command: String?
    let projectName: String
    let projectIsHome: Bool
    let worktreeName: String?
    let worktreeBranch: String?

    var id: String { "open-\(areaID.uuidString)-\(tabID.uuidString)" }

    init(
        projectID: UUID,
        worktreeID: UUID,
        areaID: UUID,
        tabID: UUID,
        title: String,
        workingDirectory: String?,
        command: String?,
        projectName: String,
        projectIsHome: Bool = false,
        worktreeName: String?,
        worktreeBranch: String?
    ) {
        self.projectID = projectID
        self.worktreeID = worktreeID
        self.areaID = areaID
        self.tabID = tabID
        self.title = title
        self.workingDirectory = workingDirectory
        self.command = command
        self.projectName = projectName
        self.projectIsHome = projectIsHome
        self.worktreeName = worktreeName
        self.worktreeBranch = worktreeBranch
    }

    var sectionTitle: String {
        let context = worktreeBranch ?? worktreeName
        guard let context, !context.isEmpty else { return "Open Tabs — \(projectName)" }
        return "Open Tabs — \(projectName) (\(context))"
    }

    var searchKey: String {
        [title, workingDirectory, command, projectName, worktreeName, worktreeBranch]
            .compactMap(\.self)
            .joined(separator: " ")
    }
}

struct TerminalOmniboxProjectItem: Identifiable, Equatable {
    let projectID: UUID
    let name: String
    let path: String
    let isHome: Bool

    var id: String { "project-\(projectID.uuidString)" }

    init(projectID: UUID, name: String, path: String, isHome: Bool = false) {
        self.projectID = projectID
        self.name = name
        self.path = path
        self.isHome = isHome
    }

    var searchKey: String {
        [name, path, "project"].joined(separator: " ")
    }
}

struct TerminalOmniboxRecentlyRemovedProjectItem: Identifiable, Equatable {
    let projectID: UUID
    let name: String
    let path: String
    let icon: String?

    var id: String { "recent-project-\(projectID.uuidString)" }

    var searchKey: String {
        [name, path, "recently removed project"].joined(separator: " ")
    }
}

struct TerminalOmniboxWorktreeItem: Identifiable, Equatable {
    let projectID: UUID
    let worktreeID: UUID
    let name: String
    let path: String
    let branch: String?
    let isPrimary: Bool

    var id: String { "worktree-\(worktreeID.uuidString)" }

    var searchKey: String {
        [name, path, branch ?? "", "worktree"].joined(separator: " ")
    }
}

struct TerminalOmniboxWorkspaceItem: Identifiable, Equatable {
    let groupID: UUID?
    let name: String
    let projectCount: Int

    var id: String { "workspace-\(groupID?.uuidString ?? "all")" }

    var searchKey: String {
        [name, "workspace"].joined(separator: " ")
    }
}

struct ExtensionPaletteItem: Identifiable, Equatable {
    let extensionID: String
    let extensionName: String
    let command: ExtensionPaletteCommand

    var id: String { "ext-\(extensionID)-\(command.id)" }

    var searchKey: String {
        [extensionName, command.title, command.subtitle ?? ""].joined(separator: " ")
    }
}

enum TerminalOmniboxItem: Identifiable, Equatable {
    case project(TerminalOmniboxProjectItem)
    case recentlyRemovedProject(TerminalOmniboxRecentlyRemovedProjectItem)
    case worktree(TerminalOmniboxWorktreeItem)
    case workspace(TerminalOmniboxWorkspaceItem)
    case openTab(OpenTerminalTabItem)
    case commandShortcut(CommandShortcut)
    case extensionCommand(ExtensionPaletteItem)

    var id: String {
        switch self {
        case let .project(project):
            project.id
        case let .recentlyRemovedProject(project):
            project.id
        case let .worktree(wt):
            wt.id
        case let .workspace(workspace):
            workspace.id
        case let .openTab(tab):
            tab.id
        case let .commandShortcut(shortcut):
            "shortcut-\(shortcut.id.uuidString)"
        case let .extensionCommand(item):
            item.id
        }
    }

    var title: String {
        switch self {
        case let .project(project):
            project.name
        case let .recentlyRemovedProject(project):
            project.name
        case let .worktree(wt):
            wt.name
        case let .workspace(workspace):
            workspace.name
        case let .openTab(tab):
            tab.title
        case let .commandShortcut(shortcut):
            shortcut.displayName
        case let .extensionCommand(item):
            item.command.title
        }
    }

    var subtitle: String? {
        switch self {
        case let .project(project):
            project.path
        case let .recentlyRemovedProject(project):
            project.path
        case let .worktree(wt):
            wt.branch.map { "(\($0)) \(wt.path)" } ?? wt.path
        case let .workspace(workspace):
            workspace.groupID == nil
                ? "All projects"
                : "\(workspace.projectCount) project\(workspace.projectCount == 1 ? "" : "s")"
        case let .openTab(tab):
            tab.command ?? tab.workingDirectory
        case let .commandShortcut(shortcut):
            shortcut.trimmedCommand
        case let .extensionCommand(item):
            item.command.subtitle ?? item.extensionName
        }
    }

    var sectionTitle: String {
        switch self {
        case .project:
            "Projects"
        case .recentlyRemovedProject:
            "Recently Removed"
        case .worktree:
            "Worktrees"
        case .workspace:
            "Workspaces"
        case let .openTab(tab):
            tab.sectionTitle
        case .commandShortcut:
            "Custom Commands"
        case .extensionCommand:
            "Extension Commands"
        }
    }

    var symbol: String {
        switch self {
        case .project:
            "folder"
        case let .recentlyRemovedProject(project):
            project.icon ?? "folder"
        case let .worktree(wt):
            wt.isPrimary ? "folder.badge.gearshape" : "arrow.triangle.branch"
        case let .workspace(workspace):
            workspace.groupID == nil ? "square.grid.2x2" : "square.stack.3d.up"
        case .openTab:
            "terminal"
        case .commandShortcut:
            "command"
        case .extensionCommand:
            "puzzlepiece.extension"
        }
    }

    var searchKey: String {
        switch self {
        case let .project(project):
            project.searchKey
        case let .recentlyRemovedProject(project):
            project.searchKey
        case let .worktree(wt):
            wt.searchKey
        case let .workspace(workspace):
            workspace.searchKey
        case let .openTab(tab):
            tab.searchKey
        case let .commandShortcut(shortcut):
            [shortcut.displayName, shortcut.trimmedCommand].joined(separator: " ")
        case let .extensionCommand(item):
            item.searchKey
        }
    }
}

@MainActor
extension TerminalOmniboxItem {
    func localizedTitle(localization: LocalizationService = .shared) -> String {
        switch self {
        case let .project(project):
            project.isHome ? localization.searchString(key: Project.homeName) : project.name
        case let .workspace(workspace):
            workspace.groupID == nil ? localization.searchString(key: "All Projects") : workspace.name
        case let .commandShortcut(shortcut):
            shortcut.localizedDisplayName(using: localization)
        default:
            title
        }
    }

    func localizedSubtitle(localization: LocalizationService = .shared) -> String? {
        guard case let .workspace(workspace) = self else { return subtitle }
        guard workspace.groupID != nil else {
            return localization.searchString(key: "All projects")
        }
        guard workspace.projectCount != 1 else {
            return localization.string("\(workspace.projectCount) project")
        }
        return localization.string("\(workspace.projectCount) projects")
    }

    func localizedSectionTitle(localization: LocalizationService = .shared) -> String {
        switch self {
        case .project:
            return localization.searchString(key: "Projects")
        case .recentlyRemovedProject:
            return localization.searchString(key: "Recently Removed")
        case .worktree:
            return localization.searchString(key: "Worktrees")
        case .workspace:
            return localization.searchString(key: "Workspaces")
        case let .openTab(tab):
            let projectName = tab.projectIsHome
                ? localization.searchString(key: Project.homeName)
                : tab.projectName
            let context = tab.worktreeBranch ?? tab.worktreeName
            guard let context, !context.isEmpty else {
                return localization.string("Open Tabs — \(projectName)")
            }
            return localization.string("Open Tabs — \(projectName) (\(context))")
        case .commandShortcut:
            return localization.searchString(key: "Custom Commands")
        case .extensionCommand:
            return localization.searchString(key: "Extension Commands")
        }
    }

    func matchesSearch(
        query: String,
        localization: LocalizationService = .shared
    ) -> Bool {
        LocalizedSearch.matches(
            query: query,
            localizedKeys: localizedSearchKeys,
            verbatimValues: [
                searchKey,
                localizedTitle(localization: localization),
                localizedSubtitle(localization: localization),
                localizedSectionTitle(localization: localization),
            ].compactMap(\.self),
            localization: localization
        )
    }

    private var localizedSearchKeys: [String] {
        switch self {
        case .project:
            ["project"]
        case .recentlyRemovedProject:
            ["recently removed project"]
        case .worktree:
            ["worktree"]
        case .workspace:
            ["workspace"]
        case .openTab:
            ["open tab"]
        case .commandShortcut:
            ["custom command"]
        case .extensionCommand:
            ["extension command"]
        }
    }
}

struct TerminalOmniboxItemContext {
    let projects: [TerminalOmniboxProjectItem]
    let recentlyRemovedProjects: [TerminalOmniboxRecentlyRemovedProjectItem]
    let worktrees: [TerminalOmniboxWorktreeItem]
    let workspaces: [TerminalOmniboxWorkspaceItem]
    let openTabs: [OpenTerminalTabItem]
    let commandShortcuts: [CommandShortcut]
    let extensionCommands: [ExtensionPaletteItem]
    let activeProjectID: UUID?
    let activeWorktreeID: UUID?
    let commandProjectIDs: Set<UUID>

    init(
        projects: [TerminalOmniboxProjectItem],
        recentlyRemovedProjects: [TerminalOmniboxRecentlyRemovedProjectItem] = [],
        worktrees: [TerminalOmniboxWorktreeItem],
        workspaces: [TerminalOmniboxWorkspaceItem] = [],
        openTabs: [OpenTerminalTabItem],
        commandShortcuts: [CommandShortcut],
        extensionCommands: [ExtensionPaletteItem] = [],
        activeProjectID: UUID?,
        activeWorktreeID: UUID?,
        commandProjectIDs: Set<UUID>
    ) {
        self.projects = projects
        self.recentlyRemovedProjects = recentlyRemovedProjects
        self.worktrees = worktrees
        self.workspaces = workspaces
        self.openTabs = openTabs
        self.commandShortcuts = commandShortcuts
        self.extensionCommands = extensionCommands
        self.activeProjectID = activeProjectID
        self.activeWorktreeID = activeWorktreeID
        self.commandProjectIDs = commandProjectIDs
    }

    func isActiveWorktree(_ tab: OpenTerminalTabItem) -> Bool {
        tab.projectID == activeProjectID && tab.worktreeID == activeWorktreeID
    }
}

enum TerminalOmniboxItemResolver {
    static func items(
        in context: TerminalOmniboxItemContext,
        launchScope: TerminalOmniboxLaunchScope
    ) -> [TerminalOmniboxItem] {
        switch launchScope {
        case .projects:
            return context.projects.map(TerminalOmniboxItem.project)
        case .recentlyRemovedProjects:
            return context.recentlyRemovedProjects.map(TerminalOmniboxItem.recentlyRemovedProject)
        case .worktrees:
            guard let activeProjectID = context.activeProjectID else { return [] }
            return context.worktrees
                .filter { $0.projectID == activeProjectID }
                .map(TerminalOmniboxItem.worktree)
        case .workspaces:
            return context.workspaces.map(TerminalOmniboxItem.workspace)
        case .openTabs:
            return context.openTabs
                .enumerated()
                .sorted { lhs, rhs in
                    let lhsActive = context.isActiveWorktree(lhs.element)
                    let rhsActive = context.isActiveWorktree(rhs.element)
                    if lhsActive != rhsActive {
                        return lhsActive
                    }
                    return lhs.offset < rhs.offset
                }
                .map { TerminalOmniboxItem.openTab($0.element) }
        case .commandShortcuts:
            let extensionItems = context.extensionCommands.map(TerminalOmniboxItem.extensionCommand)
            guard context.activeProjectID.map(context.commandProjectIDs.contains) == true else {
                return extensionItems
            }
            let shortcuts = context.commandShortcuts
                .filter { !$0.trimmedCommand.isEmpty }
                .map(TerminalOmniboxItem.commandShortcut)
            return shortcuts + extensionItems
        }
    }
}
