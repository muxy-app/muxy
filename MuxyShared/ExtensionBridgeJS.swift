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
            };
        \(surface == .inProcess ? workspaceBlock : "")
        \(surface == .background ? eventsBlock : "")
        \(surface == .background ? remoteBlock : "")
            \(surface == .inProcess ?
            "Object.freeze(muxy.tabs); Object.freeze(muxy.panes); Object.freeze(muxy.projects); Object.freeze(muxy.worktrees);" :
            "")
            Object.freeze(muxy.notifications);
            Object.freeze(muxy.dialog);
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
