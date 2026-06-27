import Foundation

@MainActor
enum SocketCommandHandler {
    static func handleRequest(
        _ message: String,
        appState: AppState,
        projectStore: ProjectStore? = nil,
        worktreeStore: WorktreeStore? = nil,
        projectGroupStore: ProjectGroupStore? = nil,
        browserProfileStore: BrowserProfileStore? = nil,
        clientContext: NotificationSocketServer.ClientContext = .init(extensionID: nil)
    ) async -> String {
        let parts = message.components(separatedBy: "|")
        guard let cmd = parts.first else {
            return "error:empty command"
        }

        if let extensionID = clientContext.extensionID {
            for required in requiredPermissions(command: cmd, parts: parts)
                where !ExtensionStore.shared.extensionHasPermission(id: extensionID, permission: required)
            {
                return "error:permission denied (\(required.rawValue))"
            }
        }

        switch cmd {
        case "split-right":
            let request = parseSplitRequest(parts: parts)
            return serialize(MuxyAPI.Panes.split(
                direction: .horizontal,
                command: request.command,
                fromPane: request.fromPane,
                appState: appState
            )) { $0.uuidString }
        case "split-down":
            let request = parseSplitRequest(parts: parts)
            return serialize(MuxyAPI.Panes.split(
                direction: .vertical,
                command: request.command,
                fromPane: request.fromPane,
                appState: appState
            )) { $0.uuidString }
        case "send":
            guard parts.count >= 3 else { return "error:usage send|paneID|text" }
            return await serialize(
                MuxyAPI.Panes.send(
                    paneIDString: parts[1],
                    text: parts.dropFirst(2).joined(separator: "|"),
                    appState: appState,
                    extensionID: clientContext.extensionID
                ),
                ok: "ok"
            )
        case "send-keys":
            guard parts.count >= 3 else { return "error:usage send-keys|paneID|key" }
            return await serialize(
                MuxyAPI.Panes.sendKeys(
                    paneIDString: parts[1],
                    key: parts[2],
                    appState: appState,
                    extensionID: clientContext.extensionID
                ),
                ok: "ok"
            )
        case "read-screen":
            guard parts.count >= 2 else { return "error:usage read-screen|paneID[|lines]" }
            let lines = parts.count >= 3 ? Int(parts[2]) ?? 50 : 50
            return await serialize(MuxyAPI.Panes.readScreen(
                paneIDString: parts[1],
                lines: lines,
                appState: appState,
                extensionID: clientContext.extensionID
            )) { $0 }
        case "close-pane":
            guard parts.count >= 2 else { return "error:usage close-pane|paneID" }
            return serialize(MuxyAPI.Panes.close(paneIDString: parts[1], appState: appState), ok: "ok")
        case "rename-pane":
            guard parts.count >= 3 else { return "error:usage rename-pane|paneID|title" }
            return serialize(
                MuxyAPI.Panes.rename(
                    paneIDString: parts[1],
                    title: parts.dropFirst(2).joined(separator: "|"),
                    appState: appState
                ),
                ok: "ok"
            )
        case "list-panes":
            let panes = MuxyAPI.Panes.list(appState: appState)
            return panes.map { pane in
                "\(pane.id.uuidString)\t\(pane.title)\t\(pane.workingDirectory)\t\(pane.isFocused)"
            }.joined(separator: "\n")
        case "list-projects":
            guard let projectStore else { return "error:project store unavailable" }
            let projects = MuxyAPI.Projects.list(appState: appState, projectStore: projectStore)
            return projects.map { project in
                "\(project.id.uuidString)\t\(project.name)\t\(project.path)\t\(project.isActive)"
            }.joined(separator: "\n")
        case "switch-project":
            guard parts.count >= 2 else { return "error:usage switch-project|name-or-id-or-path" }
            guard let projectStore, let worktreeStore else { return "error:project store unavailable" }
            return serialize(
                MuxyAPI.Projects.switchTo(
                    identifier: parts.dropFirst().joined(separator: "|"),
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore
                ),
                ok: "ok"
            )
        case "list-worktrees":
            guard let projectStore, let worktreeStore else { return "error:worktree store unavailable" }
            let identifier = parts.count >= 2 ? parts.dropFirst().joined(separator: "|") : nil
            return serialize(MuxyAPI.Worktrees.list(
                projectIdentifier: identifier,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            )) { worktrees in
                worktrees.map { worktree in
                    "\(worktree.id.uuidString)\t\(worktree.name)\t\(worktree.path)\t\(worktree.branch ?? "")\t\(worktree.isActive)"
                }.joined(separator: "\n")
            }
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
            return serialize(
                MuxyAPI.Worktrees.switchTo(
                    identifier: parts[1],
                    projectIdentifier: projectIdentifier,
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore
                ),
                ok: "ok"
            )
        case "refresh-worktrees":
            guard let projectStore, let worktreeStore else { return "error:worktree store unavailable" }
            let identifier = parts.count >= 2 ? parts.dropFirst().joined(separator: "|") : nil
            return await serialize(MuxyAPI.Worktrees.refresh(
                projectIdentifier: identifier,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            )) { result in
                "ok\t\(result.count)"
            }
        case "list-tabs":
            return serialize(MuxyAPI.Tabs.list(appState: appState)) { tabs in
                tabs.map { tab in
                    "\(tab.index)\t\(tab.id.uuidString)\t\(tab.kind.rawValue)\t\(tab.title)\t\(tab.isActive)"
                }.joined(separator: "\n")
            }
        case "switch-tab":
            guard parts.count >= 2 else { return "error:usage switch-tab|index-or-id-or-title" }
            return serialize(
                MuxyAPI.Tabs.switchTo(
                    identifier: parts.dropFirst().joined(separator: "|"),
                    appState: appState
                ),
                ok: "ok"
            )
        case "new-tab":
            return serialize(MuxyAPI.Tabs.new(appState: appState)) { newTabID in
                newTabID?.uuidString ?? "ok"
            }
        case "next-tab":
            return serialize(MuxyAPI.Tabs.next(appState: appState), ok: "ok")
        case "previous-tab":
            return serialize(MuxyAPI.Tabs.previous(appState: appState), ok: "ok")
        case "open-tab":
            guard parts.count >= 2 else { return "error:usage open-tab|<json>" }
            let payload = parts.dropFirst().joined(separator: "|")
            guard let data = payload.data(using: .utf8) else {
                return "error:invalid open-tab payload"
            }
            do {
                let request = try JSONDecoder().decode(OpenTabRequest.self, from: data)
                return await serialize(MuxyAPI.Tabs.open(
                    request,
                    appState: appState,
                    callingExtensionID: clientContext.extensionID
                )) { tabID in
                    tabID.uuidString
                }
            } catch {
                return "error:invalid open-tab payload: \(error.localizedDescription)"
            }
        case "tabs.open":
            guard parts.count >= 2 else { return "error:usage tabs.open|<base64-json>" }
            guard let extensionID = clientContext.extensionID else { return "error:identify required" }
            return await handleAPIVerb(
                verb: cmd,
                base64Payload: parts[1],
                context: MuxyAPIDispatcher.Context(
                    extensionID: extensionID,
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore,
                    projectGroupStore: projectGroupStore,
                    browserProfileStore: browserProfileStore
                )
            )
        case "browser.open":
            let split = parts.contains("--split")
            let urlParts = trimTrailingEmptyFields(parts.dropFirst().filter { $0 != "--split" })
            let url = urlParts.isEmpty ? nil : urlParts.joined(separator: "|")
            return serialize(MuxyAPI.Browser.open(url: url, split: split, appState: appState)) { tabID in
                tabID.uuidString
            }
        case "browser.navigate":
            guard parts.count >= 3 else { return "error:usage browser.navigate|<tab-id>|<url>" }
            return serialize(
                MuxyAPI.Browser.navigate(
                    tabIDString: parts[1],
                    url: parts.dropFirst(2).joined(separator: "|"),
                    appState: appState
                ),
                ok: "ok"
            )
        case "browser.list":
            let browserTabs = MuxyAPI.Browser.list(appState: appState, profileStore: browserProfileStore)
            return browserTabs.map { tab in
                "\(tab.id.uuidString)\t\(tab.title)\t\(tab.url ?? "")\t\(tab.profile)\t\(tab.isActive)"
            }.joined(separator: "\n")
        case "browser.read":
            guard parts.count >= 2 else { return "error:usage browser.read|<tab-id>" }
            return await serialize(MuxyAPI.Browser.read(tabIDString: parts[1], appState: appState)) { content in
                "\(content.title)\n\(content.url ?? "")\n\(content.text)"
            }
        case "browser.close":
            guard parts.count >= 2 else { return "error:usage browser.close|<tab-id>" }
            return serialize(MuxyAPI.Browser.close(tabIDString: parts[1], appState: appState), ok: "ok")
        case "extension.settings.get":
            guard parts.count >= 2 else { return "error:usage extension.settings.get|key" }
            return handleSettingsGet(key: parts[1], extensionID: clientContext.extensionID)
        case "extension.settings.set":
            guard parts.count >= 3 else { return "error:usage extension.settings.set|key|<json-value>" }
            let value = parts.dropFirst(2).joined(separator: "|")
            return handleSettingsSet(key: parts[1], rawValue: value, extensionID: clientContext.extensionID)
        case "extension.statusbar.set":
            guard parts.count >= 2 else { return "error:usage extension.statusbar.set|itemID[|text]" }
            let rawText = parts.count >= 3 ? parts.dropFirst(2).joined(separator: "|") : nil
            let text = (rawText?.isEmpty == true) ? nil : rawText
            return handleStatusBarSet(itemID: parts[1], text: text, extensionID: clientContext.extensionID)
        case "panel.open",
             "panel.toggle":
            guard parts.count >= 2 else { return "error:usage \(cmd)|panelID[|<json-data>]" }
            return handlePanelOpen(
                panelID: parts[1],
                rawData: parts.count >= 3 ? parts.dropFirst(2).joined(separator: "|") : nil,
                toggle: cmd == "panel.toggle",
                extensionID: clientContext.extensionID
            )
        case "panel.close":
            guard parts.count >= 2 else { return "error:usage panel.close|panelID" }
            return handlePanelClose(panelID: parts[1], extensionID: clientContext.extensionID)
        case "popover.close":
            return handlePopoverClose(extensionID: clientContext.extensionID)
        case "popover.resize":
            guard parts.count >= 3 else { return "error:usage popover.resize|width|height" }
            return handlePopoverResize(
                width: parts[1],
                height: parts[2],
                extensionID: clientContext.extensionID
            )
        case "exec":
            guard parts.count >= 2 else { return "error:usage exec|<base64-json>" }
            return await handleExec(
                base64Payload: parts[1],
                appState: appState,
                worktreeStore: worktreeStore,
                extensionID: clientContext.extensionID
            )
        case "dialog.confirm":
            guard parts.count >= 2 else { return "error:usage dialog.confirm|<base64-json>" }
            return await handleDialogConfirm(base64Payload: parts[1], extensionID: clientContext.extensionID)
        case "dialog.alert":
            guard parts.count >= 2 else { return "error:usage dialog.alert|<base64-json>" }
            return await handleDialogAlert(base64Payload: parts[1], extensionID: clientContext.extensionID)
        case "dialog.prompt",
             "dialog.pickFolder",
             "storage.get",
             "storage.set",
             "storage.delete",
             "storage.keys",
             "shortcuts.register",
             "shortcuts.unregister",
             "shortcuts.list":
            guard parts.count >= 2 else { return "error:usage \(cmd)|<base64-json>" }
            guard let extensionID = clientContext.extensionID else { return "error:identify required" }
            return await handleAPIVerb(
                verb: cmd,
                base64Payload: parts[1],
                context: MuxyAPIDispatcher.Context(
                    extensionID: extensionID,
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore,
                    projectGroupStore: projectGroupStore,
                    browserProfileStore: browserProfileStore
                )
            )
        case "modal.open",
             "modal.feed",
             "modal.finish",
             "modal.await":
            guard parts.count >= 2 else { return "error:usage \(cmd)|<base64-json>" }
            guard let extensionID = clientContext.extensionID else { return "error:identify required" }
            return await handleModalVerb(
                verb: cmd,
                base64Payload: parts[1],
                context: MuxyAPIDispatcher.Context(
                    extensionID: extensionID,
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore,
                    projectGroupStore: projectGroupStore,
                    browserProfileStore: browserProfileStore
                )
            )
        case "topbar.set",
             "statusbar.set":
            guard parts.count >= 2 else { return "error:usage \(cmd)|<base64-json>" }
            return handleBarItemSet(verb: cmd, base64Payload: parts[1], extensionID: clientContext.extensionID)
        case let verb where verb.hasPrefix("git."):
            guard parts.count >= 2 else { return "error:usage \(cmd)|<base64-json>" }
            guard let extensionID = clientContext.extensionID else { return "error:identify required" }
            return await handleGit(
                verb: verb,
                base64Payload: parts[1],
                context: MuxyAPIDispatcher.Context(
                    extensionID: extensionID,
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore,
                    projectGroupStore: projectGroupStore,
                    browserProfileStore: browserProfileStore
                )
            )
        case let verb where verb.hasPrefix("browser."):
            guard parts.count >= 2 else { return "error:usage \(cmd)|<base64-json>" }
            return await handleAPIVerb(
                verb: verb,
                base64Payload: parts[1],
                context: MuxyAPIDispatcher.Context(
                    extensionID: clientContext.extensionID ?? "",
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore,
                    projectGroupStore: projectGroupStore,
                    browserProfileStore: browserProfileStore
                ),
                enforcePermissions: false
            )
        default:
            return "error:unknown command \(cmd)"
        }
    }

    private static func handleGit(
        verb: String,
        base64Payload: String,
        context: MuxyAPIDispatcher.Context
    ) async -> String {
        await handleAPIVerb(verb: verb, base64Payload: base64Payload, context: context)
    }

    private static func handleAPIVerb(
        verb: String,
        base64Payload: String,
        context: MuxyAPIDispatcher.Context,
        enforcePermissions: Bool = true
    ) async -> String {
        guard let data = Data(base64Encoded: base64Payload),
              let args = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return "error:invalid \(verb) payload" }
        do {
            let result = try await MuxyAPIDispatcher.dispatch(
                verb: verb,
                args: args,
                context: context,
                enforcePermissions: enforcePermissions
            )
            guard let encoded = try? JSONSerialization.data(withJSONObject: result, options: [.fragmentsAllowed]) else {
                return "error:\(verb) result encoding failed"
            }
            return encoded.base64EncodedString()
        } catch let error as APIError {
            return "error:\(error.message)"
        } catch {
            return "error:\(error.localizedDescription)"
        }
    }

    private static func handleExec(
        base64Payload: String,
        appState: AppState,
        worktreeStore: WorktreeStore?,
        extensionID: String?
    ) async -> String {
        guard let extensionID else { return "error:identify required" }
        guard let data = Data(base64Encoded: base64Payload),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return "error:invalid exec payload" }

        let request: ExecRequest
        do {
            request = try ExtensionBridgeShared.decodeExecRequest(json)
        } catch {
            return "error:\(error.localizedDescription)"
        }

        let defaultCwd = ExtensionBridgeShared.activeWorktreePath(
            appState: appState,
            worktreeStore: worktreeStore
        )
        do {
            let result = try await ExtensionCommandExecutor.exec(
                request: request,
                extensionID: extensionID,
                defaultCwd: defaultCwd
            )
            let resultJSON = ExtensionBridgeShared.encodeExecResult(result)
            guard let encoded = try? JSONSerialization.data(withJSONObject: resultJSON) else {
                return "error:exec result encoding failed"
            }
            return encoded.base64EncodedString()
        } catch {
            return "error:\(error.localizedDescription)"
        }
    }

    private static func handleDialogConfirm(base64Payload: String, extensionID: String?) async -> String {
        guard let extensionID else { return "error:identify required" }
        guard let args = decodeJSONObject(base64Payload) else {
            return "error:invalid dialog payload"
        }
        do {
            let request = try ExtensionDialogService.makeConfirmRequest(extensionID: extensionID, args: args)
            let choice = try await ExtensionDialogService.confirm(request)
            return encodeJSONFragment(choice ?? NSNull())
        } catch {
            return "error:\((error as? APIError)?.message ?? error.localizedDescription)"
        }
    }

    private static func handleDialogAlert(base64Payload: String, extensionID: String?) async -> String {
        guard let extensionID else { return "error:identify required" }
        guard let args = decodeJSONObject(base64Payload) else {
            return "error:invalid alert payload"
        }
        do {
            let request = try ExtensionDialogService.makeAlertRequest(extensionID: extensionID, args: args)
            try await ExtensionDialogService.alert(request)
            return encodeJSONFragment(NSNull())
        } catch {
            return "error:\((error as? APIError)?.message ?? error.localizedDescription)"
        }
    }

    private static func handleModalVerb(
        verb: String,
        base64Payload: String,
        context: MuxyAPIDispatcher.Context
    ) async -> String {
        guard let args = decodeJSONObject(base64Payload) else {
            return "error:invalid \(verb) payload"
        }
        do {
            let result = try await MuxyAPIDispatcher.dispatch(verb: verb, args: args, context: context)
            if verb == "modal.open", let dict = result as? [String: Any], let requestID = dict["requestID"] as? String {
                registerModalResultPush(requestID: requestID, extensionID: context.extensionID)
                registerModalQueryPush(requestID: requestID, extensionID: context.extensionID)
            }
            return encodeJSONFragment(result)
        } catch {
            return "error:\((error as? APIError)?.message ?? error.localizedDescription)"
        }
    }

    private static func registerModalResultPush(requestID: String, extensionID: String) {
        ExtensionModalService.shared.onResult(requestID: requestID) { item in
            let payload = ExtensionModalService.modalResultPayload(item)
            let data = (try? JSONSerialization.data(withJSONObject: payload, options: [.fragmentsAllowed])) ?? Data("null".utf8)
            NotificationSocketServer.shared.pushModalResult(
                extensionID: extensionID,
                requestID: requestID,
                payload: data
            )
        }
    }

    private static func registerModalQueryPush(requestID: String, extensionID: String) {
        ExtensionModalService.shared.onQueryRequest(requestID: requestID) { queryID, query, options in
            NotificationSocketServer.shared.pushModalQuery(
                extensionID: extensionID,
                requestID: requestID,
                queryID: queryID,
                query: query,
                options: options
            )
        }
    }

    private static func handleBarItemSet(verb: String, base64Payload: String, extensionID: String?) -> String {
        guard let extensionID else { return "error:identify required" }
        guard ExtensionStore.shared.extensionHasPermission(id: extensionID, permission: .panelsWrite) else {
            return "error:permission denied (panels:write)"
        }
        guard let args = decodeJSONObject(base64Payload), let itemID = args["id"] as? String else {
            return "error:invalid \(verb) payload"
        }
        let icon = ExtensionIcon.parse(args["icon"])
        let visible = args["visible"] as? Bool
        if verb == "topbar.set" {
            let updated = ExtensionStore.shared.setTopbarItem(
                extensionID: extensionID,
                itemID: itemID,
                icon: icon,
                visible: visible
            )
            return updated ? encodeJSONFragment(NSNull()) : "error:unknown topbar item '\(itemID)'"
        }
        let rawText = args["text"] as? String
        let updated = ExtensionStore.shared.setStatusBarItem(
            extensionID: extensionID,
            itemID: itemID,
            update: ExtensionStore.StatusBarUpdate(
                icon: icon,
                text: (rawText?.isEmpty == true) ? nil : rawText,
                clearText: args.keys.contains("text"),
                visible: visible
            )
        )
        return updated ? encodeJSONFragment(NSNull()) : "error:unknown status bar item '\(itemID)'"
    }

    private static func decodeJSONObject(_ base64Payload: String) -> [String: Any]? {
        guard let data = Data(base64Encoded: base64Payload),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }
        return json
    }

    private static func encodeJSONFragment(_ value: Any) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed]) else {
            return "error:result encoding failed"
        }
        return data.base64EncodedString()
    }

    private static func handlePanelOpen(
        panelID: String,
        rawData: String?,
        toggle: Bool,
        extensionID: String?
    ) -> String {
        guard let extensionID else { return "error:identify required" }
        let data = rawData
            .flatMap { $0.data(using: .utf8) }
            .flatMap { try? JSONDecoder().decode(ExtensionJSON.self, from: $0) }
        return serialize(
            MuxyAPI.Panels.open(extensionID: extensionID, panelID: panelID, data: data, toggle: toggle),
            ok: "ok"
        )
    }

    private static func handlePanelClose(panelID: String, extensionID: String?) -> String {
        guard let extensionID else { return "error:identify required" }
        return serialize(
            MuxyAPI.Panels.close(extensionID: extensionID, panelID: panelID),
            ok: "ok"
        )
    }

    private static func handlePopoverClose(extensionID: String?) -> String {
        guard let extensionID else { return "error:identify required" }
        return serialize(MuxyAPI.Popovers.close(extensionID: extensionID), ok: "ok")
    }

    private static func handlePopoverResize(width: String, height: String, extensionID: String?) -> String {
        guard let extensionID else { return "error:identify required" }
        guard let widthValue = Double(width), let heightValue = Double(height) else {
            return "error:usage popover.resize|width|height"
        }
        return serialize(
            MuxyAPI.Popovers.resize(extensionID: extensionID, width: widthValue, height: heightValue),
            ok: "ok"
        )
    }

    private static func handleSettingsGet(key: String, extensionID: String?) -> String {
        guard let extensionID else { return "error:identify required" }
        guard let muxyExtension = ExtensionStore.shared.loadedExtension(id: extensionID) else {
            return "error:unknown extension"
        }
        guard let entry = muxyExtension.manifest.setting(key: key) else {
            return "error:setting '\(key)' not declared in manifest"
        }
        guard let value = ExtensionSettingsStore.shared.effectiveValue(extensionID: extensionID, entry: entry) else {
            return "ok"
        }
        do {
            let data = try JSONEncoder().encode(value)
            let json = String(data: data, encoding: .utf8) ?? "null"
            return "ok\t\(json)"
        } catch {
            return "error:encode failed"
        }
    }

    private static let maxSettingValueBytes = 64 * 1024

    private static func handleSettingsSet(key: String, rawValue: String, extensionID: String?) -> String {
        guard let extensionID else { return "error:identify required" }
        guard let data = rawValue.data(using: .utf8) else {
            return "error:invalid value encoding"
        }
        guard data.count <= maxSettingValueBytes else {
            return "error:value exceeds \(maxSettingValueBytes)-byte limit"
        }
        guard let muxyExtension = ExtensionStore.shared.loadedExtension(id: extensionID) else {
            return "error:unknown extension"
        }
        guard muxyExtension.manifest.setting(key: key) != nil else {
            return "error:setting '\(key)' not declared in manifest"
        }
        do {
            let value = try JSONDecoder().decode(ExtensionJSON.self, from: data)
            ExtensionSettingsStore.shared.setValue(value, extensionID: extensionID, key: key)
            return "ok"
        } catch {
            return "error:invalid json value: \(error.localizedDescription)"
        }
    }

    private static func handleStatusBarSet(itemID: String, text: String?, extensionID: String?) -> String {
        guard let extensionID else { return "error:identify required" }
        let updated = ExtensionStore.shared.setStatusBarText(
            extensionID: extensionID,
            itemID: itemID,
            text: text
        )
        guard updated else { return "error:unknown status bar item '\(itemID)'" }
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
        let name = arguments[0]
        let branch = arguments[1]
        let projectIdentifier = arguments.count >= 3 ? arguments[2] : nil
        let requestedPath = arguments.count >= 4 ? arguments[3] : ""
        let createBranch = arguments.count >= 5 ? arguments[4] != "false" : true
        let baseBranch = arguments.count >= 6 ? arguments[5] : ""

        let result = await MuxyAPI.Worktrees.create(
            CreateWorktreeRequest(
                name: name,
                branch: branch,
                projectIdentifier: projectIdentifier,
                requestedPath: requestedPath,
                createBranch: createBranch,
                baseBranch: baseBranch
            ),
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore
        )

        switch result {
        case let .success(worktree):
            return "ok\t\(worktree.id.uuidString)\t\(worktree.name)\t\(worktree.path)\t\(worktree.branch ?? "")"
        case let .failure(error):
            return "error:\(error.message)"
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

    private static func trimTrailingEmptyFields(_ fields: [String]) -> [String] {
        var trimmed = fields
        while trimmed.last?.isEmpty == true {
            trimmed.removeLast()
        }
        return trimmed
    }

    static func requiredPermissions(command: String, parts: [String]) -> [ExtensionPermission] {
        if command.hasPrefix("browser."), parts.count >= 2 {
            let args = decodeJSONObject(parts[1]) ?? [:]
            return MuxyAPI.Permissions.required(for: command, args: args).map { [$0] } ?? []
        }
        var permissions = MuxyAPI.Permissions.required(for: command).map { [$0] } ?? []
        guard command == "split-right" || command == "split-down" else { return permissions }
        let splitRequest = parseSplitRequest(parts: parts)
        let trimmedCommand = splitRequest.command?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmedCommand?.isEmpty == false else { return permissions }
        permissions.append(.commandsExec)
        return permissions
    }

    private static func serialize<T>(
        _ result: Result<T, APIError>,
        format: (T) -> String
    ) -> String {
        switch result {
        case let .success(value): format(value)
        case let .failure(error): "error:\(error.message)"
        }
    }

    private static func serialize(_ result: Result<some Any, APIError>, ok: String) -> String {
        switch result {
        case .success: ok
        case let .failure(error): "error:\(error.message)"
        }
    }
}
