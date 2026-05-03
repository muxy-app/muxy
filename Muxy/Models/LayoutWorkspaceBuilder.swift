import Foundation

@MainActor
enum LayoutWorkspaceBuilder {
    struct Result {
        let root: SplitNode
        let focusedAreaID: UUID
    }

    static func build(config: LayoutConfig, projectPath: String) -> Result? {
        guard let node = buildNode(from: config.root, projectPath: projectPath) else {
            return nil
        }
        return Result(root: node, focusedAreaID: firstAreaID(in: node))
    }

    private static func buildNode(from pane: LayoutConfig.Pane, projectPath: String) -> SplitNode? {
        switch pane {
        case let .leaf(tabs):
            return makeArea(tabs: tabs, projectPath: projectPath).map { .tabArea($0) }
        case let .branch(layout, panes):
            let children = panes.compactMap { buildNode(from: $0, projectPath: projectPath) }
            guard let first = children.first else { return nil }
            if children.count == 1 { return first }
            let direction: SplitDirection = layout == .horizontal ? .horizontal : .vertical
            return children.dropFirst().reduce(first) { partial, next in
                .split(SplitBranch(direction: direction, first: partial, second: next))
            }
        }
    }

    private static func makeArea(tabs: [LayoutConfig.Tab], projectPath: String) -> TabArea? {
        let terminalTabs = tabs.map { makeTab(from: $0, projectPath: projectPath) }
        guard let firstTab = terminalTabs.first else { return nil }
        let area = TabArea(projectPath: projectPath, existingTab: firstTab)
        for tab in terminalTabs.dropFirst() {
            area.insertExistingTab(tab)
        }
        area.activeTabID = firstTab.id
        return area
    }

    private static func makeTab(from tab: LayoutConfig.Tab, projectPath: String) -> TerminalTab {
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
        let pane = TerminalPaneState(
            projectPath: projectPath,
            title: resolvedTitle,
            startupCommand: resolvedCommand,
            startupCommandInteractive: true
        )
        return TerminalTab(pane: pane)
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
