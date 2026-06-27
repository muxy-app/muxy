import Foundation
import WebKit

enum APIError: Error, Equatable {
    case invalidArguments(String)
    case noActiveProject
    case noActiveWorkspace
    case noFocusedArea
    case projectStoreUnavailable
    case worktreeStoreUnavailable
    case invalidPaneID
    case paneNotFound(String)
    case paneSurfaceNotReady(paneID: String, waitedSeconds: Double)
    case projectNotFound(String)
    case worktreeNotFound(String)
    case tabNotFound(String)
    case tabCreateFailed
    case browserTabNotFound(String)
    case browserTabCreateFailed
    case browserTabSurfaceNotReady(tabID: String, waitedSeconds: Double)
    case browserDisabled
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
        case .noActiveWorkspace: "no active workspace"
        case .noFocusedArea: "no focused area"
        case .projectStoreUnavailable: "project store unavailable"
        case .worktreeStoreUnavailable: "worktree store unavailable"
        case .invalidPaneID: "invalid pane ID"
        case let .paneNotFound(id): "pane not found \(id)"
        case let .paneSurfaceNotReady(id, waited):
            "pane surface not ready \(id) (waited \(String(format: "%.1f", waited))s)"
        case let .projectNotFound(id): "project not found\(id.isEmpty ? "" : " \(id)")"
        case let .worktreeNotFound(id): "worktree not found \(id)"
        case let .tabNotFound(id): "tab not found \(id)"
        case .tabCreateFailed: "could not create tab"
        case let .browserTabNotFound(id): "browser tab not found \(id)"
        case .browserTabCreateFailed: "could not create browser tab"
        case let .browserTabSurfaceNotReady(id, waited):
            "browser tab surface not ready \(id) (waited \(String(format: "%.1f", waited))s)"
        case .browserDisabled: "the built-in browser is disabled"
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
    let sortOrder: Int
    let iconColor: String?
    let icon: String?
    let logo: String?
    let worktreesEnabled: Bool
}

struct WorktreeInfo: Equatable {
    let id: UUID
    let name: String
    let path: String
    let branch: String?
    let isActive: Bool
}

struct AgentInfo: Equatable {
    let worktreeID: UUID
    let projectID: UUID
    let paneID: UUID
    let providerID: String
    let status: AgentStatus
}

struct TabInfo: Equatable {
    let index: Int
    let id: UUID
    let kind: TerminalTab.Kind
    let title: String
    let isActive: Bool
}

struct BrowserTabInfo: Equatable {
    let id: UUID
    let title: String
    let url: String?
    let profile: String
    let isActive: Bool
}

struct BrowserPageContent: Equatable {
    let title: String
    let url: String?
    let text: String
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
    let extensionPayload: ExtensionPayload?
    let directory: String?
    let command: String?

    struct ExtensionPayload: Decodable {
        let id: String
        let tabType: String
        let data: ExtensionJSON?
        let singleton: Bool

        private enum CodingKeys: String, CodingKey {
            case id
            case tabType
            case data
            case singleton
        }

        init(id: String, tabType: String, data: ExtensionJSON?, singleton: Bool = false) {
            self.id = id
            self.tabType = tabType
            self.data = data
            self.singleton = singleton
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            id = try container.decode(String.self, forKey: .id)
            tabType = try container.decode(String.self, forKey: .tabType)
            data = try container.decodeIfPresent(ExtensionJSON.self, forKey: .data)
            singleton = try container.decodeIfPresent(Bool.self, forKey: .singleton) ?? false
        }
    }

    private enum CodingKeys: String, CodingKey {
        case kind
        case `extension`
        case directory
        case command
    }

    init(
        kind: TerminalTab.Kind,
        extensionPayload: ExtensionPayload? = nil,
        directory: String? = nil,
        command: String? = nil
    ) {
        self.kind = kind
        self.extensionPayload = extensionPayload
        self.directory = directory
        self.command = command
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        kind = try container.decode(TerminalTab.Kind.self, forKey: .kind)
        extensionPayload = try container.decodeIfPresent(ExtensionPayload.self, forKey: .extension)
        directory = try container.decodeIfPresent(String.self, forKey: .directory)
        command = try container.decodeIfPresent(String.self, forKey: .command)
    }
}

enum MuxyAPI {
    enum Permissions {
        static func required(for verb: String) -> ExtensionPermission? {
            verbPermissions[canonical(verb)]
        }

        static func required(for verb: String, args: [String: Any]) -> ExtensionPermission? {
            if verb == "browser.wait", let function = args["function"] as? String, !function.isEmpty {
                return .browserWrite
            }
            return required(for: verb)
        }

        static func required(forEvent event: String) -> ExtensionPermission? {
            eventPermissions[event]
        }

        static func canonical(_ verb: String) -> String {
            cliAliases[verb] ?? verb
        }

        static let verbNames: Set<String> = Set(cliAliases.keys).union(extensionVerbs)

        private static let extensionVerbs: Set<String> = Set([
            "exec",
            "agents.list",
            "http.fetch",
            "dialog.confirm",
            "dialog.alert",
            "shortcuts.register",
            "shortcuts.unregister",
            "shortcuts.list",
            "dialog.prompt",
            "dialog.pickFolder",
            "storage.get",
            "storage.set",
            "storage.delete",
            "storage.keys",
            "modal.open",
            "modal.feed",
            "modal.finish",
            "modal.await",
            "extension.settings.get",
            "extension.settings.set",
            "extension.statusbar.set",
            "panel.open",
            "panel.close",
            "panel.toggle",
            "popover.close",
            "popover.resize",
            "topbar.set",
            "statusbar.set",
            "tabs.open",
            "browser.open",
            "browser.navigate",
            "browser.list",
            "browser.read",
            "browser.close",
            "browser.eval",
            "browser.click",
            "browser.type",
            "browser.waitFor",
            "browser.getText",
            "browser.getHTML",
            "browser.getAttribute",
            "browser.reload",
            "browser.back",
            "browser.forward",
            "browser.waitForNavigation",
            "browser.screenshot",
            "browser.storage.get",
            "browser.storage.set",
            "browser.storage.clear",
            "browser.cookies.get",
            "browser.cookies.set",
            "browser.cookies.delete",
            "browser.cookies.clear",
            "browser.wait",
            "browser.fill",
            "browser.press",
            "browser.select",
            "browser.hover",
            "browser.scrollIntoView",
            "browser.setChecked",
            "browser.is",
            "browser.getValue",
            "browser.getCount",
            "browser.find",
            "browser.snapshot",
            "projects.delete",
            "projects.add",
            "projects.rename",
            "projects.setColor",
            "projects.setIcon",
            "projects.setLogo",
            "projects.reorder",
            "lifecycle.ackBeforeClose",
            "lifecycle.resolveBeforeClose",
            "lifecycle.closeSelf",
        ]).union(gitVerbs).union(filesVerbs)

        static let filesVerbs: Set<String> = [
            "files.list",
            "files.read",
            "files.stat",
            "files.write",
            "files.mkdir",
            "files.rename",
            "files.move",
            "files.delete",
        ]

        static let gitVerbs: Set<String> = [
            "git.status",
            "git.diff",
            "git.repoInfo",
            "git.log",
            "git.branches",
            "git.currentBranch",
            "git.aheadBehind",
            "git.pr.info",
            "git.pr.number",
            "git.pr.diff",
            "git.pr.list",
            "git.worktrees",
            "git.init",
            "git.stage",
            "git.unstage",
            "git.discard",
            "git.commit",
            "git.push",
            "git.pull",
            "git.branch.create",
            "git.branch.switch",
            "git.pr.create",
            "git.pr.merge",
            "git.pr.close",
            "git.worktree.add",
            "git.worktree.remove",
            "git.worktree.switch",
            "git.remoteBranches",
            "git.branch.delete",
            "git.branch.deleteRemote",
            "git.checkout",
            "git.cherryPick",
            "git.revert",
            "git.tag.create",
            "git.pr.checkout",
            "git.pr.checkoutWorktree",
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
            "tabs.setTitle": .tabsWrite,
            "tabs.setIcon": .tabsWrite,
            "browser.open": .browserWrite,
            "browser.navigate": .browserWrite,
            "browser.list": .browserRead,
            "browser.read": .browserRead,
            "browser.close": .browserWrite,
            "browser.eval": .browserWrite,
            "browser.click": .browserWrite,
            "browser.type": .browserWrite,
            "browser.waitFor": .browserRead,
            "browser.getText": .browserRead,
            "browser.getHTML": .browserRead,
            "browser.getAttribute": .browserRead,
            "browser.reload": .browserWrite,
            "browser.back": .browserWrite,
            "browser.forward": .browserWrite,
            "browser.waitForNavigation": .browserRead,
            "browser.screenshot": .browserRead,
            "browser.storage.get": .browserRead,
            "browser.storage.set": .browserWrite,
            "browser.storage.clear": .browserWrite,
            "browser.cookies.get": .browserRead,
            "browser.cookies.set": .browserWrite,
            "browser.cookies.delete": .browserWrite,
            "browser.cookies.clear": .browserWrite,
            "browser.wait": .browserRead,
            "browser.fill": .browserWrite,
            "browser.press": .browserWrite,
            "browser.select": .browserWrite,
            "browser.hover": .browserWrite,
            "browser.scrollIntoView": .browserWrite,
            "browser.setChecked": .browserWrite,
            "browser.is": .browserRead,
            "browser.getValue": .browserRead,
            "browser.getCount": .browserRead,
            "browser.find": .browserRead,
            "browser.snapshot": .browserRead,
            "projects.list": .projectsRead,
            "projects.switch": .projectsWrite,
            "projects.delete": .projectsDelete,
            "projects.add": .projectsWrite,
            "projects.rename": .projectsWrite,
            "projects.setColor": .projectsWrite,
            "projects.setIcon": .projectsWrite,
            "projects.setLogo": .projectsWrite,
            "projects.reorder": .projectsWrite,
            "worktrees.list": .worktreesRead,
            "worktrees.create": .worktreesWrite,
            "worktrees.switch": .worktreesWrite,
            "worktrees.refresh": .worktreesWrite,
            "agents.list": .agentsRead,
            "git.status": .gitRead,
            "git.diff": .gitRead,
            "git.repoInfo": .gitRead,
            "git.log": .gitRead,
            "git.branches": .gitRead,
            "git.currentBranch": .gitRead,
            "git.aheadBehind": .gitRead,
            "git.pr.info": .gitRead,
            "git.pr.number": .gitRead,
            "git.pr.diff": .gitRead,
            "git.pr.list": .gitRead,
            "git.worktrees": .gitRead,
            "git.init": .gitWrite,
            "git.stage": .gitWrite,
            "git.unstage": .gitWrite,
            "git.discard": .gitWrite,
            "git.commit": .gitWrite,
            "git.push": .gitWrite,
            "git.pull": .gitWrite,
            "git.branch.create": .gitWrite,
            "git.branch.switch": .gitWrite,
            "git.pr.create": .gitWrite,
            "git.pr.merge": .gitWrite,
            "git.pr.close": .gitWrite,
            "git.worktree.add": .gitWrite,
            "git.worktree.remove": .gitWrite,
            "git.worktree.switch": .gitWrite,
            "git.remoteBranches": .gitRead,
            "git.branch.delete": .gitWrite,
            "git.branch.deleteRemote": .gitWrite,
            "git.checkout": .gitWrite,
            "git.cherryPick": .gitWrite,
            "git.revert": .gitWrite,
            "git.tag.create": .gitWrite,
            "git.pr.checkout": .gitWrite,
            "git.pr.checkoutWorktree": .gitWrite,
            "files.list": .filesRead,
            "files.read": .filesRead,
            "files.stat": .filesRead,
            "files.write": .filesWrite,
            "files.mkdir": .filesWrite,
            "files.rename": .filesWrite,
            "files.move": .filesWrite,
            "files.delete": .filesWrite,
            "toast": .notificationsWrite,
            "notifications.notify": .notificationsWrite,
            "panel.open": .panelsWrite,
            "panel.close": .panelsWrite,
            "panel.toggle": .panelsWrite,
            "popover.close": .panelsWrite,
            "popover.resize": .panelsWrite,
            "topbar.set": .panelsWrite,
            "statusbar.set": .panelsWrite,
            "exec": .commandsExec,
            "shortcuts.register": .shortcutsRegister,
            "shortcuts.unregister": .shortcutsRegister,
            "storage.get": .storageRead,
            "storage.keys": .storageRead,
            "storage.set": .storageWrite,
            "storage.delete": .storageWrite,
        ]

        private static let eventPermissions: [String: ExtensionPermission] = [
            ExtensionEventName.agentStatus: .agentsRead,
            ExtensionEventName.fileChanged: .filesRead,
            ExtensionEventName.projectsChanged: .projectsRead,
            ExtensionEventName.worktreeHeadChanged: .worktreesRead,
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
    enum Popovers {
        static func close(extensionID: String) -> Result<Void, APIError> {
            PopoverHost.shared.requestClose(extensionID: extensionID)
            return .success(())
        }

        static func resize(extensionID: String, width: Double, height: Double) -> Result<Void, APIError> {
            guard width > 0, height > 0 else {
                return .failure(.invalidArguments("popover.resize requires positive width and height"))
            }
            PopoverHost.shared.resize(extensionID: extensionID, width: width, height: height)
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

            let effects = appState.dispatchReturningEffects(.splitArea(.init(
                projectID: projectID,
                areaID: areaID,
                direction: direction,
                position: .second,
                command: finalCommand
            )))

            guard let newPaneID = effects.createdPaneID else {
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
            switch await waitForView(paneID: paneID, appState: appState) {
            case let .view(view):
                view.sendText(text)
                return .success(())
            case .notFound:
                return .failure(.paneNotFound(paneIDString))
            case let .surfaceNotReady(waited):
                return .failure(.paneSurfaceNotReady(
                    paneID: paneIDString,
                    waitedSeconds: waited.secondsValue
                ))
            }
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
            switch await waitForView(paneID: paneID, appState: appState) {
            case let .view(view):
                return Self.performSendKeys(view: view, key: key, paneIDString: paneIDString)
            case .notFound:
                return .failure(.paneNotFound(paneIDString))
            case let .surfaceNotReady(waited):
                return .failure(.paneSurfaceNotReady(
                    paneID: paneIDString,
                    waitedSeconds: waited.secondsValue
                ))
            }
        }

        private static func performSendKeys(
            view: GhosttyTerminalNSView,
            key: String,
            paneIDString: String
        ) -> Result<Void, APIError> {
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
            switch await waitForView(paneID: paneID, appState: appState) {
            case let .view(view):
                return .success(view.readScreenText(lastLines: clamped))
            case .notFound:
                return .failure(.paneNotFound(paneIDString))
            case let .surfaceNotReady(waited):
                return .failure(.paneSurfaceNotReady(
                    paneID: paneIDString,
                    waitedSeconds: waited.secondsValue
                ))
            }
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
            appState.closeTab(loc.tabID, areaID: loc.areaID, projectID: loc.key.projectID)
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
        struct Context {
            let extensionID: String
            let appState: AppState
            let projectStore: ProjectStore
            let worktreeStore: WorktreeStore
            let projectGroupStore: ProjectGroupStore
        }

        static func list(appState: AppState, projectStore: ProjectStore) -> [ProjectInfo] {
            projectStore.projects.map { project in
                ProjectInfo(
                    id: project.id,
                    name: project.name,
                    path: project.path,
                    isActive: project.id == appState.activeProjectID,
                    sortOrder: project.sortOrder,
                    iconColor: project.iconColor,
                    icon: project.icon,
                    logo: project.logo,
                    worktreesEnabled: project.worktreesEnabled
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

        static func delete(
            identifier: String,
            context: Context
        ) async -> Result<Void, APIError> {
            guard let project = context.projectGroupStore.resolveProject(
                identifier: identifier,
                localProjects: context.projectStore.projects,
                activeProjectID: context.appState.activeProjectID
            )
            else {
                return .failure(.projectNotFound(identifier))
            }
            guard project.id != Project.homeID else {
                return .failure(.invalidArguments("the home project cannot be deleted"))
            }
            let consent = ExtensionConsentRequestBuilder.make(
                extensionID: context.extensionID,
                verb: .projectsDelete,
                payload: .project(name: project.name, path: project.path),
                source: "muxy-api"
            )
            guard await ExtensionConsentService.shared.gate(consent) == .allow else {
                return .failure(.consentDenied(verb: "projects.delete"))
            }
            do {
                try await ProjectRemovalService.remove(
                    project,
                    appState: context.appState,
                    projectStore: context.projectStore,
                    worktreeStore: context.worktreeStore,
                    projectGroupStore: context.projectGroupStore
                )
                return .success(())
            } catch {
                return .failure(.underlying(error.localizedDescription))
            }
        }

        static func add(path: String, context: Context) -> Result<UUID, APIError> {
            let before = Set(context.projectStore.projects.map(\.id))
            let confirmed = ProjectOpenService.confirmProjectPath(
                path,
                appState: context.appState,
                projectStore: context.projectStore,
                worktreeStore: context.worktreeStore,
                projectGroupStore: context.projectGroupStore,
                createIfMissing: false
            )
            guard confirmed, let project = findProject(path, in: context.projectStore.projects) else {
                return .failure(.invalidArguments("could not open project at path '\(path)'"))
            }
            if project.id != Project.homeID, !before.contains(project.id) {
                context.projectStore.setWorktreesEnabled(id: project.id, to: true)
            }
            return .success(project.id)
        }

        static func rename(identifier: String, name: String, context: Context) -> Result<Void, APIError> {
            let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else {
                return .failure(.invalidArguments("name cannot be empty"))
            }
            return resolveMutableProject(identifier, context: context).map {
                context.projectStore.rename(id: $0.id, to: trimmed)
            }
        }

        static func setColor(identifier: String, color: String?, context: Context) -> Result<Void, APIError> {
            resolveMutableProject(identifier, context: context).map {
                context.projectStore.setIconColor(id: $0.id, to: color)
            }
        }

        static func setIcon(identifier: String, icon: String?, context: Context) -> Result<Void, APIError> {
            resolveMutableProject(identifier, context: context).map {
                context.projectStore.setIcon(id: $0.id, to: icon)
            }
        }

        static func setLogo(identifier: String, logo: String?, context: Context) -> Result<Void, APIError> {
            switch resolveMutableProject(identifier, context: context) {
            case let .success(project):
                if let logo, !ProjectLogoStorage.isStoredLogoFilename(logo, forProjectID: project.id) {
                    return .failure(.invalidArguments("invalid project logo"))
                }
                context.projectStore.setLogo(id: project.id, to: logo)
                return .success(())
            case let .failure(error):
                return .failure(error)
            }
        }

        static func reorder(identifiers: [String], context: Context) -> Result<Void, APIError> {
            var orderedIDs: [UUID] = []
            for identifier in identifiers {
                switch resolveMutableProject(identifier, context: context) {
                case let .success(project): orderedIDs.append(project.id)
                case let .failure(error): return .failure(error)
                }
            }
            let reorderable = Set(context.projectStore.projects.lazy.filter { !$0.isHome && !$0.isRemote }.map(\.id))
            guard orderedIDs.count == reorderable.count, Set(orderedIDs) == reorderable else {
                return .failure(.invalidArguments("identifiers must list every project exactly once"))
            }
            context.projectStore.persistOrder(orderedIDs, scopedTo: reorderable)
            return .success(())
        }

        private static func resolveMutableProject(_ identifier: String, context: Context) -> Result<Project, APIError> {
            guard let project = context.projectGroupStore.resolveProject(
                identifier: identifier,
                localProjects: context.projectStore.projects,
                activeProjectID: context.appState.activeProjectID
            )
            else {
                return .failure(.projectNotFound(identifier))
            }
            guard project.id != Project.homeID else {
                return .failure(.invalidArguments("the home project cannot be modified"))
            }
            guard !project.isRemote else {
                return .failure(.invalidArguments("remote projects cannot be modified"))
            }
            return .success(project)
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

            let workspaceContext = ActiveWorkspaceContext.shared.current
            let expandedPath = workspaceContext.isRemote ? trimmedPath : NSString(string: trimmedPath).expandingTildeInPath
            let path = trimmedPath.isEmpty
                ? WorktreeLocationResolver.worktreeDirectory(for: project, slug: slug(from: trimmedName))
                : expandedPath
            guard await !workspaceContext.fileOps.exists(at: path) else {
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
                    ),
                    context: workspaceContext
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
            worktreeStore: WorktreeStore,
            projectGroupStore: ProjectGroupStore? = nil
        ) async -> Result<RefreshWorktreesResult, APIError> {
            guard let project = resolveProject(projectIdentifier, appState: appState, projectStore: projectStore) else {
                return .failure(.projectNotFound(projectIdentifier ?? ""))
            }
            do {
                let context = projectGroupStore?.workspaceContext(for: project)
                    ?? (project.isRemote ? ActiveWorkspaceContext.shared.current : .local)
                let worktrees = try await worktreeStore.refreshFromGit(project: project, context: context)
                return .success(RefreshWorktreesResult(count: worktrees.count))
            } catch {
                return .failure(.underlying(error.localizedDescription))
            }
        }
    }

    @MainActor
    enum Agents {
        static func list() -> [AgentInfo] {
            AgentStatusStore.shared.entries.values.map { entry in
                AgentInfo(
                    worktreeID: entry.worktreeID,
                    projectID: entry.projectID,
                    paneID: entry.paneID,
                    providerID: entry.providerID,
                    status: entry.status
                )
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
            let effects = appState.dispatchReturningEffects(.createTab(projectID: projectID, areaID: nil))
            return .success(effects.createdTabID)
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

        static func setTitle(
            instanceID: String,
            title: String,
            appState: AppState,
            callingExtensionID: String
        ) -> Result<Void, APIError> {
            locateExtensionTab(instanceID: instanceID, callingExtensionID: callingExtensionID, appState: appState)
                .map { state in
                    let trimmed = title.trimmingCharacters(in: .whitespacesAndNewlines)
                    state.customTitle = trimmed.isEmpty ? nil : title
                }
        }

        static func setIcon(
            instanceID: String,
            icon: ExtensionIcon?,
            appState: AppState,
            callingExtensionID: String
        ) -> Result<Void, APIError> {
            locateExtensionTab(instanceID: instanceID, callingExtensionID: callingExtensionID, appState: appState)
                .map { state in state.customIcon = icon }
        }

        private static func locateExtensionTab(
            instanceID: String,
            callingExtensionID: String,
            appState: AppState
        ) -> Result<ExtensionTabState, APIError> {
            guard let id = UUID(uuidString: instanceID) else {
                return .failure(.invalidArguments("invalid tab instance id"))
            }
            for (_, root) in appState.workspaceRoots {
                for area in root.allAreas() {
                    for tab in area.tabs {
                        guard let state = tab.content.extensionState, state.id == id else { continue }
                        guard state.extensionID == callingExtensionID else {
                            return .failure(.tabNotFound(instanceID))
                        }
                        return .success(state)
                    }
                }
            }
            return .failure(.tabNotFound(instanceID))
        }

        static func open(
            _ request: OpenTabRequest,
            appState: AppState,
            callingExtensionID: String? = nil,
            consent: ExtensionConsentService = .shared
        ) async -> Result<UUID, APIError> {
            let target: OpenTabTarget
            switch resolveOpenTarget(appState: appState) {
            case let .success(resolved):
                target = resolved
            case let .failure(error):
                return .failure(error)
            }
            switch request.kind {
            case .terminal:
                return await openTerminalTab(
                    request,
                    target: target,
                    appState: appState,
                    callingExtensionID: callingExtensionID,
                    consent: consent
                )
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
                activateOpenTarget(target, appState: appState)
                let effects = appState.dispatchReturningEffects(.createExtensionTab(
                    projectID: target.key.projectID,
                    areaID: target.areaID,
                    request: AppState.CreateExtensionTabRequest(
                        extensionID: payload.id,
                        tabTypeID: payload.tabType,
                        title: tabType.title,
                        data: payload.data ?? tabType.defaultData,
                        singleton: payload.singleton
                    )
                ))
                guard let tabID = effects.createdTabID else { return .failure(.tabCreateFailed) }
                return .success(tabID)
            case .browser:
                return .failure(.invalidArguments("browser tabs cannot be opened via this API yet"))
            }
        }

        private static func openTerminalTab(
            _ request: OpenTabRequest,
            target: OpenTabTarget,
            appState: AppState,
            callingExtensionID: String?,
            consent: ExtensionConsentService
        ) async -> Result<UUID, APIError> {
            let workspaceContext = ActiveWorkspaceContext.shared.current
            var resolvedDirectory: String?
            if let directory = request.directory {
                guard let root = appState.workspaceRoots[target.key]?.findArea(id: target.areaID)?.projectPath,
                      let resolved = resolveTabDirectory(root: root, relativePath: directory, context: workspaceContext),
                      await directoryExists(at: resolved, context: workspaceContext)
                else {
                    return .failure(.invalidArguments("directory must be an existing folder inside the worktree"))
                }
                resolvedDirectory = resolved
            }

            let trimmedCommand = request.command?.trimmingCharacters(in: .whitespacesAndNewlines)
            guard let command = trimmedCommand, !command.isEmpty else {
                activateOpenTarget(target, appState: appState)
                let effects: WorkspaceSideEffects = if let directory = resolvedDirectory {
                    appState.dispatchReturningEffects(.createTabInDirectory(
                        projectID: target.key.projectID,
                        areaID: target.areaID,
                        directory: directory
                    ))
                } else {
                    appState.dispatchReturningEffects(
                        .createTab(projectID: target.key.projectID, areaID: target.areaID)
                    )
                }
                guard let tabID = effects.createdTabID else { return .failure(.tabCreateFailed) }
                return .success(tabID)
            }

            guard let callingExtensionID else {
                return .failure(.consentDenied(verb: ExtensionGatedVerb.tabsRunCommand.rawValue))
            }
            let allowed = await tabCommandConsentGranted(
                extensionID: callingExtensionID,
                command: command,
                consent: consent
            )
            guard allowed else {
                return .failure(.consentDenied(verb: ExtensionGatedVerb.tabsRunCommand.rawValue))
            }

            activateOpenTarget(target, appState: appState)
            let effects = appState.dispatchReturningEffects(.createCommandTab(CommandTabRequest(
                projectID: target.key.projectID,
                areaID: target.areaID,
                name: command,
                command: command,
                closesOnCommandExit: false,
                directory: resolvedDirectory
            )))
            guard let tabID = effects.createdTabID else { return .failure(.tabCreateFailed) }
            return .success(tabID)
        }

        static func resolveTabDirectory(
            root: String,
            relativePath: String,
            context: WorkspaceContext
        ) -> String? {
            guard context.isRemote else {
                return Files.resolve(root: root, relativePath: relativePath)
            }
            let normalizedRoot = ProjectPickerPathService.standardizedRemotePath(root)
            let trimmed = relativePath.hasPrefix("/") ? String(relativePath.dropFirst()) : relativePath
            let joined = trimmed.isEmpty ? normalizedRoot : normalizedRoot + "/" + trimmed
            let resolved = ProjectPickerPathService.standardizedRemotePath(joined)
            guard resolved == normalizedRoot || resolved.hasPrefix(normalizedRoot + "/") else { return nil }
            return resolved
        }

        private static func directoryExists(at path: String, context: WorkspaceContext) async -> Bool {
            guard context.isRemote else {
                var isDirectory: ObjCBool = false
                let exists = FileManager.default.fileExists(atPath: path, isDirectory: &isDirectory)
                return exists && isDirectory.boolValue
            }
            guard case let .ssh(destination) = context else { return false }
            let quoted = RemoteCommandBuilder.quoteRemotePath(path)
            let result = try? await SSHCommandRunner.run(
                destination: destination,
                remoteCommand: "[ -d \(quoted) ] && echo yes || true"
            )
            return result?.stdout.trimmingCharacters(in: .whitespacesAndNewlines) == "yes"
        }

        private static func activateOpenTarget(_ target: OpenTabTarget, appState: AppState) {
            appState.dispatch(.navigate(
                projectID: target.key.projectID,
                worktreeID: target.key.worktreeID,
                areaID: target.areaID,
                tabID: nil
            ))
        }

        private struct OpenTabTarget {
            let key: WorktreeKey
            let areaID: UUID
        }

        private static func resolveOpenTarget(appState: AppState) -> Result<OpenTabTarget, APIError> {
            if let entry = appState.navigation.current {
                let key = WorktreeKey(projectID: entry.projectID, worktreeID: entry.worktreeID)
                if let target = openTarget(key: key, preferredAreaID: entry.areaID, appState: appState) {
                    return .success(target)
                }
            }
            if let projectID = appState.activeProjectID,
               let key = appState.activeWorktreeKey(for: projectID),
               let target = openTarget(key: key, preferredAreaID: appState.focusedAreaID[key], appState: appState)
            {
                return .success(target)
            }
            if appState.workspaceRoots.count == 1,
               let key = appState.workspaceRoots.keys.first,
               let target = openTarget(key: key, preferredAreaID: appState.focusedAreaID[key], appState: appState)
            {
                return .success(target)
            }
            return .failure(appState.workspaceRoots.isEmpty ? .noActiveProject : .noActiveWorkspace)
        }

        private static func openTarget(
            key: WorktreeKey,
            preferredAreaID: UUID?,
            appState: AppState
        ) -> OpenTabTarget? {
            guard let root = appState.workspaceRoots[key] else { return nil }
            if let preferredAreaID,
               let area = root.findArea(id: preferredAreaID)
            {
                return OpenTabTarget(key: key, areaID: area.id)
            }
            guard let area = root.allAreas().first else { return nil }
            return OpenTabTarget(key: key, areaID: area.id)
        }
    }

    @MainActor
    enum Browser {
        static func open(
            url: String?,
            profileID: UUID? = nil,
            split: Bool = false,
            appState: AppState
        ) -> Result<UUID, APIError> {
            guard BrowserPreferences.isEnabled else { return .failure(.browserDisabled) }
            guard let projectID = appState.activeProjectID else { return .failure(.noActiveProject) }
            let resolvedURL = url.flatMap(BrowserURL.resolve(from:)) ?? BrowserURL.homeURL
            let resolvedProfileID = profileID ?? BrowserPreferences.defaultProfileID

            if split, let areaID = appState.focusedAreaID(for: projectID) {
                appState.dispatch(.splitArea(.init(
                    projectID: projectID,
                    areaID: areaID,
                    direction: .horizontal,
                    position: .second
                )))
            }

            let effects = appState.dispatchReturningEffects(.createBrowserTab(
                projectID: projectID,
                areaID: appState.focusedAreaID(for: projectID),
                url: resolvedURL,
                profileID: resolvedProfileID
            ))
            guard let newID = effects.createdTabID else {
                return .failure(.browserTabCreateFailed)
            }
            return .success(newID)
        }

        static func navigate(tabIDString: String, url: String, appState: AppState) -> Result<Void, APIError> {
            guard BrowserPreferences.isEnabled else { return .failure(.browserDisabled) }
            guard let state = locate(tabIDString: tabIDString, appState: appState)?.state else {
                return .failure(.browserTabNotFound(tabIDString))
            }
            guard let resolved = BrowserURL.resolve(from: url) else {
                return .failure(.invalidArguments("invalid url"))
            }
            state.pendingURL = resolved
            return .success(())
        }

        static func list(appState: AppState, profileStore: BrowserProfileStore?) -> [BrowserTabInfo] {
            guard BrowserPreferences.isEnabled else { return [] }
            var infos: [BrowserTabInfo] = []
            for (key, root) in appState.workspaceRoots {
                let focusedAreaID = appState.focusedAreaID[key]
                for area in root.allAreas() {
                    for tab in area.tabs {
                        guard let state = tab.content.browserState else { continue }
                        let isActive = area.id == focusedAreaID && tab.id == area.activeTabID
                        let profileName = profileStore?.profile(id: state.profileID)?.name ?? BrowserProfile.defaultName
                        infos.append(BrowserTabInfo(
                            id: tab.id,
                            title: tab.title,
                            url: state.url?.absoluteString,
                            profile: profileName,
                            isActive: isActive
                        ))
                    }
                }
            }
            return infos
        }

        private static let readTextLimit = 1_000_000
        private static let readSurfaceTimeout: Duration = .seconds(3)

        static func read(tabIDString: String, appState: AppState) async -> Result<BrowserPageContent, APIError> {
            guard BrowserPreferences.isEnabled else { return .failure(.browserDisabled) }
            guard let located = locate(tabIDString: tabIDString, appState: appState) else {
                return .failure(.browserTabNotFound(tabIDString))
            }
            let start = ContinuousClock.now
            guard let webView = await waitForRegisteredWebView(tabID: located.state.id, start: start) else {
                let waited = (ContinuousClock.now - start).secondsValue
                return .failure(.browserTabSurfaceNotReady(tabID: tabIDString, waitedSeconds: waited))
            }
            let script = "({ title: document.title, text: (document.body ? document.body.innerText : '').slice(0, \(readTextLimit)) })"
            do {
                let result = try await webView.evaluateJavaScript(script)
                let dict = result as? [String: Any]
                return .success(BrowserPageContent(
                    title: dict?["title"] as? String ?? located.state.pageTitle ?? "",
                    url: located.state.url?.absoluteString,
                    text: dict?["text"] as? String ?? ""
                ))
            } catch {
                return .failure(.underlying(error.localizedDescription))
            }
        }

        static func close(tabIDString: String, appState: AppState) -> Result<Void, APIError> {
            guard BrowserPreferences.isEnabled else { return .failure(.browserDisabled) }
            guard let located = locate(tabIDString: tabIDString, appState: appState) else {
                return .failure(.browserTabNotFound(tabIDString))
            }
            appState.closeTab(located.tabID, areaID: located.areaID, projectID: located.projectID)
            return .success(())
        }

        static func waitForRegisteredWebView(
            tabID: UUID,
            start: ContinuousClock.Instant
        ) async -> WKWebView? {
            let deadline = start + readSurfaceTimeout
            while ContinuousClock.now < deadline {
                if let webView = BrowserWebViewRegistry.shared.webView(for: tabID) {
                    return webView
                }
                do {
                    try await Task.sleep(for: .milliseconds(50))
                } catch {
                    return nil
                }
            }
            return BrowserWebViewRegistry.shared.webView(for: tabID)
        }

        private struct Located {
            let state: BrowserTabState
            let tabID: UUID
            let areaID: UUID
            let projectID: UUID
        }

        private static func locate(tabIDString: String, appState: AppState) -> Located? {
            guard let id = UUID(uuidString: tabIDString) else { return nil }
            for (key, root) in appState.workspaceRoots {
                for area in root.allAreas() {
                    for tab in area.tabs where tab.id == id {
                        guard let state = tab.content.browserState else { return nil }
                        return Located(state: state, tabID: tab.id, areaID: area.id, projectID: key.projectID)
                    }
                }
            }
            return nil
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
private func tabCommandConsentGranted(
    extensionID: String,
    command: String,
    consent: ExtensionConsentService
) async -> Bool {
    let request = ExtensionConsentRequestBuilder.make(
        extensionID: extensionID,
        verb: .tabsRunCommand,
        payload: .tabCommand(command: command),
        source: "muxy-api"
    )
    let decision = await consent.gate(request)
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

extension Duration {
    var secondsValue: Double {
        Double(components.seconds) + Double(components.attoseconds) / 1e18
    }
}

private enum WaitForViewResult {
    case view(GhosttyTerminalNSView)
    case notFound
    case surfaceNotReady(waited: Duration)
}

@MainActor
private func waitForView(
    paneID: UUID,
    appState: AppState? = nil,
    timeout: Duration = .seconds(3)
) async -> WaitForViewResult {
    let start = ContinuousClock.now
    guard let appState else {
        return await waitForRegisteredView(paneID: paneID, start: start, timeout: timeout)
    }
    guard appState.locatePane(paneID: paneID) != nil else {
        return .notFound
    }
    if let view = TerminalSurfaceMaterializer.materialize(paneID: paneID, appState: appState) {
        return .view(view)
    }
    return .surfaceNotReady(waited: ContinuousClock.now - start)
}

@MainActor
private func waitForRegisteredView(
    paneID: UUID,
    start: ContinuousClock.Instant,
    timeout: Duration
) async -> WaitForViewResult {
    let deadline = ContinuousClock.now + timeout
    while ContinuousClock.now < deadline {
        if let view = TerminalViewRegistry.shared.existingView(for: paneID) {
            return view.ensureLiveSurfaceForExternalIO()
                ? .view(view)
                : .surfaceNotReady(waited: ContinuousClock.now - start)
        }
        do {
            try await Task.sleep(for: .milliseconds(50))
        } catch {
            return .surfaceNotReady(waited: ContinuousClock.now - start)
        }
    }
    return .notFound
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
