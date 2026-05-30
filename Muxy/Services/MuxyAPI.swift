import Foundation

enum APIError: Error, Equatable {
    case invalidArguments(String)
    case noActiveProject
    case noFocusedArea
    case projectStoreUnavailable
    case worktreeStoreUnavailable
    case invalidPaneID
    case paneNotFound(String)
    case projectNotFound(String)
    case worktreeNotFound(String)
    case tabNotFound(String)
    case unsupportedKey(String)
    case worktreePathExists
    case splitFailed
    case renameFailed
    case consentDenied(verb: String)
    case underlying(String)

    var message: String {
        switch self {
        case let .invalidArguments(detail): detail
        case .noActiveProject: "no active project"
        case .noFocusedArea: "no focused area"
        case .projectStoreUnavailable: "project store unavailable"
        case .worktreeStoreUnavailable: "worktree store unavailable"
        case .invalidPaneID: "invalid pane ID"
        case let .paneNotFound(id): "pane not found \(id)"
        case let .projectNotFound(id): "project not found\(id.isEmpty ? "" : " \(id)")"
        case let .worktreeNotFound(id): "worktree not found \(id)"
        case let .tabNotFound(id): "tab not found \(id)"
        case let .unsupportedKey(key): "unsupported key \(key)"
        case .worktreePathExists: "worktree path already exists"
        case .splitFailed: "split succeeded but could not determine new pane ID"
        case .renameFailed: "could not rename pane"
        case let .consentDenied(verb): "user denied consent for \(verb)"
        case let .underlying(message): message
        }
    }
}

struct PaneInfo: Equatable {
    let id: UUID
    let title: String
    let workingDirectory: String
    let isFocused: Bool
}

struct ProjectInfo: Equatable {
    let id: UUID
    let name: String
    let path: String
    let isActive: Bool
}

struct WorktreeInfo: Equatable {
    let id: UUID
    let name: String
    let path: String
    let branch: String?
    let isActive: Bool
}

struct TabInfo: Equatable {
    let index: Int
    let id: UUID
    let kind: TerminalTab.Kind
    let title: String
    let isActive: Bool
}

struct CreatedWorktreeInfo: Equatable {
    let id: UUID
    let name: String
    let path: String
    let branch: String?
}

struct RefreshWorktreesResult: Equatable {
    let count: Int
}

struct CreateWorktreeRequest {
    let name: String
    let branch: String
    let projectIdentifier: String?
    let requestedPath: String
    let createBranch: Bool
    let baseBranch: String
}

struct OpenTabRequest: Decodable {
    let kind: TerminalTab.Kind
    let filePath: String?
    let extensionPayload: ExtensionPayload?

    struct ExtensionPayload: Decodable {
        let id: String
        let tabType: String
        let data: ExtensionJSON?
    }

    private enum CodingKeys: String, CodingKey {
        case kind
        case filePath
        case `extension`
    }

    init(kind: TerminalTab.Kind, filePath: String? = nil, extensionPayload: ExtensionPayload? = nil) {
        self.kind = kind
        self.filePath = filePath
        self.extensionPayload = extensionPayload
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        kind = try container.decode(TerminalTab.Kind.self, forKey: .kind)
        filePath = try container.decodeIfPresent(String.self, forKey: .filePath)
        extensionPayload = try container.decodeIfPresent(ExtensionPayload.self, forKey: .extension)
    }
}

enum MuxyAPI {
    enum Permissions {
        static func required(for verb: String) -> ExtensionPermission? {
            verbPermissions[canonical(verb)]
        }

        static func canonical(_ verb: String) -> String {
            cliAliases[verb] ?? verb
        }

        static let verbNames: Set<String> = Set(cliAliases.keys).union(extensionVerbs)

        private static let extensionVerbs: Set<String> = [
            "extension.settings.get",
            "extension.settings.set",
            "extension.statusbar.set",
            "panel.open",
            "panel.close",
            "panel.toggle",
        ]

        private static let cliAliases: [String: String] = [
            "split-right": "panes.split",
            "split-down": "panes.split",
            "send": "panes.send",
            "send-keys": "panes.sendKeys",
            "read-screen": "panes.readScreen",
            "close-pane": "panes.close",
            "rename-pane": "panes.rename",
            "list-panes": "panes.list",
            "list-projects": "projects.list",
            "switch-project": "projects.switch",
            "list-worktrees": "worktrees.list",
            "create-worktree": "worktrees.create",
            "switch-worktree": "worktrees.switch",
            "refresh-worktrees": "worktrees.refresh",
            "list-tabs": "tabs.list",
            "switch-tab": "tabs.switch",
            "new-tab": "tabs.new",
            "next-tab": "tabs.next",
            "previous-tab": "tabs.previous",
            "open-tab": "tabs.open",
        ]

        private static let verbPermissions: [String: ExtensionPermission] = [
            "panes.split": .panesWrite,
            "panes.list": .panesRead,
            "panes.send": .panesWrite,
            "panes.sendKeys": .panesWrite,
            "panes.readScreen": .panesRead,
            "panes.close": .panesWrite,
            "panes.rename": .panesWrite,
            "tabs.list": .tabsRead,
            "tabs.switch": .tabsWrite,
            "tabs.new": .tabsWrite,
            "tabs.next": .tabsWrite,
            "tabs.previous": .tabsWrite,
            "tabs.open": .tabsWrite,
            "projects.list": .projectsRead,
            "projects.switch": .projectsWrite,
            "worktrees.list": .worktreesRead,
            "worktrees.create": .worktreesWrite,
            "worktrees.switch": .worktreesWrite,
            "worktrees.refresh": .worktreesWrite,
            "toast": .notificationsWrite,
            "panel.open": .panelsWrite,
            "panel.close": .panelsWrite,
            "panel.toggle": .panelsWrite,
            "exec": .commandsExec,
        ]
    }

    @MainActor
    enum Panels {
        static func open(
            extensionID: String,
            panelID: String,
            data: ExtensionJSON?,
            toggle: Bool
        ) -> Result<Void, APIError> {
            guard let muxyExtension = ExtensionStore.shared.loadedExtension(id: extensionID),
                  let panel = muxyExtension.manifest.panel(id: panelID)
            else {
                return .failure(.invalidArguments("unknown panel '\(panelID)'"))
            }
            if toggle {
                ExtensionPanelRegistry.shared.toggle(extensionID: extensionID, panel: panel, data: data)
            } else {
                ExtensionPanelRegistry.shared.open(extensionID: extensionID, panel: panel, data: data)
            }
            return .success(())
        }

        static func close(extensionID: String, panelID: String) -> Result<Void, APIError> {
            let hostPanelID = ExtensionPanelState.hostPanelID(extensionID: extensionID, panelID: panelID)
            ExtensionPanelRegistry.shared.close(hostPanelID: hostPanelID)
            return .success(())
        }
    }

    @MainActor
    enum Panes {
        static func split(
            direction: SplitDirection,
            command: String?,
            fromPane: String?,
            appState: AppState
        ) -> Result<UUID, APIError> {
            let projectID: UUID
            let areaID: UUID

            if let fromPane, let paneID = UUID(uuidString: fromPane),
               let loc = locateTab(paneID: paneID, appState: appState)
            {
                projectID = loc.key.projectID
                areaID = loc.areaID
            } else {
                guard let activeID = appState.activeProjectID else {
                    return .failure(.noActiveProject)
                }
                guard let area = appState.focusedArea(for: activeID) else {
                    return .failure(.noFocusedArea)
                }
                projectID = activeID
                areaID = area.id
            }

            let trimmed = command?.trimmingCharacters(in: .whitespacesAndNewlines)
            let finalCommand = (trimmed?.isEmpty ?? true) ? nil : trimmed

            let existing = collectAllPaneIDs(appState: appState)

            appState.dispatch(.splitArea(.init(
                projectID: projectID,
                areaID: areaID,
                direction: direction,
                position: .second,
                command: finalCommand
            )))

            let added = collectAllPaneIDs(appState: appState).subtracting(existing)
            guard let newPaneID = added.first else {
                return .failure(.splitFailed)
            }
            return .success(newPaneID)
        }

        static func send(
            paneIDString: String,
            text: String,
            appState: AppState,
            extensionID: String? = nil
        ) async -> Result<Void, APIError> {
            guard let paneID = UUID(uuidString: paneIDString) else {
                return .failure(.invalidPaneID)
            }
            if let extensionID,
               await !consentGranted(extensionID: extensionID, verb: .panesSend, paneIDString: paneIDString)
            {
                return .failure(.consentDenied(verb: ExtensionGatedVerb.panesSend.rawValue))
            }
            guard let view = await waitForView(paneID: paneID, appState: appState) else {
                return .failure(.paneNotFound(paneIDString))
            }
            view.sendText(text)
            return .success(())
        }

        static func sendKeys(
            paneIDString: String,
            key: String,
            appState: AppState,
            extensionID: String? = nil
        ) async -> Result<Void, APIError> {
            guard let paneID = UUID(uuidString: paneIDString) else {
                return .failure(.invalidPaneID)
            }
            if let extensionID,
               await !consentGranted(extensionID: extensionID, verb: .panesSendKeys, paneIDString: paneIDString)
            {
                return .failure(.consentDenied(verb: ExtensionGatedVerb.panesSendKeys.rawValue))
            }
            guard let view = await waitForView(paneID: paneID, appState: appState) else {
                return .failure(.paneNotFound(paneIDString))
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
                return .failure(.unsupportedKey(key))
            }

            view.sendRemoteBytes(bytes)
            return .success(())
        }

        static func readScreen(
            paneIDString: String,
            lines: Int,
            appState: AppState,
            extensionID: String? = nil
        ) async -> Result<String, APIError> {
            guard let paneID = UUID(uuidString: paneIDString) else {
                return .failure(.invalidPaneID)
            }
            if let extensionID,
               await !consentGranted(extensionID: extensionID, verb: .panesReadScreen, paneIDString: paneIDString)
            {
                return .failure(.consentDenied(verb: ExtensionGatedVerb.panesReadScreen.rawValue))
            }
            let clamped = min(max(lines, 1), 500)
            guard let view = await waitForView(paneID: paneID, appState: appState) else {
                return .failure(.paneNotFound(paneIDString))
            }
            return .success(view.readScreenText(lastLines: clamped))
        }

        static func close(
            paneIDString: String,
            appState: AppState
        ) -> Result<Void, APIError> {
            guard let paneID = UUID(uuidString: paneIDString) else {
                return .failure(.invalidPaneID)
            }
            guard let loc = locateTab(paneID: paneID, appState: appState) else {
                return .failure(.paneNotFound(paneIDString))
            }
            appState.dispatch(.closeTab(projectID: loc.key.projectID, areaID: loc.areaID, tabID: loc.tabID))
            return .success(())
        }

        static func rename(
            paneIDString: String,
            title: String,
            appState: AppState
        ) -> Result<Void, APIError> {
            guard let paneID = UUID(uuidString: paneIDString) else {
                return .failure(.invalidPaneID)
            }
            guard let loc = locateTab(paneID: paneID, appState: appState) else {
                return .failure(.paneNotFound(paneIDString))
            }
            for (_, root) in appState.workspaceRoots {
                guard let area = root.findArea(id: loc.areaID) else { continue }
                area.setCustomTitle(loc.tabID, title: title)
                return .success(())
            }
            return .failure(.renameFailed)
        }

        static func list(appState: AppState) -> [PaneInfo] {
            var result: [PaneInfo] = []
            for (key, root) in appState.workspaceRoots {
                let focusedAreaID = appState.focusedAreaID(for: key.projectID)
                for area in root.allAreas() {
                    for tab in area.tabs {
                        guard let pane = tab.content.pane else { continue }
                        let isFocused = area.id == focusedAreaID && tab.id == area.activeTabID
                        let title = tab.customTitle ?? pane.title
                        let cwd = pane.currentWorkingDirectory ?? pane.projectPath
                        result.append(PaneInfo(
                            id: pane.id,
                            title: title,
                            workingDirectory: cwd,
                            isFocused: isFocused
                        ))
                    }
                }
            }
            return result
        }
    }

    @MainActor
    enum Projects {
        static func list(appState: AppState, projectStore: ProjectStore) -> [ProjectInfo] {
            projectStore.projects.map { project in
                ProjectInfo(
                    id: project.id,
                    name: project.name,
                    path: project.path,
                    isActive: project.id == appState.activeProjectID
                )
            }
        }

        static func switchTo(
            identifier: String,
            appState: AppState,
            projectStore: ProjectStore,
            worktreeStore: WorktreeStore
        ) -> Result<Void, APIError> {
            guard let project = findProject(identifier, in: projectStore.projects) else {
                return .failure(.projectNotFound(identifier))
            }
            guard let worktree = worktreeStore.preferred(
                for: project.id,
                matching: appState.activeWorktreeID[project.id]
            )
            else {
                return .failure(.underlying("no worktree for project \(project.name)"))
            }
            appState.selectProject(project, worktree: worktree)
            return .success(())
        }
    }

    @MainActor
    enum Worktrees {
        static func list(
            projectIdentifier: String?,
            appState: AppState,
            projectStore: ProjectStore,
            worktreeStore: WorktreeStore
        ) -> Result<[WorktreeInfo], APIError> {
            guard let project = resolveProject(projectIdentifier, appState: appState, projectStore: projectStore) else {
                return .failure(.projectNotFound(projectIdentifier ?? ""))
            }
            let infos = worktreeStore.list(for: project.id).map { worktree in
                let isActive = appState.activeProjectID == project.id
                    && appState.activeWorktreeID[project.id] == worktree.id
                return WorktreeInfo(
                    id: worktree.id,
                    name: worktree.name,
                    path: worktree.path,
                    branch: worktree.branch,
                    isActive: isActive
                )
            }
            return .success(infos)
        }

        static func switchTo(
            identifier: String,
            projectIdentifier: String?,
            appState: AppState,
            projectStore: ProjectStore,
            worktreeStore: WorktreeStore
        ) -> Result<Void, APIError> {
            guard let project = resolveProject(projectIdentifier, appState: appState, projectStore: projectStore) else {
                return .failure(.projectNotFound(projectIdentifier ?? ""))
            }
            guard let worktree = findWorktree(identifier, in: worktreeStore.list(for: project.id)) else {
                return .failure(.worktreeNotFound(identifier))
            }
            appState.selectWorktree(projectID: project.id, worktree: worktree)
            return .success(())
        }

        static func create(
            _ request: CreateWorktreeRequest,
            appState: AppState,
            projectStore: ProjectStore,
            worktreeStore: WorktreeStore
        ) async -> Result<CreatedWorktreeInfo, APIError> {
            let trimmedName = request.name.trimmingCharacters(in: .whitespacesAndNewlines)
            let trimmedBranch = request.branch.trimmingCharacters(in: .whitespacesAndNewlines)
            let trimmedPath = request.requestedPath.trimmingCharacters(in: .whitespacesAndNewlines)
            let trimmedBase = request.baseBranch.trimmingCharacters(in: .whitespacesAndNewlines)

            guard !trimmedName.isEmpty, !trimmedBranch.isEmpty else {
                return .failure(.invalidArguments("name and branch are required"))
            }
            guard let project = resolveProject(
                request.projectIdentifier,
                appState: appState,
                projectStore: projectStore
            )
            else {
                return .failure(.projectNotFound(request.projectIdentifier ?? ""))
            }

            let path = trimmedPath.isEmpty
                ? WorktreeLocationResolver.worktreeDirectory(for: project, slug: slug(from: trimmedName))
                : NSString(string: trimmedPath).expandingTildeInPath
            guard !FileManager.default.fileExists(atPath: path) else {
                return .failure(.worktreePathExists)
            }

            do {
                let worktree = try await worktreeStore.createWorktree(
                    project: project,
                    request: WorktreeCreationRequest(
                        name: trimmedName,
                        path: path,
                        branch: trimmedBranch,
                        createBranch: request.createBranch,
                        baseBranch: request.createBranch && !trimmedBase.isEmpty ? trimmedBase : nil
                    )
                )
                appState.selectWorktree(projectID: project.id, worktree: worktree)
                return .success(CreatedWorktreeInfo(
                    id: worktree.id,
                    name: worktree.name,
                    path: worktree.path,
                    branch: worktree.branch
                ))
            } catch {
                return .failure(.underlying(error.localizedDescription))
            }
        }

        static func refresh(
            projectIdentifier: String?,
            appState: AppState,
            projectStore: ProjectStore,
            worktreeStore: WorktreeStore
        ) async -> Result<RefreshWorktreesResult, APIError> {
            guard let project = resolveProject(projectIdentifier, appState: appState, projectStore: projectStore) else {
                return .failure(.projectNotFound(projectIdentifier ?? ""))
            }
            do {
                let worktrees = try await worktreeStore.refreshFromGit(project: project)
                return .success(RefreshWorktreesResult(count: worktrees.count))
            } catch {
                return .failure(.underlying(error.localizedDescription))
            }
        }
    }

    @MainActor
    enum Tabs {
        static func list(appState: AppState) -> Result<[TabInfo], APIError> {
            guard let projectID = appState.activeProjectID,
                  let key = appState.activeWorktreeKey(for: projectID),
                  let root = appState.workspaceRoots[key]
            else { return .failure(.noActiveProject) }

            let focusedAreaID = appState.focusedAreaID[key]
            var index = 0
            var infos: [TabInfo] = []
            for area in root.allAreas() {
                for tab in area.tabs {
                    let isActive = area.id == focusedAreaID && tab.id == area.activeTabID
                    infos.append(TabInfo(
                        index: index,
                        id: tab.id,
                        kind: tab.kind,
                        title: tab.title,
                        isActive: isActive
                    ))
                    index += 1
                }
            }
            return .success(infos)
        }

        static func switchTo(identifier: String, appState: AppState) -> Result<Void, APIError> {
            guard let projectID = appState.activeProjectID,
                  let key = appState.activeWorktreeKey(for: projectID),
                  let root = appState.workspaceRoots[key]
            else { return .failure(.noActiveProject) }

            if let index = Int(identifier) {
                guard tab(at: index, in: root) != nil else { return .failure(.tabNotFound(identifier)) }
                appState.selectTabByIndex(index, projectID: projectID)
                return .success(())
            }
            for area in root.allAreas() {
                guard let tab = area.tabs.first(where: { tabMatches($0, identifier: identifier) }) else { continue }
                appState.dispatch(.selectTab(projectID: projectID, areaID: area.id, tabID: tab.id))
                return .success(())
            }
            return .failure(.tabNotFound(identifier))
        }

        static func new(appState: AppState) -> Result<UUID?, APIError> {
            guard let projectID = appState.activeProjectID else { return .failure(.noActiveProject) }
            let before = collectTabs(appState: appState)
            appState.dispatch(.createTab(projectID: projectID, areaID: nil))
            let added = collectTabs(appState: appState).subtracting(before)
            return .success(added.first)
        }

        static func next(appState: AppState) -> Result<Void, APIError> {
            guard let projectID = appState.activeProjectID else { return .failure(.noActiveProject) }
            appState.selectNextTab(projectID: projectID)
            return .success(())
        }

        static func previous(appState: AppState) -> Result<Void, APIError> {
            guard let projectID = appState.activeProjectID else { return .failure(.noActiveProject) }
            appState.selectPreviousTab(projectID: projectID)
            return .success(())
        }

        static func open(
            _ request: OpenTabRequest,
            appState: AppState,
            callingExtensionID: String? = nil
        ) async -> Result<Void, APIError> {
            guard let projectID = appState.activeProjectID else {
                return .failure(.noActiveProject)
            }
            switch request.kind {
            case .terminal:
                appState.dispatch(.createTab(projectID: projectID, areaID: nil))
                return .success(())
            case .vcs:
                appState.dispatch(.createVCSTab(projectID: projectID, areaID: nil))
                return .success(())
            case .editor:
                guard let filePath = request.filePath, !filePath.isEmpty else {
                    return .failure(.invalidArguments("editor tabs require filePath"))
                }
                appState.dispatch(.createEditorTab(
                    projectID: projectID,
                    areaID: nil,
                    filePath: filePath,
                    suppressInitialFocus: false
                ))
                return .success(())
            case .imageViewer:
                guard let filePath = request.filePath, !filePath.isEmpty else {
                    return .failure(.invalidArguments("imageViewer tabs require filePath"))
                }
                appState.dispatch(.createImageViewerTab(
                    projectID: projectID,
                    areaID: nil,
                    filePath: filePath
                ))
                return .success(())
            case .extensionWebView:
                guard let payload = request.extensionPayload else {
                    return .failure(.invalidArguments("extensionWebView tabs require extension payload"))
                }
                guard let muxyExtension = ExtensionStore.shared.loadedExtension(id: payload.id) else {
                    return .failure(.invalidArguments("extension '\(payload.id)' is not loaded"))
                }
                guard let tabType = muxyExtension.manifest.tabType(id: payload.tabType) else {
                    return .failure(.invalidArguments(
                        "extension '\(payload.id)' has no tab type '\(payload.tabType)'"
                    ))
                }
                if let callingExtensionID, callingExtensionID != payload.id {
                    let allowed = await foreignTabConsentGranted(
                        callingExtensionID: callingExtensionID,
                        targetExtensionID: payload.id,
                        tabTypeID: payload.tabType
                    )
                    if !allowed {
                        return .failure(.consentDenied(verb: ExtensionGatedVerb.tabsOpenForeign.rawValue))
                    }
                }
                appState.dispatch(.createExtensionTab(
                    projectID: projectID,
                    areaID: nil,
                    request: AppState.CreateExtensionTabRequest(
                        extensionID: payload.id,
                        tabTypeID: payload.tabType,
                        title: tabType.title,
                        data: payload.data ?? tabType.defaultData
                    )
                ))
                return .success(())
            case .diffViewer:
                return .failure(.invalidArguments("diffViewer cannot be opened via open-tab yet"))
            }
        }
    }
}

private struct PaneLocation {
    let key: WorktreeKey
    let areaID: UUID
    let tabID: UUID
}

@MainActor
private func consentGranted(
    extensionID: String,
    verb: ExtensionGatedVerb,
    paneIDString: String
) async -> Bool {
    let request = ExtensionConsentRequestBuilder.make(
        extensionID: extensionID,
        verb: verb,
        payload: .pane(id: paneIDString),
        source: "muxy-api"
    )
    let decision = await ExtensionConsentService.shared.gate(request)
    return decision == .allow
}

@MainActor
private func foreignTabConsentGranted(
    callingExtensionID: String,
    targetExtensionID: String,
    tabTypeID: String
) async -> Bool {
    let request = ExtensionConsentRequestBuilder.make(
        extensionID: callingExtensionID,
        verb: .tabsOpenForeign,
        payload: .foreignTab(targetExtensionID: targetExtensionID, tabTypeID: tabTypeID),
        source: "muxy-api"
    )
    let decision = await ExtensionConsentService.shared.gate(request)
    return decision == .allow
}

@MainActor
private func locateTab(paneID: UUID, appState: AppState) -> PaneLocation? {
    for (key, root) in appState.workspaceRoots {
        for area in root.allAreas() {
            for tab in area.tabs where tab.content.pane?.id == paneID {
                return PaneLocation(key: key, areaID: area.id, tabID: tab.id)
            }
        }
    }
    return nil
}

@MainActor
private func collectAllPaneIDs(appState: AppState) -> Set<UUID> {
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

@MainActor
private func collectTabs(appState: AppState) -> Set<UUID> {
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

@MainActor
private func waitForView(
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

@MainActor
private func findProject(_ identifier: String, in projects: [Project]) -> Project? {
    let standardizedPath = URL(fileURLWithPath: identifier).standardizedFileURL.path
    return projects.first { project in
        project.id.uuidString == identifier
            || project.name.localizedCaseInsensitiveCompare(identifier) == .orderedSame
            || URL(fileURLWithPath: project.path).standardizedFileURL.path == standardizedPath
    }
}

@MainActor
private func resolveProject(
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

@MainActor
private func findWorktree(_ identifier: String, in worktrees: [Worktree]) -> Worktree? {
    let standardizedPath = URL(fileURLWithPath: identifier).standardizedFileURL.path
    return worktrees.first { worktree in
        worktree.id.uuidString == identifier
            || worktree.name.localizedCaseInsensitiveCompare(identifier) == .orderedSame
            || worktree.branch?.localizedCaseInsensitiveCompare(identifier) == .orderedSame
            || URL(fileURLWithPath: worktree.path).standardizedFileURL.path == standardizedPath
    }
}

private func slug(from name: String) -> String {
    let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "._-"))
    let scalars = name.unicodeScalars.map { allowed.contains($0) ? Character($0) : "-" }
    let collapsed = String(scalars)
        .split(separator: "-", omittingEmptySubsequences: true)
        .joined(separator: "-")
    return collapsed.isEmpty ? UUID().uuidString : collapsed
}

@MainActor
private func tabMatches(_ tab: TerminalTab, identifier: String) -> Bool {
    tab.id.uuidString == identifier
        || tab.content.pane?.id.uuidString == identifier
        || tab.title.localizedCaseInsensitiveCompare(identifier) == .orderedSame
}

@MainActor
private func tab(at index: Int, in root: SplitNode) -> TerminalTab? {
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
