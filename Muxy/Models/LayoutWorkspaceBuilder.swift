import Foundation

@MainActor
enum LayoutWorkspaceBuilder {
    struct PendingCommand {
        let paneID: UUID
        let command: String
    }

    struct Result {
        let root: SplitNode
        let focusedAreaID: UUID
        let pendingCommands: [PendingCommand]
    }

    static func build(config: LayoutConfig, projectPath: String) -> Result? {
        var pending: [PendingCommand] = []
        guard let node = buildNode(from: config.root, projectPath: projectPath, pending: &pending) else {
            return nil
        }
        return Result(root: node, focusedAreaID: firstAreaID(in: node), pendingCommands: pending)
    }

    private static func buildNode(
        from pane: LayoutConfig.Pane,
        projectPath: String,
        pending: inout [PendingCommand]
    ) -> SplitNode? {
        switch pane {
        case let .leaf(tabs):
            return makeArea(tabs: tabs, projectPath: projectPath, pending: &pending).map { .tabArea($0) }
        case let .branch(layout, panes):
            let children = panes.compactMap { buildNode(from: $0, projectPath: projectPath, pending: &pending) }
            guard let first = children.first else { return nil }
            if children.count == 1 { return first }
            let direction: SplitDirection = layout == .horizontal ? .horizontal : .vertical
            return children.dropFirst().reduce(first) { partial, next in
                .split(SplitBranch(direction: direction, first: partial, second: next))
            }
        }
    }

    private static func makeArea(
        tabs: [LayoutConfig.Tab],
        projectPath: String,
        pending: inout [PendingCommand]
    ) -> TabArea? {
        let terminalTabs = tabs.map { makeTab(from: $0, projectPath: projectPath, pending: &pending) }
        guard let firstTab = terminalTabs.first else { return nil }
        let area = TabArea(projectPath: projectPath, existingTab: firstTab)
        for tab in terminalTabs.dropFirst() {
            area.insertExistingTab(tab)
        }
        area.activeTabID = firstTab.id
        return area
    }

    private static func makeTab(
        from tab: LayoutConfig.Tab,
        projectPath: String,
        pending: inout [PendingCommand]
    ) -> TerminalTab {
        let trimmedCommand = tab.command?.trimmingCharacters(in: .whitespacesAndNewlines)
        let resolvedCommand = (trimmedCommand?.isEmpty ?? true) ? nil : trimmedCommand
        let trimmedName = tab.name?.trimmingCharacters(in: .whitespacesAndNewlines)
        let resolvedTitle: String = if let trimmedName, !trimmedName.isEmpty {
            trimmedName
        } else if let resolvedCommand {
            commandTitle(resolvedCommand)
        } else {
            "Terminal"
        }
        let pane = TerminalPaneState(projectPath: projectPath, title: resolvedTitle)
        let terminalTab = TerminalTab(pane: pane)
        if let resolvedCommand {
            pending.append(PendingCommand(paneID: pane.id, command: resolvedCommand))
        }
        return terminalTab
    }

    private static func commandTitle(_ command: String) -> String {
        let trimmed = command.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let first = trimmed.split(separator: " ").first else { return "Terminal" }
        return String(first)
    }

    private static func firstAreaID(in node: SplitNode) -> UUID {
        switch node {
        case let .tabArea(area): area.id
        case let .split(branch): firstAreaID(in: branch.first)
        }
    }
}
