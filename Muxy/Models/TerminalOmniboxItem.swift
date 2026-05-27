import Foundation

enum TerminalOmniboxLaunchScope: String {
    case projects
    case worktrees
    case openTabs
    case commandShortcuts
    case history
}

struct OpenTerminalTabItem: Identifiable, Equatable {
    let projectID: UUID
    let worktreeID: UUID
    let areaID: UUID
    let tabID: UUID
    let title: String
    let workingDirectory: String?
    let command: String?

    var id: String { "open-\(areaID.uuidString)-\(tabID.uuidString)" }

    var searchKey: String {
        [title, workingDirectory, command].compactMap(\.self).joined(separator: " ")
    }
}

struct TerminalOmniboxProjectItem: Identifiable, Equatable {
    let projectID: UUID
    let name: String
    let path: String

    var id: String { "project-\(projectID.uuidString)" }

    var searchKey: String {
        [name, path, "project"].joined(separator: " ")
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
    case worktree(TerminalOmniboxWorktreeItem)
    case openTab(OpenTerminalTabItem)
    case closedTab(ClosedTerminalTabSnapshot)
    case commandShortcut(CommandShortcut)
    case extensionCommand(ExtensionPaletteItem)

    var id: String {
        switch self {
        case let .project(project):
            project.id
        case let .worktree(wt):
            wt.id
        case let .openTab(tab):
            tab.id
        case let .closedTab(snapshot):
            "closed-\(snapshot.id.uuidString)"
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
        case let .worktree(wt):
            wt.name
        case let .openTab(tab):
            tab.title
        case let .closedTab(snapshot):
            snapshot.title
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
        case let .worktree(wt):
            wt.branch.map { "(\($0)) \(wt.path)" } ?? wt.path
        case let .openTab(tab):
            tab.command ?? tab.workingDirectory
        case let .closedTab(snapshot):
            snapshot.commandToRestore ?? snapshot.workingDirectory
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
        case .worktree:
            "Worktrees"
        case .openTab:
            "Open Tabs"
        case .closedTab:
            "History"
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
        case let .worktree(wt):
            wt.isPrimary ? "folder.badge.gearshape" : "arrow.triangle.branch"
        case .openTab:
            "terminal"
        case .closedTab:
            "clock.arrow.circlepath"
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
        case let .worktree(wt):
            wt.searchKey
        case let .openTab(tab):
            tab.searchKey
        case let .closedTab(snapshot):
            [
                snapshot.title,
                snapshot.workingDirectory,
                snapshot.commandToRestore,
            ].compactMap(\.self).joined(separator: " ")
        case let .commandShortcut(shortcut):
            [shortcut.displayName, shortcut.trimmedCommand].joined(separator: " ")
        case let .extensionCommand(item):
            item.searchKey
        }
    }
}

struct TerminalOmniboxItemContext {
    let projects: [TerminalOmniboxProjectItem]
    let worktrees: [TerminalOmniboxWorktreeItem]
    let openTabs: [OpenTerminalTabItem]
    let closedTabs: [ClosedTerminalTabSnapshot]
    let commandShortcuts: [CommandShortcut]
    let extensionCommands: [ExtensionPaletteItem]
    let activeProjectID: UUID?
    let activeWorktreeID: UUID?
    let commandProjectIDs: Set<UUID>

    init(
        projects: [TerminalOmniboxProjectItem],
        worktrees: [TerminalOmniboxWorktreeItem],
        openTabs: [OpenTerminalTabItem],
        closedTabs: [ClosedTerminalTabSnapshot],
        commandShortcuts: [CommandShortcut],
        extensionCommands: [ExtensionPaletteItem] = [],
        activeProjectID: UUID?,
        activeWorktreeID: UUID?,
        commandProjectIDs: Set<UUID>
    ) {
        self.projects = projects
        self.worktrees = worktrees
        self.openTabs = openTabs
        self.closedTabs = closedTabs
        self.commandShortcuts = commandShortcuts
        self.extensionCommands = extensionCommands
        self.activeProjectID = activeProjectID
        self.activeWorktreeID = activeWorktreeID
        self.commandProjectIDs = commandProjectIDs
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
        case .worktrees:
            guard let activeProjectID = context.activeProjectID else { return [] }
            return context.worktrees
                .filter { $0.projectID == activeProjectID }
                .map(TerminalOmniboxItem.worktree)
        case .openTabs:
            guard let activeProjectID = context.activeProjectID,
                  let activeWorktreeID = context.activeWorktreeID
            else { return [] }
            return context.openTabs
                .filter { $0.projectID == activeProjectID && $0.worktreeID == activeWorktreeID }
                .map(TerminalOmniboxItem.openTab)
        case .commandShortcuts:
            let extensionItems = context.extensionCommands.map(TerminalOmniboxItem.extensionCommand)
            guard context.activeProjectID.map(context.commandProjectIDs.contains) == true else {
                return extensionItems
            }
            let shortcuts = context.commandShortcuts
                .filter { !$0.trimmedCommand.isEmpty }
                .map(TerminalOmniboxItem.commandShortcut)
            return shortcuts + extensionItems
        case .history:
            guard let activeProjectID = context.activeProjectID,
                  let activeWorktreeID = context.activeWorktreeID
            else { return [] }
            return context.closedTabs
                .filter { $0.projectID == activeProjectID && $0.worktreeID == activeWorktreeID }
                .map(TerminalOmniboxItem.closedTab)
        }
    }
}
