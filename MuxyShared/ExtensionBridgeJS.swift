import Foundation

public enum ExtensionBridgeJS {
    public enum Surface {
        case inProcess
        case background
    }

    public static func script(extensionID: String, surface: Surface) -> String {
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
                \(surface == .inProcess ? "toast: (opts) => dispatch('toast', opts || {})," : "")
                notifications: { notify: (opts) => dispatch('notifications.notify', opts || {}) },
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
                dialog: {
                    confirm(opts) {
                        const o = opts || {};
                        const payload = {};
                        if (o.title != null) payload.title = String(o.title);
                        if (o.message != null) payload.message = String(o.message);
                        if (Array.isArray(o.buttons)) payload.buttons = o.buttons.map(String);
                        if (o.default != null) payload.default = String(o.default);
                        if (o.cancel != null) payload.cancel = String(o.cancel);
                        if (o.style != null) payload.style = String(o.style);
                        return dispatch('dialog.confirm', payload);
                    },
                    alert(opts) {
                        const o = opts || {};
                        const payload = {};
                        if (o.title != null) payload.title = String(o.title);
                        if (o.message != null) payload.message = String(o.message);
                        if (o.style != null) payload.style = String(o.style);
                        return dispatch('dialog.alert', payload);
                    },
                },
                modal: {
                    open(opts) {
                        const o = opts || {};
                        const payload = { items: Array.isArray(o.items) ? o.items : [] };
                        if (o.placeholder != null) payload.placeholder = String(o.placeholder);
                        if (o.emptyLabel != null) payload.emptyLabel = String(o.emptyLabel);
                        if (o.noMatchLabel != null) payload.noMatchLabel = String(o.noMatchLabel);
                        return dispatch('modal.open', payload);
                    },
                },
            };
        \(surface == .inProcess ? workspaceBlock : "")
        \(surface == .inProcess ? filesBlock : "")
        \(surface == .background ? eventsBlock : "")
        \(surface == .background ? remoteBlock : "")
        \(gitBlock)
            \(surface == .inProcess ?
            "Object.freeze(muxy.tabs); Object.freeze(muxy.panes); Object.freeze(muxy.projects); Object.freeze(muxy.worktrees); Object.freeze(muxy.files);" :
            "")
            Object.freeze(muxy.git); Object.freeze(muxy.git.pr); Object.freeze(muxy.git.branch); Object.freeze(muxy.git.worktree);
            Object.freeze(muxy.notifications);
            Object.freeze(muxy.dialog);
            Object.freeze(muxy.modal);
            \(surface == .background ? "Object.freeze(muxy.events); Object.freeze(muxy.remote);" : "")
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

    public static func dispatchEvent(name: String, payloadJSON: String) -> String {
        """
        (() => {
            const store = globalThis.__muxyEventHandlers || {};
            const handlers = store[\(jsLiteral(name))] || [];
            const payload = \(payloadJSON);
            for (const handler of handlers.slice()) {
                try { handler(payload); } catch (e) { console.error(e); }
            }
        })();
        """
    }

    private static let workspaceBlock = """
            muxy.tabs = {
                list:     ()              => dispatch('tabs.list', {}),
                switchTo: (identifier)    => dispatch('tabs.switch', { identifier: String(identifier) }),
                new:      ()              => dispatch('tabs.new', {}),
                next:     ()              => dispatch('tabs.next', {}),
                previous: ()              => dispatch('tabs.previous', {}),
                open:     (request)       => dispatch('tabs.open', request || {}),
            };
            muxy.panes = {
                list:       ()                  => dispatch('panes.list', {}),
                send:       (paneID, text)      => dispatch('panes.send', { paneID, text: String(text) }),
                sendKeys:   (paneID, key)       => dispatch('panes.sendKeys', { paneID, key: String(key) }),
                readScreen: (paneID, lines)     => dispatch('panes.readScreen', { paneID, lines: lines == null ? 50 : Number(lines) }),
                close:      (paneID)            => dispatch('panes.close', { paneID }),
                rename:     (paneID, title)     => dispatch('panes.rename', { paneID, title: String(title) }),
            };
            muxy.projects = {
                list:     ()           => dispatch('projects.list', {}),
                switchTo: (identifier) => dispatch('projects.switch', { identifier: String(identifier) }),
            };
            muxy.worktrees = {
                list:     (project)             => dispatch('worktrees.list', { project: project == null ? null : String(project) }),
                switchTo: (identifier, project) => dispatch('worktrees.switch', {
                    identifier: String(identifier),
                    project: project == null ? null : String(project),
                }),
                refresh:  (project)             => dispatch('worktrees.refresh', { project: project == null ? null : String(project) }),
            };
    """

    private static let gitBlock = """
            const gitProject = (o) => (o && o.project != null ? String(o.project) : null);
            muxy.git = {
                status:        (o) => dispatch('git.status', { project: gitProject(o) }),
                diff:          (o) => dispatch('git.diff', {
                    project: gitProject(o),
                    filePath: String((o || {}).filePath || ''),
                    staged: (o || {}).staged == null ? null : Boolean(o.staged),
                    lineLimit: (o || {}).lineLimit == null ? null : Number(o.lineLimit),
                }),
                log:           (o) => dispatch('git.log', {
                    project: gitProject(o),
                    maxCount: (o || {}).maxCount == null ? null : Number(o.maxCount),
                    skip: (o || {}).skip == null ? null : Number(o.skip),
                }),
                branches:      (o) => dispatch('git.branches', { project: gitProject(o) }),
                currentBranch: (o) => dispatch('git.currentBranch', { project: gitProject(o) }),
                aheadBehind:   (o) => dispatch('git.aheadBehind', { project: gitProject(o) }),
                worktrees:     (o) => dispatch('git.worktrees', { project: gitProject(o) }),
                stage:         (o) => dispatch('git.stage', { project: gitProject(o), paths: ((o || {}).paths || []).map(String) }),
                unstage:       (o) => dispatch('git.unstage', { project: gitProject(o), paths: ((o || {}).paths || []).map(String) }),
                discard:       (o) => dispatch('git.discard', {
                    project: gitProject(o),
                    paths: ((o || {}).paths || []).map(String),
                    untrackedPaths: ((o || {}).untrackedPaths || []).map(String),
                }),
                commit:        (o) => dispatch('git.commit', {
                    project: gitProject(o),
                    message: String((o || {}).message || ''),
                    stageAll: Boolean((o || {}).stageAll),
                }),
                push:          (o) => dispatch('git.push', { project: gitProject(o) }),
                pull:          (o) => dispatch('git.pull', { project: gitProject(o) }),
                branch: {
                    create: (o) => dispatch('git.branch.create', { project: gitProject(o), name: String((o || {}).name || '') }),
                    switchTo: (o) => dispatch('git.branch.switch', { project: gitProject(o), branch: String((o || {}).branch || '') }),
                },
                pr: {
                    info:   (o) => dispatch('git.pr.info', { project: gitProject(o) }),
                    list:   (o) => dispatch('git.pr.list', {
                        project: gitProject(o),
                        filter: (o || {}).filter == null ? null : String(o.filter),
                        limit: (o || {}).limit == null ? null : Number(o.limit),
                    }),
                    create: (o) => dispatch('git.pr.create', {
                        project: gitProject(o),
                        title: String((o || {}).title || ''),
                        body: String((o || {}).body || ''),
                        baseBranch: (o || {}).baseBranch == null ? null : String(o.baseBranch),
                        draft: Boolean((o || {}).draft),
                    }),
                    merge:  (o) => dispatch('git.pr.merge', {
                        project: gitProject(o),
                        number: Number((o || {}).number),
                        method: (o || {}).method == null ? null : String(o.method),
                        deleteBranch: (o || {}).deleteBranch == null ? true : Boolean(o.deleteBranch),
                    }),
                    close:  (o) => dispatch('git.pr.close', { project: gitProject(o), number: Number((o || {}).number) }),
                },
                worktree: {
                    add: (o) => dispatch('git.worktree.add', {
                        project: gitProject(o),
                        path: String((o || {}).path || ''),
                        branch: String((o || {}).branch || ''),
                        createBranch: Boolean((o || {}).createBranch),
                        baseBranch: (o || {}).baseBranch == null ? null : String(o.baseBranch),
                    }),
                    remove: (o) => dispatch('git.worktree.remove', {
                        project: gitProject(o),
                        path: String((o || {}).path || ''),
                        force: Boolean((o || {}).force),
                    }),
                },
            };
    """

    private static let filesBlock = """
            const filesProject = (o) => (o && o.project != null ? String(o.project) : null);
            muxy.files = {
                list:   (path, o) => dispatch('files.list', { project: filesProject(o), path: String(path == null ? '' : path) }),
                read:   (path, o) => dispatch('files.read', { project: filesProject(o), path: String(path == null ? '' : path) }),
                stat:   (path, o) => dispatch('files.stat', { project: filesProject(o), path: String(path == null ? '' : path) }),
                write:  (path, contents, o) => dispatch('files.write', {
                    project: filesProject(o),
                    path: String(path == null ? '' : path),
                    contents: String(contents == null ? '' : contents),
                }),
                mkdir:  (path, o) => dispatch('files.mkdir', { project: filesProject(o), path: String(path == null ? '' : path) }),
                rename: (path, newName, o) => dispatch('files.rename', {
                    project: filesProject(o),
                    path: String(path == null ? '' : path),
                    newName: String(newName == null ? '' : newName),
                }),
                move:   (paths, into, o) => dispatch('files.move', {
                    project: filesProject(o),
                    paths: (paths || []).map(String),
                    into: String(into == null ? '' : into),
                }),
                delete: (paths, o) => dispatch('files.delete', { project: filesProject(o), paths: (paths || []).map(String) }),
            };
    """

    private static let eventsBlock = """
            const handlerStore = {};
            this.__muxyEventHandlers = handlerStore;
            muxy.events = {
                subscribe(name, handler) {
                    const key = String(name);
                    if (!handlerStore[key]) {
                        handlerStore[key] = [];
                        __muxySubscribe(key);
                    }
                    handlerStore[key].push(handler);
                },
                unsubscribe(name, handler) {
                    const key = String(name);
                    const list = handlerStore[key];
                    if (!list) return;
                    const index = list.indexOf(handler);
                    if (index >= 0) list.splice(index, 1);
                },
            };
    """

    private static let remoteBlock = """
            const remoteHandlers = {};
            this.__muxyRemoteHandlers = remoteHandlers;
            muxy.remote = {
                handle(action, handler) {
                    remoteHandlers[String(action)] = handler;
                },
                unhandle(action) {
                    delete remoteHandlers[String(action)];
                },
            };
            this.__muxyDispatchInvoke = (callID, action, argument) => {
                const handler = remoteHandlers[String(action)];
                if (typeof handler !== 'function') {
                    __muxyInvokeReject(callID, "no handler registered for '" + action + "'");
                    return;
                }
                let result;
                try {
                    result = handler(argument);
                } catch (error) {
                    __muxyInvokeReject(callID, String((error && error.message) || error));
                    return;
                }
                Promise.resolve(result).then(
                    (value) => {
                        let json;
                        try {
                            json = JSON.stringify(value === undefined ? null : value);
                        } catch (e) {
                            __muxyInvokeReject(callID, 'result is not serializable');
                            return;
                        }
                        __muxyInvokeResolve(callID, json == null ? 'null' : json);
                    },
                    (error) => {
                        __muxyInvokeReject(callID, String((error && error.message) || error));
                    }
                );
            };
    """

    private static func jsLiteral(_ value: String) -> String {
        guard let data = try? JSONEncoder().encode(value),
              let literal = String(data: data, encoding: .utf8)
        else { return "\"\"" }
        return literal
    }
}
