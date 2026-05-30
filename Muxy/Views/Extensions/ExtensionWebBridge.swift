import Foundation

enum ExtensionWebBridge {
    static let messageHandlerName = "muxy"

    static func script(
        extensionID: String,
        tabInstanceID: String,
        data: ExtensionJSON?,
        theme: [String: String]
    ) -> String {
        let encodedData = encodeAsLiteral(data)
        let extensionLiteral = jsLiteral(extensionID)
        let instanceLiteral = jsLiteral(tabInstanceID)
        let themeLiteral = jsonObjectLiteral(theme)
        return """
        (() => {
            const handler = window.webkit?.messageHandlers?.muxy;
            if (!handler) return;
            let nextID = 1;

            const send = async (verb, args) => {
                const requestID = String(nextID++);
                const reply = await handler.postMessage({ verb, args: args ?? {}, requestID });
                if (reply && reply.ok) return reply.value;
                const message = reply && reply.error ? String(reply.error) : 'extension api error';
                throw new Error(message);
            };

            const themeListeners = new Set();
            let currentTheme = \(themeLiteral);

            const writeThemeToDocument = (theme) => {
                const root = document.documentElement;
                if (!root) return;
                for (const [key, value] of Object.entries(theme)) {
                    const cssName = key.replace(/[A-Z]/g, (m) => '-' + m.toLowerCase());
                    root.style.setProperty(`--muxy-${cssName}`, value);
                }
                root.style.colorScheme = theme.colorScheme || 'light';
            };

            window.__muxyApplyTheme = (theme) => {
                if (!theme || typeof theme !== 'object') return;
                currentTheme = Object.freeze({ ...theme });
                writeThemeToDocument(currentTheme);
                for (const listener of themeListeners) {
                    try { listener(currentTheme); } catch (_) {}
                }
            };

            if (document.documentElement) writeThemeToDocument(currentTheme);
            else document.addEventListener('DOMContentLoaded', () => writeThemeToDocument(currentTheme), { once: true });

            const eventListeners = new Map();
            window.__muxyEventDispatch = (name, payload) => {
                const listeners = eventListeners.get(name);
                if (!listeners) return;
                for (const callback of listeners) {
                    try { callback(payload || {}); } catch (_) {}
                }
            };

            const muxy = {
                extensionID: \(extensionLiteral),
                tabInstanceID: \(instanceLiteral),
                data: \(encodedData),
                get theme() { return currentTheme; },
                onThemeChange(callback) {
                    if (typeof callback !== 'function') return () => {};
                    themeListeners.add(callback);
                    return () => themeListeners.delete(callback);
                },
                toast(opts) {
                    return send('toast', opts || {});
                },
                tabs: {
                    open(request) { return send('tabs.open', request || {}); },
                    list() { return send('tabs.list', {}); },
                    switchTo(identifier) { return send('tabs.switch', { identifier: String(identifier) }); },
                    new() { return send('tabs.new', {}); },
                    next() { return send('tabs.next', {}); },
                    previous() { return send('tabs.previous', {}); },
                },
                panes: {
                    list() { return send('panes.list', {}); },
                    send(paneID, text) { return send('panes.send', { paneID, text: String(text) }); },
                    sendKeys(paneID, key) { return send('panes.sendKeys', { paneID, key: String(key) }); },
                    readScreen(paneID, lines) {
                        return send('panes.readScreen', { paneID, lines: lines == null ? 50 : Number(lines) });
                    },
                    close(paneID) { return send('panes.close', { paneID }); },
                    rename(paneID, title) { return send('panes.rename', { paneID, title: String(title) }); },
                },
                projects: {
                    list() { return send('projects.list', {}); },
                    switchTo(identifier) { return send('projects.switch', { identifier: String(identifier) }); },
                },
                panels: {
                    open(panel, data) { return send('panel.open', { panel: String(panel), data: data ?? null }); },
                    toggle(panel, data) { return send('panel.toggle', { panel: String(panel), data: data ?? null }); },
                    close(panel) { return send('panel.close', { panel: String(panel) }); },
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
                    return send('exec', payload);
                },
                worktrees: {
                    list(project) { return send('worktrees.list', { project: project == null ? null : String(project) }); },
                    switchTo(identifier, project) {
                        return send('worktrees.switch', {
                            identifier: String(identifier),
                            project: project == null ? null : String(project),
                        });
                    },
                    refresh(project) { return send('worktrees.refresh', { project: project == null ? null : String(project) }); },
                },
                events: {
                    subscribe(name, callback) {
                        if (typeof name !== 'string' || typeof callback !== 'function') {
                            return () => {};
                        }
                        let set = eventListeners.get(name);
                        if (!set) {
                            set = new Set();
                            eventListeners.set(name, set);
                            send('events.subscribe', { event: name }).catch((err) => {
                                eventListeners.delete(name);
                                try { console.error('muxy.events.subscribe failed:', err.message || err); } catch (_) {}
                            });
                        }
                        set.add(callback);
                        return () => {
                            const current = eventListeners.get(name);
                            if (!current) return;
                            current.delete(callback);
                            if (current.size === 0) {
                                eventListeners.delete(name);
                                send('events.unsubscribe', { event: name }).catch(() => {});
                            }
                        };
                    },
                },
            };

            Object.freeze(muxy.tabs);
            Object.freeze(muxy.panes);
            Object.freeze(muxy.projects);
            Object.freeze(muxy.panels);
            Object.freeze(muxy.worktrees);
            Object.freeze(muxy.events);
            Object.freeze(muxy);
            window.muxy = muxy;

            const consoleHandler = window.webkit?.messageHandlers?.muxyConsole;
            if (consoleHandler) {
                const formatForConsole = (value) => {
                    if (value === null) return 'null';
                    if (value === undefined) return 'undefined';
                    if (typeof value === 'string') return value;
                    if (value instanceof Error) return value.stack || value.message;
                    try { return JSON.stringify(value); } catch (_) { return String(value); }
                };
                const wrap = (originalFn, level) => function () {
                    const message = Array.prototype.map.call(arguments, formatForConsole).join(' ');
                    try { consoleHandler.postMessage({ level, message }); } catch (_) {}
                    if (originalFn) {
                        try { originalFn.apply(console, arguments); } catch (_) {}
                    }
                };
                console.log = wrap(console.log, 'log');
                console.warn = wrap(console.warn, 'warn');
                console.error = wrap(console.error, 'err');

                window.addEventListener('error', (event) => {
                    try {
                        const detail = event.error ? formatForConsole(event.error)
                            : (event.message || 'unknown error');
                        consoleHandler.postMessage({ level: 'err', message: detail });
                    } catch (_) {}
                });
                window.addEventListener('unhandledrejection', (event) => {
                    try {
                        const reason = event.reason !== undefined ? formatForConsole(event.reason) : 'unhandledrejection';
                        consoleHandler.postMessage({ level: 'err', message: reason });
                    } catch (_) {}
                });
            }
        })();
        """
    }

    static func themeUpdateScript(theme: [String: String]) -> String {
        let literal = jsonObjectLiteral(theme)
        return """
        (() => {
            if (typeof window.__muxyApplyTheme === 'function') {
                window.__muxyApplyTheme(\(literal));
            }
        })();
        """
    }

    private static func jsLiteral(_ value: String) -> String {
        guard let data = try? JSONEncoder().encode(value),
              let literal = String(data: data, encoding: .utf8)
        else {
            return "\"\""
        }
        return literal
    }

    private static func jsonObjectLiteral(_ object: [String: String]) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys]),
              let literal = String(data: data, encoding: .utf8)
        else {
            return "{}"
        }
        return literal
    }

    private static func encodeAsLiteral(_ value: ExtensionJSON?) -> String {
        guard let value else { return "null" }
        guard let data = try? JSONEncoder().encode(value),
              let literal = String(data: data, encoding: .utf8)
        else {
            return "null"
        }
        return literal
    }
}
