import Foundation
import JavaScriptCore
import os

private let logger = Logger(subsystem: "app.muxy", category: "ExtensionScriptRunner")

@MainActor
final class ExtensionScriptRunner {
    static let shared = ExtensionScriptRunner()

    enum RunError: Error, LocalizedError {
        case scriptUnreadable(URL)
        case evaluationFailed(String)

        var errorDescription: String? {
            switch self {
            case let .scriptUnreadable(url): "Could not read script at \(url.path)"
            case let .evaluationFailed(message): "Script error: \(message)"
            }
        }
    }

    private struct ContextHandle {
        let context: JSContext
        let queue: DispatchQueue
    }

    private var contexts: [String: ContextHandle] = [:]

    private init() {}

    func evict(extensionID: String) {
        contexts.removeValue(forKey: extensionID)
    }

    func runScript(
        extensionID: String,
        scriptURL: URL,
        appState: AppState,
        projectStore: ProjectStore?,
        worktreeStore: WorktreeStore?
    ) async throws {
        guard let source = try? String(contentsOf: scriptURL, encoding: .utf8) else {
            throw RunError.scriptUnreadable(scriptURL)
        }

        let handle = contextHandle(for: extensionID)
        let bridge = ScriptBridge(
            extensionID: extensionID,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore
        )
        bridge.install(into: handle.context)

        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            handle.queue.async {
                let capture = ExceptionCapture()
                handle.context.exceptionHandler = { _, exception in
                    capture.message = exception?.toString() ?? "unknown error"
                }
                _ = handle.context.evaluateScript(source, withSourceURL: scriptURL)
                handle.context.exceptionHandler = nil
                if let message = capture.message {
                    logger.error("Extension \(extensionID) script error: \(message)")
                    continuation.resume(throwing: RunError.evaluationFailed(message))
                } else {
                    continuation.resume()
                }
            }
        }
    }

    private final class ExceptionCapture {
        var message: String?
    }

    private func contextHandle(for extensionID: String) -> ContextHandle {
        if let existing = contexts[extensionID] {
            return existing
        }
        let queue = DispatchQueue(label: "app.muxy.extension.\(extensionID)")
        guard let context = JSContext() else {
            fatalError("Failed to create JSContext for extension \(extensionID)")
        }
        let handle = ContextHandle(context: context, queue: queue)
        contexts[extensionID] = handle
        return handle
    }
}

private final class ScriptBridge: @unchecked Sendable {
    private let extensionID: String
    private weak var appState: AppState?
    private weak var projectStore: ProjectStore?
    private weak var worktreeStore: WorktreeStore?

    @MainActor
    init(
        extensionID: String,
        appState: AppState,
        projectStore: ProjectStore?,
        worktreeStore: WorktreeStore?
    ) {
        self.extensionID = extensionID
        self.appState = appState
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
    }

    @MainActor
    func install(into context: JSContext) {
        let dispatcher: @convention(block) (String, JSValue?) -> Any = { [weak self] verb, args in
            guard let self else { return Self.errorObject("bridge released") }
            let dict = (args?.toDictionary() as? [String: Any]) ?? [:]
            return self.dispatch(verb: verb, args: dict)
        }
        context.setObject(dispatcher, forKeyedSubscript: "__muxyDispatch" as NSString)

        let extID = extensionID
        let consoleBridge: @convention(block) (String, String) -> Void = { level, message in
            ExtensionLogStore.shared.append(extensionID: extID, line: "[\(level)] \(message)")
        }
        context.setObject(consoleBridge, forKeyedSubscript: "__muxyConsole" as NSString)
        context.evaluateScript(Self.bridgeScript(extensionID: extensionID))
    }

    private func dispatch(verb: String, args: [String: Any]) -> Any {
        let bridge = self
        let argsBox = AnyBox(args)
        do {
            if let required = MuxyAPI.Permissions.required(for: verb) {
                let allowed: Bool = try syncAwait { @MainActor in
                    ExtensionStore.shared.extensionHasPermission(id: bridge.extensionID, permission: required)
                }
                if !allowed {
                    return Self.errorObject("permission denied (\(required.rawValue))")
                }
            }
            let encoded = try syncAwait { @MainActor in
                let raw = try await bridge.handle(verb: verb, args: argsBox.value)
                return try BridgeValue(from: raw)
            }
            return ["ok": true, "value": encoded.unwrap()]
        } catch let error as APIError {
            return Self.errorObject(error.message)
        } catch {
            return Self.errorObject(error.localizedDescription)
        }
    }

    private static func errorObject(_ message: String) -> [String: Any] {
        ["ok": false, "error": message]
    }

    @MainActor
    private func handle(verb: String, args: [String: Any]) async throws -> Any {
        guard let appState else { throw APIError.underlying("app state unavailable") }
        switch verb {
        case "toast":
            return try await handleToast(args: args, appState: appState)
        case "exec":
            return try await handleExec(args: args, appState: appState)
        case "tabs.list":
            return try unwrap(MuxyAPI.Tabs.list(appState: appState)).map(tabDict)
        case "tabs.switch":
            try unwrap(MuxyAPI.Tabs.switchTo(identifier: stringArg(args, "identifier"), appState: appState))
            return NSNull()
        case "tabs.new":
            return try unwrap(MuxyAPI.Tabs.new(appState: appState))?.uuidString ?? NSNull()
        case "tabs.next":
            try unwrap(MuxyAPI.Tabs.next(appState: appState))
            return NSNull()
        case "tabs.previous":
            try unwrap(MuxyAPI.Tabs.previous(appState: appState))
            return NSNull()
        case "tabs.open":
            try unwrap(MuxyAPI.Tabs.open(decodeOpenTabRequest(args), appState: appState))
            return NSNull()
        case "panes.list":
            return MuxyAPI.Panes.list(appState: appState).map(paneDict)
        case "panes.send":
            try await unwrap(MuxyAPI.Panes.send(
                paneIDString: stringArg(args, "paneID"),
                text: stringArg(args, "text"),
                appState: appState
            ))
            return NSNull()
        case "panes.sendKeys":
            try await unwrap(MuxyAPI.Panes.sendKeys(
                paneIDString: stringArg(args, "paneID"),
                key: stringArg(args, "key"),
                appState: appState
            ))
            return NSNull()
        case "panes.readScreen":
            let lines = (args["lines"] as? Int) ?? 50
            return try await unwrap(MuxyAPI.Panes.readScreen(
                paneIDString: stringArg(args, "paneID"),
                lines: lines,
                appState: appState
            ))
        case "panes.close":
            try unwrap(MuxyAPI.Panes.close(paneIDString: stringArg(args, "paneID"), appState: appState))
            return NSNull()
        case "panes.rename":
            try unwrap(MuxyAPI.Panes.rename(
                paneIDString: stringArg(args, "paneID"),
                title: stringArg(args, "title"),
                appState: appState
            ))
            return NSNull()
        case "projects.list":
            guard let projectStore else { throw APIError.projectStoreUnavailable }
            return MuxyAPI.Projects.list(appState: appState, projectStore: projectStore).map(projectDict)
        case "projects.switch":
            guard let projectStore, let worktreeStore else { throw APIError.projectStoreUnavailable }
            try unwrap(MuxyAPI.Projects.switchTo(
                identifier: stringArg(args, "identifier"),
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            ))
            return NSNull()
        case "worktrees.list":
            guard let projectStore, let worktreeStore else { throw APIError.worktreeStoreUnavailable }
            return try unwrap(MuxyAPI.Worktrees.list(
                projectIdentifier: args["project"] as? String,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            )).map(worktreeDict)
        case "worktrees.switch":
            guard let projectStore, let worktreeStore else { throw APIError.worktreeStoreUnavailable }
            try unwrap(MuxyAPI.Worktrees.switchTo(
                identifier: stringArg(args, "identifier"),
                projectIdentifier: args["project"] as? String,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            ))
            return NSNull()
        case "worktrees.refresh":
            guard let projectStore, let worktreeStore else { throw APIError.worktreeStoreUnavailable }
            let result = try await unwrap(MuxyAPI.Worktrees.refresh(
                projectIdentifier: args["project"] as? String,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            ))
            return ["count": result.count]
        default:
            throw APIError.invalidArguments("unknown verb \(verb)")
        }
    }

    @MainActor
    private func handleExec(args: [String: Any], appState: AppState) async throws -> Any {
        let request = try ExtensionBridgeShared.decodeExecRequest(args)
        let defaultCwd = ExtensionBridgeShared.activeWorktreePath(
            appState: appState,
            worktreeStore: worktreeStore
        )
        let result = try await ExtensionCommandExecutor.exec(
            request: request,
            extensionID: extensionID,
            defaultCwd: defaultCwd
        )
        return ExtensionBridgeShared.encodeExecResult(result)
    }

    @MainActor
    private func handleToast(args: [String: Any], appState: AppState) async throws -> Any {
        let title = (args["title"] as? String) ?? ""
        let body = (args["body"] as? String) ?? ""
        guard !title.isEmpty || !body.isEmpty else {
            throw APIError.invalidArguments("toast requires title or body")
        }
        let source = AIProviderRegistry.shared.notificationSource(for: extensionID)
        if let paneIDString = args["paneID"] as? String, let paneID = UUID(uuidString: paneIDString) {
            NotificationStore.shared.add(
                paneID: paneID,
                source: source,
                title: title,
                body: body,
                appState: appState
            )
            return NSNull()
        }
        guard let projectID = appState.activeProjectID,
              let key = appState.activeWorktreeKey(for: projectID),
              let root = appState.workspaceRoots[key]
        else { throw APIError.noActiveProject }
        for area in root.allAreas() {
            for tab in area.tabs where tab.content.pane != nil {
                let context = NavigationContext(
                    projectID: key.projectID,
                    worktreeID: key.worktreeID,
                    worktreePath: area.projectPath,
                    areaID: area.id,
                    tabID: tab.id
                )
                NotificationStore.shared.addWithContext(
                    context: context,
                    source: source,
                    title: title,
                    body: body,
                    appState: appState
                )
                return NSNull()
            }
        }
        throw APIError.noFocusedArea
    }

    private func stringArg(_ args: [String: Any], _ key: String) throws -> String {
        if let value = args[key] as? String { return value }
        throw APIError.invalidArguments("missing argument '\(key)'")
    }

    private func unwrap<T>(_ result: Result<T, APIError>) throws -> T {
        switch result {
        case let .success(value): return value
        case let .failure(error): throw error
        }
    }

    private func decodeOpenTabRequest(_ args: [String: Any]) throws -> OpenTabRequest {
        let data = try JSONSerialization.data(withJSONObject: args)
        do {
            return try JSONDecoder().decode(OpenTabRequest.self, from: data)
        } catch {
            throw APIError.invalidArguments("invalid open tab request: \(error.localizedDescription)")
        }
    }

    private func tabDict(_ tab: TabInfo) -> [String: Any] {
        [
            "index": tab.index,
            "id": tab.id.uuidString,
            "kind": tab.kind.rawValue,
            "title": tab.title,
            "isActive": tab.isActive,
        ]
    }

    private func paneDict(_ pane: PaneInfo) -> [String: Any] {
        [
            "id": pane.id.uuidString,
            "title": pane.title,
            "workingDirectory": pane.workingDirectory,
            "isFocused": pane.isFocused,
        ]
    }

    private func projectDict(_ project: ProjectInfo) -> [String: Any] {
        [
            "id": project.id.uuidString,
            "name": project.name,
            "path": project.path,
            "isActive": project.isActive,
        ]
    }

    private func worktreeDict(_ worktree: WorktreeInfo) -> [String: Any] {
        [
            "id": worktree.id.uuidString,
            "name": worktree.name,
            "path": worktree.path,
            "branch": worktree.branch ?? NSNull(),
            "isActive": worktree.isActive,
        ]
    }

    private static func bridgeScript(extensionID: String) -> String {
        let extLiteral = jsLiteral(extensionID)
        return """
        (() => {
            const dispatch = (verb, args) => {
                const reply = __muxyDispatch(verb, args || {});
                if (reply && reply.ok) return reply.value;
                throw new Error((reply && reply.error) || 'extension api error');
            };
            const muxy = {
                extensionID: \(extLiteral),
                toast: (opts) => dispatch('toast', opts || {}),
                tabs: {
                    list:     ()              => dispatch('tabs.list', {}),
                    switchTo: (identifier)    => dispatch('tabs.switch', { identifier: String(identifier) }),
                    new:      ()              => dispatch('tabs.new', {}),
                    next:     ()              => dispatch('tabs.next', {}),
                    previous: ()              => dispatch('tabs.previous', {}),
                    open:     (request)       => dispatch('tabs.open', request || {}),
                },
                panes: {
                    list:       ()                  => dispatch('panes.list', {}),
                    send:       (paneID, text)      => dispatch('panes.send', { paneID, text: String(text) }),
                    sendKeys:   (paneID, key)       => dispatch('panes.sendKeys', { paneID, key: String(key) }),
                    readScreen: (paneID, lines)     => dispatch('panes.readScreen', { paneID, lines: lines == null ? 50 : Number(lines) }),
                    close:      (paneID)            => dispatch('panes.close', { paneID }),
                    rename:     (paneID, title)     => dispatch('panes.rename', { paneID, title: String(title) }),
                },
                projects: {
                    list:     ()           => dispatch('projects.list', {}),
                    switchTo: (identifier) => dispatch('projects.switch', { identifier: String(identifier) }),
                },
                worktrees: {
                    list:     (project)             => dispatch('worktrees.list', { project: project == null ? null : String(project) }),
                    switchTo: (identifier, project) => dispatch('worktrees.switch', {
                        identifier: String(identifier),
                        project: project == null ? null : String(project),
                    }),
                    refresh:  (project)             => dispatch('worktrees.refresh', { project: project == null ? null : String(project) }),
                },
                exec(argvOrOptions, maybeOptions) {
                    let payload;
                    if (Array.isArray(argvOrOptions)) {
                        const opts = maybeOptions || {};
                        payload = { argv: argvOrOptions.map(String) };
                        if (opts.cwd != null) payload.cwd = String(opts.cwd);
                        if (opts.env) payload.env = opts.env;
                        if (opts.stdin != null) payload.stdin = String(opts.stdin);
                        if (opts.timeoutMs != null) payload.timeoutMs = Number(opts.timeoutMs);
                    } else {
                        const opts = argvOrOptions || {};
                        payload = {};
                        if (opts.shell != null) payload.shell = String(opts.shell);
                        if (opts.argv) payload.argv = opts.argv.map(String);
                        if (opts.cwd != null) payload.cwd = String(opts.cwd);
                        if (opts.env) payload.env = opts.env;
                        if (opts.stdin != null) payload.stdin = String(opts.stdin);
                        if (opts.timeoutMs != null) payload.timeoutMs = Number(opts.timeoutMs);
                    }
                    return dispatch('exec', payload);
                },
            };
            Object.freeze(muxy.tabs);
            Object.freeze(muxy.panes);
            Object.freeze(muxy.projects);
            Object.freeze(muxy.worktrees);
            Object.freeze(muxy);
            this.muxy = muxy;

            const formatForConsole = (value) => {
                if (value === null) return 'null';
                if (value === undefined) return 'undefined';
                if (typeof value === 'string') return value;
                if (value instanceof Error) return value.stack || value.message;
                try { return JSON.stringify(value); } catch (_) { return String(value); }
            };
            const consoleSend = (level, args) => {
                const message = Array.prototype.map.call(args, formatForConsole).join(' ');
                __muxyConsole(level, message);
            };
            this.console = {
                log:   function () { consoleSend('log', arguments); },
                warn:  function () { consoleSend('warn', arguments); },
                error: function () { consoleSend('err', arguments); },
            };
        })();
        """
    }

    private static func jsLiteral(_ value: String) -> String {
        guard let data = try? JSONEncoder().encode(value),
              let literal = String(data: data, encoding: .utf8)
        else { return "\"\"" }
        return literal
    }
}

private final class ResultBox<T>: @unchecked Sendable {
    var value: Result<T, Error>?
}

private struct AnyBox<T>: @unchecked Sendable {
    let value: T
    init(_ value: T) {
        self.value = value
    }
}

private struct BridgeValue: @unchecked Sendable {
    private let storage: Any

    init(from value: Any) throws {
        if value is NSNull || value is String || value is Int || value is Double || value is Bool {
            storage = value
            return
        }
        if let array = value as? [Any] {
            storage = array
            return
        }
        if let dict = value as? [String: Any] {
            storage = dict
            return
        }
        throw APIError.underlying("unsupported bridge value type")
    }

    func unwrap() -> Any {
        storage
    }
}

private func syncAwait<T: Sendable>(_ operation: @MainActor @Sendable @escaping () async throws -> T) throws -> T {
    let semaphore = DispatchSemaphore(value: 0)
    let box = ResultBox<T>()
    Task { @MainActor in
        do {
            box.value = try await .success(operation())
        } catch {
            box.value = .failure(error)
        }
        semaphore.signal()
    }
    semaphore.wait()
    guard let result = box.value else {
        throw APIError.underlying("script bridge produced no result")
    }
    switch result {
    case let .success(value): return value
    case let .failure(error): throw error
    }
}
