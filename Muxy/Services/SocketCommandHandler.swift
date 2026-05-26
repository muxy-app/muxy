import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "SocketCommandHandler")

@MainActor
enum SocketCommandHandler {
    private static let verbPermissions: [String: ExtensionPermission] = [
        "split-right": .panesWrite,
        "split-down": .panesWrite,
        "send": .panesWrite,
        "send-keys": .panesWrite,
        "read-screen": .panesRead,
        "close-pane": .panesWrite,
        "rename-pane": .panesWrite,
        "list-panes": .panesRead,
        "list-projects": .projectsRead,
        "switch-project": .projectsWrite,
        "list-worktrees": .worktreesRead,
        "create-worktree": .worktreesWrite,
        "switch-worktree": .worktreesWrite,
        "refresh-worktrees": .worktreesWrite,
        "list-tabs": .tabsRead,
        "switch-tab": .tabsWrite,
        "new-tab": .tabsWrite,
        "next-tab": .tabsWrite,
        "previous-tab": .tabsWrite,
    ]

    static func handleRequest(
        _ message: String,
        appState: AppState,
        projectStore: ProjectStore? = nil,
        worktreeStore: WorktreeStore? = nil,
        clientContext: NotificationSocketServer.ClientContext = .init(extensionID: nil)
    ) async -> String {
        let parts = message.components(separatedBy: "|")
        guard let cmd = parts.first else {
            return "error:empty command"
        }

        if let extensionID = clientContext.extensionID,
           let required = verbPermissions[cmd],
           !ExtensionStore.shared.extensionHasPermission(id: extensionID, permission: required)
        {
            return "error:permission denied (\(required.rawValue))"
        }

        switch cmd {
        case "split-right":
            let request = parseSplitRequest(parts: parts)
            return handleSplit(direction: .horizontal, command: request.command, fromPane: request.fromPane, appState: appState)
        case "split-down":
            let request = parseSplitRequest(parts: parts)
            return handleSplit(direction: .vertical, command: request.command, fromPane: request.fromPane, appState: appState)
        case "send":
            guard parts.count >= 3 else { return "error:usage send|paneID|text" }
            return await handleSend(paneIDStr: parts[1], text: parts.dropFirst(2).joined(separator: "|"), appState: appState)
        case "send-keys":
            guard parts.count >= 3 else { return "error:usage send-keys|paneID|key" }
            return await handleSendKeys(paneIDStr: parts[1], key: parts[2], appState: appState)
        case "read-screen":
            guard parts.count >= 2 else { return "error:usage read-screen|paneID[|lines]" }
            let lines = parts.count >= 3 ? Int(parts[2]) ?? 50 : 50
            return await handleReadScreen(paneIDStr: parts[1], lines: lines, appState: appState)
        case "close-pane":
            guard parts.count >= 2 else { return "error:usage close-pane|paneID" }
            return handleClosePane(paneIDStr: parts[1], appState: appState)
        case "rename-pane":
            guard parts.count >= 3 else { return "error:usage rename-pane|paneID|title" }
            return handleRenamePane(paneIDStr: parts[1], title: parts.dropFirst(2).joined(separator: "|"), appState: appState)
        case "list-panes":
            return handleListPanes(appState: appState)
        case "list-projects":
            guard let projectStore else { return "error:project store unavailable" }
            return handleListProjects(appState: appState, projectStore: projectStore)
        case "switch-project":
            guard parts.count >= 2 else { return "error:usage switch-project|name-or-id-or-path" }
            guard let projectStore, let worktreeStore else { return "error:project store unavailable" }
            return handleSwitchProject(
                identifier: parts.dropFirst().joined(separator: "|"),
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            )
        case "list-worktrees":
            guard let projectStore, let worktreeStore else { return "error:worktree store unavailable" }
            let identifier = parts.count >= 2 ? parts.dropFirst().joined(separator: "|") : nil
            return handleListWorktrees(
                projectIdentifier: identifier,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            )
        case "create-worktree":
            guard let projectStore, let worktreeStore else { return "error:worktree store unavailable" }
            return await handleCreateWorktree(
                arguments: Array(parts.dropFirst()),
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            )
        case "switch-worktree":
            guard parts.count >= 2 else { return "error:usage switch-worktree|name-or-id-or-path[|project]" }
            guard let projectStore, let worktreeStore else { return "error:worktree store unavailable" }
            let projectIdentifier = parts.count >= 3 ? parts.dropFirst(2).joined(separator: "|") : nil
            return handleSwitchWorktree(
                identifier: parts[1],
                projectIdentifier: projectIdentifier,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            )
        case "refresh-worktrees":
            guard let projectStore, let worktreeStore else { return "error:worktree store unavailable" }
            let identifier = parts.count >= 2 ? parts.dropFirst().joined(separator: "|") : nil
            return await handleRefreshWorktrees(
                projectIdentifier: identifier,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            )
        case "list-tabs":
            return handleListTabs(appState: appState)
        case "switch-tab":
            guard parts.count >= 2 else { return "error:usage switch-tab|index-or-id-or-title" }
            return handleSwitchTab(identifier: parts.dropFirst().joined(separator: "|"), appState: appState)
        case "new-tab":
            return handleNewTab(appState: appState)
        case "next-tab":
            return handleTabStep(next: true, appState: appState)
        case "previous-tab":
            return handleTabStep(next: false, appState: appState)
        default:
            return "error:unknown command \(cmd)"
        }
    }

    private static func parseSplitRequest(parts: [String]) -> (fromPane: String?, command: String?) {
        guard parts.count >= 2 else { return (nil, nil) }
        let firstValue = parts[1]
        let firstValueIsPane = firstValue.isEmpty || UUID(uuidString: firstValue) != nil
        if firstValueIsPane {
            let command = parts.count >= 3 ? parts.dropFirst(2).joined(separator: "|") : nil
            return (firstValue, command)
        }
        if parts.count >= 3, let fromPane = parts.last, UUID(uuidString: fromPane) != nil {
            return (fromPane, parts.dropFirst(1).dropLast().joined(separator: "|"))
        }
        return (nil, parts.dropFirst(1).joined(separator: "|"))
    }

    private static func handleSplit(direction: SplitDirection, command: String?, fromPane: String?, appState: AppState) -> String {
        let projectID: UUID
        let areaID: UUID

        if let fromPane, let paneID = UUID(uuidString: fromPane),
           let loc = locateTab(paneID: paneID, appState: appState)
        {
            projectID = loc.key.projectID
            areaID = loc.areaID
        } else {
            guard let activeID = appState.activeProjectID else {
                return "error:no active project"
            }
            guard let area = appState.focusedArea(for: activeID) else {
                return "error:no focused area"
            }
            projectID = activeID
            areaID = area.id
        }

        let trimmedCommand = command?.trimmingCharacters(in: .whitespacesAndNewlines)
        let finalCommand = (trimmedCommand?.isEmpty ?? true) ? nil : trimmedCommand

        let existingPaneIDs = collectAllPaneIDs(appState: appState)

        appState.dispatch(.splitArea(.init(
            projectID: projectID,
            areaID: areaID,
            direction: direction,
            position: .second,
            command: finalCommand
        )))

        let newPaneIDs = collectAllPaneIDs(appState: appState)
        let added = newPaneIDs.subtracting(existingPaneIDs)

        guard let newPaneID = added.first else {
            return "error:split succeeded but could not determine new pane ID"
        }

        return newPaneID.uuidString
    }

    private static func handleSend(paneIDStr: String, text: String, appState: AppState) async -> String {
        guard let paneID = UUID(uuidString: paneIDStr) else {
            return "error:invalid pane ID"
        }
        guard let view = await waitForView(paneID: paneID, appState: appState) else {
            return "error:pane not found \(paneIDStr)"
        }

        view.sendText(text)
        return "ok"
    }

    private static func handleSendKeys(paneIDStr: String, key: String, appState: AppState) async -> String {
        guard let paneID = UUID(uuidString: paneIDStr) else {
            return "error:invalid pane ID"
        }
        guard let view = await waitForView(paneID: paneID, appState: appState) else {
            return "error:pane not found \(paneIDStr)"
        }

        let bytes: Data
        switch key.lowercased() {
        case "escape",
             "esc":
            bytes = Data([0x1B])
        case "enter",
             "return":
            bytes = Data([0x0D])
        case "tab":
            bytes = Data([0x09])
        case "ctrl+c",
             "ctrl-c":
            bytes = Data([0x03])
        case "ctrl+d",
             "ctrl-d":
            bytes = Data([0x04])
        case "ctrl+z",
             "ctrl-z":
            bytes = Data([0x1A])
        case "backspace":
            bytes = Data([0x7F])
        default:
            return "error:unsupported key \(key)"
        }

        view.sendRemoteBytes(bytes)
        return "ok"
    }

    private static func handleReadScreen(paneIDStr: String, lines: Int, appState: AppState) async -> String {
        guard let paneID = UUID(uuidString: paneIDStr) else {
            return "error:invalid pane ID"
        }
        let clampedLines = min(max(lines, 1), 500)

        guard let view = await waitForView(paneID: paneID, appState: appState) else {
            return "error:pane not found \(paneIDStr)"
        }

        return view.readScreenText(lastLines: clampedLines)
    }

    private static func handleClosePane(paneIDStr: String, appState: AppState) -> String {
        guard let paneID = UUID(uuidString: paneIDStr) else {
            return "error:invalid pane ID"
        }

        guard let loc = locateTab(paneID: paneID, appState: appState) else {
            return "error:pane not found \(paneIDStr)"
        }

        appState.dispatch(.closeTab(projectID: loc.key.projectID, areaID: loc.areaID, tabID: loc.tabID))
        return "ok"
    }

    private static func handleRenamePane(paneIDStr: String, title: String, appState: AppState) -> String {
        guard let paneID = UUID(uuidString: paneIDStr) else {
            return "error:invalid pane ID"
        }

        guard let loc = locateTab(paneID: paneID, appState: appState) else {
            return "error:pane not found \(paneIDStr)"
        }

        for (_, root) in appState.workspaceRoots {
            guard let area = root.findArea(id: loc.areaID) else { continue }
            area.setCustomTitle(loc.tabID, title: title)
            return "ok"
        }

        return "error:could not rename pane"
    }

    private static func handleListPanes(appState: AppState) -> String {
        var lines: [String] = []
        for (key, root) in appState.workspaceRoots {
            let focusedAreaID = appState.focusedAreaID(for: key.projectID)
            for area in root.allAreas() {
                for tab in area.tabs {
                    guard let pane = tab.content.pane else { continue }
                    let isFocused = area.id == focusedAreaID && tab.id == area.activeTabID
                    let title = tab.customTitle ?? pane.title
                    let cwd = pane.currentWorkingDirectory ?? pane.projectPath
                    lines.append("\(pane.id.uuidString)\t\(title)\t\(cwd)\t\(isFocused)")
                }
            }
        }
        return lines.joined(separator: "\n")
    }

    private static func handleListProjects(appState: AppState, projectStore: ProjectStore) -> String {
        projectStore.projects.map { project in
            let active = project.id == appState.activeProjectID
            return "\(project.id.uuidString)\t\(project.name)\t\(project.path)\t\(active)"
        }.joined(separator: "\n")
    }

    private static func handleSwitchProject(
        identifier: String,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore
    ) -> String {
        guard let project = findProject(identifier, in: projectStore.projects) else {
            return "error:project not found \(identifier)"
        }
        guard let worktree = worktreeStore.preferred(for: project.id, matching: appState.activeWorktreeID[project.id]) else {
            return "error:no worktree for project \(project.name)"
        }
        appState.selectProject(project, worktree: worktree)
        return "ok"
    }

    private static func handleListWorktrees(
        projectIdentifier: String?,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore
    ) -> String {
        guard let project = resolveProject(projectIdentifier, appState: appState, projectStore: projectStore) else {
            return "error:project not found"
        }
        return worktreeStore.list(for: project.id).map { worktree in
            let active = appState.activeProjectID == project.id && appState.activeWorktreeID[project.id] == worktree.id
            return "\(worktree.id.uuidString)\t\(worktree.name)\t\(worktree.path)\t\(worktree.branch ?? "")\t\(active)"
        }.joined(separator: "\n")
    }

    private static func handleSwitchWorktree(
        identifier: String,
        projectIdentifier: String?,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore
    ) -> String {
        guard let project = resolveProject(projectIdentifier, appState: appState, projectStore: projectStore) else {
            return "error:project not found"
        }
        guard let worktree = findWorktree(identifier, in: worktreeStore.list(for: project.id)) else {
            return "error:worktree not found \(identifier)"
        }
        appState.selectWorktree(projectID: project.id, worktree: worktree)
        return "ok"
    }

    private static func handleCreateWorktree(
        arguments: [String],
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore
    ) async -> String {
        guard arguments.count >= 2 else {
            return "error:usage create-worktree|name|branch[|project][|path][|createBranch][|baseBranch]"
        }
        let name = arguments[0].trimmingCharacters(in: .whitespacesAndNewlines)
        let branch = arguments[1].trimmingCharacters(in: .whitespacesAndNewlines)
        let projectIdentifier = arguments.count >= 3 ? arguments[2] : nil
        let requestedPath = arguments.count >= 4 ? arguments[3].trimmingCharacters(in: .whitespacesAndNewlines) : ""
        let createBranch = arguments.count >= 5 ? arguments[4] != "false" : true
        let baseBranch = arguments.count >= 6 ? arguments[5].trimmingCharacters(in: .whitespacesAndNewlines) : ""

        guard !name.isEmpty, !branch.isEmpty else {
            return "error:name and branch are required"
        }
        guard let project = resolveProject(projectIdentifier, appState: appState, projectStore: projectStore) else {
            return "error:project not found"
        }

        let path = requestedPath.isEmpty
            ? WorktreeLocationResolver.worktreeDirectory(for: project, slug: slug(from: name))
            : NSString(string: requestedPath).expandingTildeInPath
        guard !FileManager.default.fileExists(atPath: path) else {
            return "error:worktree path already exists"
        }

        do {
            let worktree = try await worktreeStore.createWorktree(
                project: project,
                request: WorktreeCreationRequest(
                    name: name,
                    path: path,
                    branch: branch,
                    createBranch: createBranch,
                    baseBranch: createBranch && !baseBranch.isEmpty ? baseBranch : nil
                )
            )
            appState.selectWorktree(projectID: project.id, worktree: worktree)
            return "ok\t\(worktree.id.uuidString)\t\(worktree.name)\t\(worktree.path)\t\(worktree.branch ?? "")"
        } catch {
            return "error:\(error.localizedDescription)"
        }
    }

    private static func handleRefreshWorktrees(
        projectIdentifier: String?,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore
    ) async -> String {
        guard let project = resolveProject(projectIdentifier, appState: appState, projectStore: projectStore) else {
            return "error:project not found"
        }
        do {
            let worktrees = try await worktreeStore.refreshFromGit(project: project)
            return "ok\t\(worktrees.count)"
        } catch {
            return "error:\(error.localizedDescription)"
        }
    }

    private static func handleListTabs(appState: AppState) -> String {
        guard let projectID = appState.activeProjectID,
              let key = appState.activeWorktreeKey(for: projectID),
              let root = appState.workspaceRoots[key]
        else { return "error:no active project" }
        let focusedAreaID = appState.focusedAreaID[key]
        var index = 0
        var lines: [String] = []
        for area in root.allAreas() {
            for tab in area.tabs {
                let active = area.id == focusedAreaID && tab.id == area.activeTabID
                lines.append("\(index)\t\(tab.id.uuidString)\t\(tab.kind.rawValue)\t\(tab.title)\t\(active)")
                index += 1
            }
        }
        return lines.joined(separator: "\n")
    }

    private static func handleSwitchTab(identifier: String, appState: AppState) -> String {
        guard let projectID = appState.activeProjectID,
              let key = appState.activeWorktreeKey(for: projectID),
              let root = appState.workspaceRoots[key]
        else { return "error:no active project" }
        if let index = Int(identifier) {
            guard tab(at: index, in: root) != nil else { return "error:tab not found \(identifier)" }
            appState.selectTabByIndex(index, projectID: projectID)
            return "ok"
        }
        for area in root.allAreas() {
            guard let tab = area.tabs.first(where: { tabMatches($0, identifier: identifier) }) else { continue }
            appState.dispatch(.selectTab(projectID: projectID, areaID: area.id, tabID: tab.id))
            return "ok"
        }
        return "error:tab not found \(identifier)"
    }

    private static func handleNewTab(appState: AppState) -> String {
        guard let projectID = appState.activeProjectID else { return "error:no active project" }
        let before = collectTabs(appState: appState)
        appState.dispatch(.createTab(projectID: projectID, areaID: nil))
        let added = collectTabs(appState: appState).subtracting(before)
        return added.first?.uuidString ?? "ok"
    }

    private static func handleTabStep(next: Bool, appState: AppState) -> String {
        guard let projectID = appState.activeProjectID else { return "error:no active project" }
        if next {
            appState.selectNextTab(projectID: projectID)
        } else {
            appState.selectPreviousTab(projectID: projectID)
        }
        return "ok"
    }

    private static func findProject(_ identifier: String, in projects: [Project]) -> Project? {
        let standardizedPath = URL(fileURLWithPath: identifier).standardizedFileURL.path
        return projects.first { project in
            project.id.uuidString == identifier
                || project.name.localizedCaseInsensitiveCompare(identifier) == .orderedSame
                || URL(fileURLWithPath: project.path).standardizedFileURL.path == standardizedPath
        }
    }

    private static func resolveProject(
        _ identifier: String?,
        appState: AppState,
        projectStore: ProjectStore
    ) -> Project? {
        if let identifier, !identifier.isEmpty {
            return findProject(identifier, in: projectStore.projects)
        }
        guard let activeProjectID = appState.activeProjectID else { return nil }
        return projectStore.projects.first { $0.id == activeProjectID }
    }

    private static func findWorktree(_ identifier: String, in worktrees: [Worktree]) -> Worktree? {
        let standardizedPath = URL(fileURLWithPath: identifier).standardizedFileURL.path
        return worktrees.first { worktree in
            worktree.id.uuidString == identifier
                || worktree.name.localizedCaseInsensitiveCompare(identifier) == .orderedSame
                || worktree.branch?.localizedCaseInsensitiveCompare(identifier) == .orderedSame
                || URL(fileURLWithPath: worktree.path).standardizedFileURL.path == standardizedPath
        }
    }

    private static func slug(from name: String) -> String {
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "._-"))
        let scalars = name.unicodeScalars.map { allowed.contains($0) ? Character($0) : "-" }
        let collapsed = String(scalars)
            .split(separator: "-", omittingEmptySubsequences: true)
            .joined(separator: "-")
        return collapsed.isEmpty ? UUID().uuidString : collapsed
    }

    private static func tabMatches(_ tab: TerminalTab, identifier: String) -> Bool {
        tab.id.uuidString == identifier
            || tab.content.pane?.id.uuidString == identifier
            || tab.title.localizedCaseInsensitiveCompare(identifier) == .orderedSame
    }

    private static func tab(at index: Int, in root: SplitNode) -> TerminalTab? {
        guard index >= 0 else { return nil }
        var currentIndex = 0
        for area in root.allAreas() {
            for tab in area.tabs {
                if currentIndex == index { return tab }
                currentIndex += 1
            }
        }
        return nil
    }

    private static func collectTabs(appState: AppState) -> Set<UUID> {
        var ids = Set<UUID>()
        for root in appState.workspaceRoots.values {
            for area in root.allAreas() {
                for tab in area.tabs {
                    ids.insert(tab.id)
                }
            }
        }
        return ids
    }

    private static func waitForView(
        paneID: UUID,
        appState: AppState? = nil,
        timeout: Duration = .seconds(3)
    ) async -> GhosttyTerminalNSView? {
        if let view = TerminalViewRegistry.shared.existingView(for: paneID) {
            return view
        }
        if let appState, locateTab(paneID: paneID, appState: appState) == nil {
            return nil
        }
        let deadline = ContinuousClock.now + timeout
        while ContinuousClock.now < deadline {
            if let view = TerminalViewRegistry.shared.existingView(for: paneID) {
                return view
            }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return nil
    }

    private static func collectAllPaneIDs(appState: AppState) -> Set<UUID> {
        var ids = Set<UUID>()
        for (_, root) in appState.workspaceRoots {
            for area in root.allAreas() {
                for tab in area.tabs {
                    if let pane = tab.content.pane {
                        ids.insert(pane.id)
                    }
                }
            }
        }
        return ids
    }

    private struct PaneLocation {
        let key: WorktreeKey
        let areaID: UUID
        let tabID: UUID
    }

    private static func locateTab(paneID: UUID, appState: AppState) -> PaneLocation? {
        for (key, root) in appState.workspaceRoots {
            for area in root.allAreas() {
                for tab in area.tabs where tab.content.pane?.id == paneID {
                    return PaneLocation(key: key, areaID: area.id, tabID: tab.id)
                }
            }
        }
        return nil
    }
}
