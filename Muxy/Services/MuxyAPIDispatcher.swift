import Foundation

struct ExtensionAPIStores {
    weak var projectStore: ProjectStore?
    weak var worktreeStore: WorktreeStore?
    weak var projectGroupStore: ProjectGroupStore?

    init(
        projectStore: ProjectStore? = nil,
        worktreeStore: WorktreeStore? = nil,
        projectGroupStore: ProjectGroupStore? = nil
    ) {
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
        self.projectGroupStore = projectGroupStore
    }
}

@MainActor
enum MuxyAPIDispatcher {
    struct Context {
        let extensionID: String
        let appState: AppState
        let projectStore: ProjectStore?
        let worktreeStore: WorktreeStore?
        var projectGroupStore: ProjectGroupStore?

        init(
            extensionID: String,
            appState: AppState,
            projectStore: ProjectStore?,
            worktreeStore: WorktreeStore?,
            projectGroupStore: ProjectGroupStore?
        ) {
            self.extensionID = extensionID
            self.appState = appState
            self.projectStore = projectStore
            self.worktreeStore = worktreeStore
            self.projectGroupStore = projectGroupStore
        }

        init(extensionID: String, appState: AppState, stores: ExtensionAPIStores) {
            self.init(
                extensionID: extensionID,
                appState: appState,
                projectStore: stores.projectStore,
                worktreeStore: stores.worktreeStore,
                projectGroupStore: stores.projectGroupStore
            )
        }
    }

    static func dispatch(verb: String, args: [String: Any], context: Context) async throws -> Any {
        if let required = MuxyAPI.Permissions.required(for: verb),
           !ExtensionStore.shared.extensionHasPermission(id: context.extensionID, permission: required)
        {
            throw APIError.underlying("permission denied (\(required.rawValue))")
        }
        switch verb {
        case "toast",
             "notifications.notify":
            return try await handleNotify(args: args, context: context)
        case "panel.open":
            try unwrap(MuxyAPI.Panels.open(
                extensionID: context.extensionID,
                panelID: stringArg(args, "panel"),
                data: panelData(args),
                toggle: false
            ))
            return NSNull()
        case "panel.toggle":
            try unwrap(MuxyAPI.Panels.open(
                extensionID: context.extensionID,
                panelID: stringArg(args, "panel"),
                data: panelData(args),
                toggle: true
            ))
            return NSNull()
        case "panel.close":
            try unwrap(MuxyAPI.Panels.close(
                extensionID: context.extensionID,
                panelID: stringArg(args, "panel")
            ))
            return NSNull()
        case "popover.close":
            try unwrap(MuxyAPI.Popovers.close(extensionID: context.extensionID))
            return NSNull()
        case "popover.resize":
            try unwrap(MuxyAPI.Popovers.resize(
                extensionID: context.extensionID,
                width: doubleArg(args, "width"),
                height: doubleArg(args, "height")
            ))
            return NSNull()
        case "topbar.set":
            let topbarItemID = try stringArg(args, "id")
            guard ExtensionStore.shared.setTopbarItem(
                extensionID: context.extensionID,
                itemID: topbarItemID,
                icon: ExtensionIcon.parse(args["icon"]),
                visible: args["visible"] as? Bool
            )
            else {
                throw APIError.invalidArguments("unknown topbar item '\(topbarItemID)'")
            }
            return NSNull()
        case "statusbar.set":
            let statusItemID = try stringArg(args, "id")
            let rawText = args["text"] as? String
            guard ExtensionStore.shared.setStatusBarItem(
                extensionID: context.extensionID,
                itemID: statusItemID,
                update: ExtensionStore.StatusBarUpdate(
                    icon: ExtensionIcon.parse(args["icon"]),
                    text: (rawText?.isEmpty == true) ? nil : rawText,
                    clearText: args.keys.contains("text"),
                    visible: args["visible"] as? Bool
                )
            )
            else {
                throw APIError.invalidArguments("unknown status bar item '\(statusItemID)'")
            }
            return NSNull()
        case "exec":
            return try await handleExec(args: args, context: context)
        case "http.fetch":
            return try await handleHTTPFetch(args: args, context: context)
        case "dialog.confirm":
            let request = try ExtensionDialogService.makeConfirmRequest(extensionID: context.extensionID, args: args)
            return try await ExtensionDialogService.confirm(request) ?? NSNull()
        case "dialog.alert":
            let request = try ExtensionDialogService.makeAlertRequest(extensionID: context.extensionID, args: args)
            try await ExtensionDialogService.alert(request)
            return NSNull()
        case "modal.open":
            let requestID = ExtensionModalService.shared.openSession(extensionID: context.extensionID, args: args)
            return ["requestID": requestID]
        case "modal.feed":
            ExtensionModalService.shared.feedSession(modalItems(args))
            return NSNull()
        case "modal.finish":
            ExtensionModalService.shared.finishSession()
            return NSNull()
        case "modal.await":
            let requestID = (args["requestID"] as? String) ?? ""
            let selected = await ExtensionModalService.shared.awaitSelection(requestID: requestID)
            return selected.map(modalItemDict) ?? NSNull()
        case "tabs.list":
            return try unwrap(MuxyAPI.Tabs.list(appState: context.appState)).map(tabDict)
        case "tabs.switch":
            try unwrap(MuxyAPI.Tabs.switchTo(
                identifier: stringArg(args, "identifier"),
                appState: context.appState
            ))
            return NSNull()
        case "tabs.new":
            return try unwrap(MuxyAPI.Tabs.new(appState: context.appState))?.uuidString ?? NSNull()
        case "tabs.next":
            try unwrap(MuxyAPI.Tabs.next(appState: context.appState))
            return NSNull()
        case "tabs.previous":
            try unwrap(MuxyAPI.Tabs.previous(appState: context.appState))
            return NSNull()
        case "tabs.open":
            try await unwrap(MuxyAPI.Tabs.open(
                decodeOpenTabRequest(args),
                appState: context.appState,
                callingExtensionID: context.extensionID
            ))
            return NSNull()
        case "tabs.setTitle":
            try unwrap(MuxyAPI.Tabs.setTitle(
                instanceID: stringArg(args, "tabInstanceID"),
                title: stringArg(args, "title"),
                appState: context.appState,
                callingExtensionID: context.extensionID
            ))
            return NSNull()
        case "tabs.setIcon":
            try unwrap(MuxyAPI.Tabs.setIcon(
                instanceID: stringArg(args, "tabInstanceID"),
                icon: ExtensionIcon.parse(args["icon"]),
                appState: context.appState,
                callingExtensionID: context.extensionID
            ))
            return NSNull()
        case "panes.list":
            return MuxyAPI.Panes.list(appState: context.appState).map(paneDict)
        case "panes.send":
            try await unwrap(MuxyAPI.Panes.send(
                paneIDString: stringArg(args, "paneID"),
                text: stringArg(args, "text"),
                appState: context.appState,
                extensionID: context.extensionID
            ))
            return NSNull()
        case "panes.sendKeys":
            try await unwrap(MuxyAPI.Panes.sendKeys(
                paneIDString: stringArg(args, "paneID"),
                key: stringArg(args, "key"),
                appState: context.appState,
                extensionID: context.extensionID
            ))
            return NSNull()
        case "panes.readScreen":
            let lines = (args["lines"] as? Int) ?? 50
            return try await unwrap(MuxyAPI.Panes.readScreen(
                paneIDString: stringArg(args, "paneID"),
                lines: lines,
                appState: context.appState,
                extensionID: context.extensionID
            ))
        case "panes.close":
            try unwrap(MuxyAPI.Panes.close(
                paneIDString: stringArg(args, "paneID"),
                appState: context.appState
            ))
            return NSNull()
        case "panes.rename":
            try unwrap(MuxyAPI.Panes.rename(
                paneIDString: stringArg(args, "paneID"),
                title: stringArg(args, "title"),
                appState: context.appState
            ))
            return NSNull()
        case "projects.list":
            guard let projectStore = context.projectStore else { throw APIError.projectStoreUnavailable }
            return MuxyAPI.Projects.list(appState: context.appState, projectStore: projectStore).map(projectDict)
        case "projects.switch":
            guard let projectStore = context.projectStore,
                  let worktreeStore = context.worktreeStore
            else { throw APIError.projectStoreUnavailable }
            try unwrap(MuxyAPI.Projects.switchTo(
                identifier: stringArg(args, "identifier"),
                appState: context.appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            ))
            return NSNull()
        case "projects.delete":
            guard let projectStore = context.projectStore,
                  let worktreeStore = context.worktreeStore,
                  let projectGroupStore = context.projectGroupStore
            else { throw APIError.projectStoreUnavailable }
            try await unwrap(MuxyAPI.Projects.delete(
                identifier: stringArg(args, "identifier"),
                context: MuxyAPI.Projects.Context(
                    extensionID: context.extensionID,
                    appState: context.appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore,
                    projectGroupStore: projectGroupStore
                )
            ))
            return NSNull()
        case "worktrees.list":
            guard let projectStore = context.projectStore,
                  let worktreeStore = context.worktreeStore
            else { throw APIError.worktreeStoreUnavailable }
            return try unwrap(MuxyAPI.Worktrees.list(
                projectIdentifier: args["project"] as? String,
                appState: context.appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            )).map(worktreeDict)
        case "worktrees.switch":
            guard let projectStore = context.projectStore,
                  let worktreeStore = context.worktreeStore
            else { throw APIError.worktreeStoreUnavailable }
            try unwrap(MuxyAPI.Worktrees.switchTo(
                identifier: stringArg(args, "identifier"),
                projectIdentifier: args["project"] as? String,
                appState: context.appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            ))
            return NSNull()
        case "worktrees.refresh":
            guard let projectStore = context.projectStore,
                  let worktreeStore = context.worktreeStore
            else { throw APIError.worktreeStoreUnavailable }
            let result = try await unwrap(MuxyAPI.Worktrees.refresh(
                projectIdentifier: args["project"] as? String,
                appState: context.appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: context.projectGroupStore
            ))
            return ["count": result.count]
        default:
            if verb.hasPrefix("git.") {
                return try await handleGit(verb: verb, args: args, context: context)
            }
            if verb.hasPrefix("files.") {
                return try await handleFiles(verb: verb, args: args, context: context)
            }
            throw APIError.invalidArguments("unknown verb \(verb)")
        }
    }

    private static func handleFiles(verb: String, args: [String: Any], context: Context) async throws -> Any {
        guard let projectStore = context.projectStore,
              let worktreeStore = context.worktreeStore,
              let projectGroupStore = context.projectGroupStore
        else { throw APIError.worktreeStoreUnavailable }
        let files = MuxyAPI.Files.Context(
            extensionID: context.extensionID,
            appState: context.appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
        let project = args["project"] as? String

        switch verb {
        case "files.list":
            return try await unwrap(MuxyAPI.Files.list(
                projectIdentifier: project,
                path: stringArg(args, "path"),
                context: files
            )).map(FilesDTO.entry)
        case "files.read":
            return try await FilesDTO.readResult(unwrap(MuxyAPI.Files.read(
                projectIdentifier: project,
                path: stringArg(args, "path"),
                context: files
            )))
        case "files.stat":
            return try await FilesDTO.stat(unwrap(MuxyAPI.Files.stat(
                projectIdentifier: project,
                path: stringArg(args, "path"),
                context: files
            )))
        case "files.write":
            let path = try await unwrap(MuxyAPI.Files.write(
                projectIdentifier: project,
                path: stringArg(args, "path"),
                contents: stringArg(args, "contents"),
                context: files
            ))
            return ["path": path]
        case "files.mkdir":
            let path = try await unwrap(MuxyAPI.Files.mkdir(
                projectIdentifier: project,
                path: stringArg(args, "path"),
                context: files
            ))
            return ["path": path]
        case "files.rename":
            let path = try await unwrap(MuxyAPI.Files.rename(
                projectIdentifier: project,
                path: stringArg(args, "path"),
                newName: stringArg(args, "newName"),
                context: files
            ))
            return ["path": path]
        case "files.move":
            return try await unwrap(MuxyAPI.Files.move(
                projectIdentifier: project,
                paths: stringArrayArg(args, "paths"),
                into: stringArg(args, "into"),
                context: files
            ))
        case "files.delete":
            try await unwrap(MuxyAPI.Files.delete(
                projectIdentifier: project,
                paths: stringArrayArg(args, "paths"),
                context: files
            ))
            return NSNull()
        default:
            throw APIError.invalidArguments("unknown verb \(verb)")
        }
    }

    private static func handleGit(verb: String, args: [String: Any], context: Context) async throws -> Any {
        guard let projectStore = context.projectStore,
              let worktreeStore = context.worktreeStore,
              let projectGroupStore = context.projectGroupStore
        else { throw APIError.worktreeStoreUnavailable }
        let git = MuxyAPI.Git.Context(
            extensionID: context.extensionID,
            appState: context.appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
        let project = args["project"] as? String
        let fresh = args["fresh"] as? Bool ?? false

        switch verb {
        case "git.status":
            return try await GitDTO.status(unwrap(MuxyAPI.Git.status(
                projectIdentifier: project,
                local: args["local"] as? Bool ?? false,
                fresh: fresh,
                context: git
            )))
        case "git.diff":
            let rawMode = args["raw"] as? Bool ?? false
            let diffRequest = MuxyAPI.Git.DiffRequest(
                projectIdentifier: project,
                filePath: (args["filePath"] as? String).flatMap { $0.isEmpty ? nil : $0 },
                staged: args["staged"] as? Bool,
                lineLimit: intArg(args, "lineLimit"),
                fresh: fresh
            )
            if rawMode {
                return try await GitDTO.rawDiff(unwrap(MuxyAPI.Git.rawDiff(diffRequest, context: git)))
            }
            return try await GitDTO.diff(unwrap(MuxyAPI.Git.diff(diffRequest, context: git)))
        case "git.repoInfo":
            return try await GitDTO.repoInfo(unwrap(MuxyAPI.Git.repoInfo(projectIdentifier: project, context: git)))
        case "git.log":
            return try await unwrap(MuxyAPI.Git.log(
                projectIdentifier: project,
                maxCount: intArg(args, "maxCount") ?? 100,
                skip: intArg(args, "skip") ?? 0,
                fresh: fresh,
                context: git
            )).map(GitDTO.commit)
        case "git.branches":
            return try await unwrap(MuxyAPI.Git.branches(projectIdentifier: project, context: git))
        case "git.currentBranch":
            return try await unwrap(MuxyAPI.Git.currentBranch(projectIdentifier: project, context: git))
        case "git.aheadBehind":
            return try await GitDTO.aheadBehind(unwrap(MuxyAPI.Git.aheadBehind(projectIdentifier: project, fresh: fresh, context: git)))
        case "git.pr.info":
            let info = try await unwrap(MuxyAPI.Git.pullRequestInfo(projectIdentifier: project, fresh: fresh, context: git))
            return info.map(GitDTO.prInfo) ?? NSNull()
        case "git.pr.number":
            let number = try await unwrap(MuxyAPI.Git.pullRequestNumber(projectIdentifier: project, fresh: fresh, context: git))
            return number ?? NSNull()
        case "git.pr.diff":
            return try await GitDTO.rawDiff(unwrap(MuxyAPI.Git.pullRequestDiff(
                projectIdentifier: project,
                number: intArgRequired(args, "number"),
                lineLimit: intArg(args, "lineLimit"),
                fresh: fresh,
                context: git
            )))
        case "git.pr.list":
            return try await unwrap(MuxyAPI.Git.pullRequestList(
                projectIdentifier: project,
                filter: prListFilter(args["filter"] as? String),
                limit: intArg(args, "limit") ?? 100,
                includeChecks: boolArg(args, "checks") ?? true,
                context: git
            )).map(GitDTO.prListItem)
        case "git.worktrees":
            return try await unwrap(MuxyAPI.Git.worktrees(projectIdentifier: project, context: git))
                .map(GitDTO.worktree)
        case "git.stage":
            try await unwrap(MuxyAPI.Git.stage(projectIdentifier: project, paths: stringArrayArg(args, "paths"), context: git))
            return NSNull()
        case "git.unstage":
            try await unwrap(MuxyAPI.Git.unstage(projectIdentifier: project, paths: stringArrayArg(args, "paths"), context: git))
            return NSNull()
        case "git.discard":
            try await unwrap(MuxyAPI.Git.discard(
                projectIdentifier: project,
                paths: stringArrayArg(args, "paths"),
                untrackedPaths: stringArrayArg(args, "untrackedPaths"),
                context: git
            ))
            return NSNull()
        case "git.commit":
            let hash = try await unwrap(MuxyAPI.Git.commit(
                projectIdentifier: project,
                message: stringArg(args, "message"),
                stageAll: args["stageAll"] as? Bool ?? false,
                context: git
            ))
            return ["hash": hash]
        case "git.push":
            try await unwrap(MuxyAPI.Git.push(
                projectIdentifier: project,
                setUpstream: args["setUpstream"] as? Bool ?? false,
                context: git
            ))
            return NSNull()
        case "git.init":
            try await unwrap(MuxyAPI.Git.initRepository(projectIdentifier: project, context: git))
            return NSNull()
        case "git.branch.delete":
            try await unwrap(MuxyAPI.Git.deleteLocalBranch(
                projectIdentifier: project,
                name: stringArg(args, "name"),
                force: args["force"] as? Bool ?? false,
                context: git
            ))
            return NSNull()
        case "git.pull":
            try await unwrap(MuxyAPI.Git.pull(projectIdentifier: project, context: git))
            return NSNull()
        case "git.branch.create":
            try await unwrap(MuxyAPI.Git.createBranch(projectIdentifier: project, name: stringArg(args, "name"), context: git))
            return NSNull()
        case "git.branch.switch":
            try await unwrap(MuxyAPI.Git.switchBranch(projectIdentifier: project, branch: stringArg(args, "branch"), context: git))
            return NSNull()
        case "git.pr.create":
            return try await GitDTO.prInfo(unwrap(MuxyAPI.Git.createPullRequest(
                MuxyAPI.Git.CreatePRRequest(
                    projectIdentifier: project,
                    title: stringArg(args, "title"),
                    body: args["body"] as? String ?? "",
                    baseBranch: args["baseBranch"] as? String,
                    draft: args["draft"] as? Bool ?? false
                ),
                context: git
            )))
        case "git.pr.merge":
            try await unwrap(MuxyAPI.Git.mergePullRequest(
                projectIdentifier: project,
                number: intArgRequired(args, "number"),
                method: prMergeMethod(args["method"] as? String),
                deleteBranch: args["deleteBranch"] as? Bool ?? true,
                context: git
            ))
            return NSNull()
        case "git.pr.close":
            try await unwrap(MuxyAPI.Git.closePullRequest(
                projectIdentifier: project,
                number: intArgRequired(args, "number"),
                context: git
            ))
            return NSNull()
        case "git.worktree.add":
            try await unwrap(MuxyAPI.Git.addWorktree(
                MuxyAPI.Git.AddWorktreeRequest(
                    projectIdentifier: project,
                    path: stringArg(args, "path"),
                    branch: stringArg(args, "branch"),
                    createBranch: args["createBranch"] as? Bool ?? false,
                    baseBranch: args["baseBranch"] as? String
                ),
                context: git
            ))
            return NSNull()
        case "git.worktree.remove":
            try await unwrap(MuxyAPI.Git.removeWorktree(
                projectIdentifier: project,
                path: stringArg(args, "path"),
                force: args["force"] as? Bool ?? false,
                context: git
            ))
            return NSNull()
        case "git.worktree.switch":
            guard let projectStore = context.projectStore,
                  let worktreeStore = context.worktreeStore
            else { throw APIError.worktreeStoreUnavailable }
            try unwrap(MuxyAPI.Worktrees.switchTo(
                identifier: stringArg(args, "identifier"),
                projectIdentifier: project,
                appState: context.appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            ))
            return NSNull()
        case "git.remoteBranches":
            return try await unwrap(MuxyAPI.Git.remoteBranches(projectIdentifier: project, context: git))
        case "git.branch.deleteRemote":
            try await unwrap(MuxyAPI.Git.deleteRemoteBranch(
                projectIdentifier: project,
                branch: stringArg(args, "branch"),
                context: git
            ))
            return NSNull()
        case "git.checkout":
            try await unwrap(MuxyAPI.Git.checkout(projectIdentifier: project, hash: stringArg(args, "hash"), context: git))
            return NSNull()
        case "git.cherryPick":
            try await unwrap(MuxyAPI.Git.cherryPick(projectIdentifier: project, hash: stringArg(args, "hash"), context: git))
            return NSNull()
        case "git.revert":
            try await unwrap(MuxyAPI.Git.revert(projectIdentifier: project, hash: stringArg(args, "hash"), context: git))
            return NSNull()
        case "git.tag.create":
            try await unwrap(MuxyAPI.Git.createTag(
                projectIdentifier: project,
                name: stringArg(args, "name"),
                hash: stringArg(args, "hash"),
                context: git
            ))
            return NSNull()
        case "git.pr.checkout":
            try await unwrap(MuxyAPI.Git.checkoutPullRequest(
                projectIdentifier: project,
                number: intArgRequired(args, "number"),
                context: git
            ))
            return NSNull()
        case "git.pr.checkoutWorktree":
            let branch = try await unwrap(MuxyAPI.Git.checkoutPullRequestWorktree(
                projectIdentifier: project,
                path: stringArg(args, "path"),
                number: intArgRequired(args, "number"),
                context: git
            ))
            return ["branch": branch]
        default:
            throw APIError.invalidArguments("unknown verb \(verb)")
        }
    }

    private static func handleExec(args: [String: Any], context: Context) async throws -> Any {
        let request = try ExtensionBridgeShared.decodeExecRequest(args)
        let defaultCwd = ExtensionBridgeShared.activeWorktreePath(
            appState: context.appState,
            worktreeStore: context.worktreeStore
        )
        let result = try await ExtensionCommandExecutor.exec(
            request: request,
            extensionID: context.extensionID,
            defaultCwd: defaultCwd
        )
        return ExtensionBridgeShared.encodeExecResult(result)
    }

    private static func handleHTTPFetch(args: [String: Any], context: Context) async throws -> Any {
        let request = try ExtensionBridgeShared.decodeHTTPRequest(args)
        let result = try await ExtensionHTTPClient.fetch(request: request, extensionID: context.extensionID)
        return ExtensionBridgeShared.encodeHTTPResult(result)
    }

    private static func handleNotify(args: [String: Any], context: Context) async throws -> Any {
        let title = (args["title"] as? String) ?? ""
        let body = (args["body"] as? String) ?? ""
        guard !title.isEmpty || !body.isEmpty else {
            throw APIError.invalidArguments("notification requires title or body")
        }
        let source = MuxyNotification.Source.aiProvider(context.extensionID)
        if let paneIDString = args["paneID"] as? String, let paneID = UUID(uuidString: paneIDString) {
            NotificationStore.shared.add(
                paneID: paneID,
                source: source,
                title: title,
                body: body,
                appState: context.appState
            )
            return NSNull()
        }
        guard let projectID = context.appState.activeProjectID,
              let key = context.appState.activeWorktreeKey(for: projectID),
              let root = context.appState.workspaceRoots[key]
        else { throw APIError.noActiveProject }
        for area in root.allAreas() {
            for tab in area.tabs where tab.content.pane != nil {
                let navigationContext = NavigationContext(
                    projectID: key.projectID,
                    worktreeID: key.worktreeID,
                    worktreePath: area.projectPath,
                    areaID: area.id,
                    tabID: tab.id
                )
                NotificationStore.shared.addWithContext(
                    context: navigationContext,
                    source: source,
                    title: title,
                    body: body,
                    appState: context.appState
                )
                return NSNull()
            }
        }
        throw APIError.noFocusedArea
    }

    private static func stringArg(_ args: [String: Any], _ key: String) throws -> String {
        if let value = args[key] as? String { return value }
        throw APIError.invalidArguments("missing argument '\(key)'")
    }

    private static func doubleArg(_ args: [String: Any], _ key: String) throws -> Double {
        if let value = args[key] as? Double { return value }
        if let value = args[key] as? Int { return Double(value) }
        if let value = args[key] as? NSNumber { return value.doubleValue }
        throw APIError.invalidArguments("missing argument '\(key)'")
    }

    private static func unwrap<T>(_ result: Result<T, APIError>) throws -> T {
        switch result {
        case let .success(value): return value
        case let .failure(error): throw error
        }
    }

    private static func panelData(_ args: [String: Any]) -> ExtensionJSON? {
        guard let raw = args["data"] else { return nil }
        guard let data = try? JSONSerialization.data(withJSONObject: raw) else { return nil }
        return try? JSONDecoder().decode(ExtensionJSON.self, from: data)
    }

    private static func decodeOpenTabRequest(_ args: [String: Any]) throws -> OpenTabRequest {
        let data = try JSONSerialization.data(withJSONObject: args)
        do {
            return try JSONDecoder().decode(OpenTabRequest.self, from: data)
        } catch {
            throw APIError.invalidArguments("invalid open tab request: \(error.localizedDescription)")
        }
    }

    private static func modalItems(_ args: [String: Any]) -> [ExtensionModalService.Item] {
        ExtensionModalService.parseItems(args["items"] as? [Any] ?? [])
    }

    private static func modalItemDict(_ item: ExtensionModalService.Item) -> [String: Any] {
        [
            "id": item.id,
            "title": item.title,
            "subtitle": item.subtitle ?? NSNull(),
        ]
    }

    private static func tabDict(_ tab: TabInfo) -> [String: Any] {
        [
            "index": tab.index,
            "id": tab.id.uuidString,
            "kind": tab.kind.rawValue,
            "title": tab.title,
            "isActive": tab.isActive,
        ]
    }

    private static func paneDict(_ pane: PaneInfo) -> [String: Any] {
        [
            "id": pane.id.uuidString,
            "title": pane.title,
            "workingDirectory": pane.workingDirectory,
            "isFocused": pane.isFocused,
        ]
    }

    private static func projectDict(_ project: ProjectInfo) -> [String: Any] {
        [
            "id": project.id.uuidString,
            "name": project.name,
            "path": project.path,
            "isActive": project.isActive,
        ]
    }

    private static func worktreeDict(_ worktree: WorktreeInfo) -> [String: Any] {
        [
            "id": worktree.id.uuidString,
            "name": worktree.name,
            "path": worktree.path,
            "branch": worktree.branch ?? NSNull(),
            "isActive": worktree.isActive,
        ]
    }

    private static func intArg(_ args: [String: Any], _ key: String) -> Int? {
        if let value = args[key] as? Int { return value }
        if let value = args[key] as? NSNumber { return value.intValue }
        return nil
    }

    private static func boolArg(_ args: [String: Any], _ key: String) -> Bool? {
        if let value = args[key] as? Bool { return value }
        if let value = args[key] as? NSNumber { return value.boolValue }
        return nil
    }

    private static func intArgRequired(_ args: [String: Any], _ key: String) throws -> Int {
        guard let value = intArg(args, key) else {
            throw APIError.invalidArguments("missing argument '\(key)'")
        }
        return value
    }

    private static func stringArrayArg(_ args: [String: Any], _ key: String) -> [String] {
        (args[key] as? [Any])?.compactMap { $0 as? String } ?? []
    }

    private static func prListFilter(_ raw: String?) -> GitRepositoryService.PRListFilter {
        raw.flatMap(GitRepositoryService.PRListFilter.init(rawValue:)) ?? .open
    }

    private static func prMergeMethod(_ raw: String?) -> GitRepositoryService.PRMergeMethod {
        raw.flatMap(GitRepositoryService.PRMergeMethod.init(rawValue:)) ?? .merge
    }
}
