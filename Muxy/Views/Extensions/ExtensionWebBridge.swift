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

            const gitProject = (o) => (o && o.project != null ? String(o.project) : null);
            const filesProject = (o) => (o && o.project != null ? String(o.project) : null);

            const themeListeners = new Set();
            let currentTheme = \(themeLiteral);

            const dataListeners = new Set();
            let currentData = \(encodedData);

            window.__muxyApplyData = (data) => {
                currentData = data ?? null;
                for (const listener of dataListeners) {
                    try { listener(currentData); } catch (_) {}
                }
            };

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
            const isExtensionLocalEvent = (name) => {
                const key = String(name);
                return key.startsWith('extension.') && key.length > 'extension.'.length;
            };

            const normalizeModalItems = (raw) => (Array.isArray(raw) ? raw : (raw && raw.items) || [])
                .map((it) => (it && it.id != null && it.title != null
                    ? { id: String(it.id), title: String(it.title), subtitle: it.subtitle == null ? null : String(it.subtitle) }
                    : null))
                .filter(Boolean);
            const modalLabels = (o) => {
                const labels = {};
                if (o.placeholder != null) labels.placeholder = String(o.placeholder);
                if (o.emptyLabel != null) labels.emptyLabel = String(o.emptyLabel);
                if (o.noMatchLabel != null) labels.noMatchLabel = String(o.noMatchLabel);
                return labels;
            };

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
                get data() { return currentData; },
                onDataChange(callback) {
                    if (typeof callback !== 'function') return () => {};
                    dataListeners.add(callback);
                    return () => dataListeners.delete(callback);
                },
                get theme() { return currentTheme; },
                onThemeChange(callback) {
                    if (typeof callback !== 'function') return () => {};
                    themeListeners.add(callback);
                    return () => themeListeners.delete(callback);
                },
                toast(opts) {
                    return send('toast', opts || {});
                },
                notifications: {
                    notify(opts) { return send('notifications.notify', opts || {}); },
                },
                tabs: {
                    open(request) { return send('tabs.open', request || {}); },
                    list() { return send('tabs.list', {}); },
                    switchTo(identifier) { return send('tabs.switch', { identifier: String(identifier) }); },
                    new() { return send('tabs.new', {}); },
                    next() { return send('tabs.next', {}); },
                    previous() { return send('tabs.previous', {}); },
                    setTitle(title) {
                        return send('tabs.setTitle', { tabInstanceID: muxy.tabInstanceID, title: String(title == null ? '' : title) });
                    },
                    setIcon(icon) {
                        return send('tabs.setIcon', { tabInstanceID: muxy.tabInstanceID, icon: icon ?? null });
                    },
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
                popover: {
                    close() { return send('popover.close', {}); },
                    resize(width, height) { return send('popover.resize', { width: Number(width), height: Number(height) }); },
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
                        return send('dialog.confirm', payload);
                    },
                    alert(opts) {
                        const o = opts || {};
                        const payload = {};
                        if (o.title != null) payload.title = String(o.title);
                        if (o.message != null) payload.message = String(o.message);
                        if (o.style != null) payload.style = String(o.style);
                        return send('dialog.alert', payload);
                    },
                },
                modal: {
                    async open(opts) {
                        const o = opts || {};
                        const opened = await send('modal.open', modalLabels(o));
                        const requestID = opened && opened.requestID;
                        const emit = (batch) => send('modal.feed', { items: normalizeModalItems(batch) });
                        if (typeof o.items === 'function') {
                            const produced = await o.items(emit);
                            if (produced != null) await emit(produced);
                        } else {
                            await emit(o.items);
                        }
                        await send('modal.finish', {});
                        const choice = await send('modal.await', { requestID });
                        if (typeof o.onSelect === 'function') o.onSelect(choice);
                        return choice;
                    },
                },
                topbar: {
                    set(opts) {
                        const o = opts || {};
                        const payload = { id: String(o.id == null ? '' : o.id) };
                        if (o.icon != null) payload.icon = o.icon;
                        if ('visible' in o) payload.visible = !!o.visible;
                        return send('topbar.set', payload);
                    },
                    show(id) { return send('topbar.set', { id: String(id == null ? '' : id), visible: true }); },
                    hide(id) { return send('topbar.set', { id: String(id == null ? '' : id), visible: false }); },
                },
                statusbar: {
                    set(opts) {
                        const o = opts || {};
                        const payload = { id: String(o.id == null ? '' : o.id) };
                        if (o.icon != null) payload.icon = o.icon;
                        if ('text' in o) payload.text = o.text == null ? null : String(o.text);
                        if ('visible' in o) payload.visible = !!o.visible;
                        return send('statusbar.set', payload);
                    },
                    show(id) { return send('statusbar.set', { id: String(id == null ? '' : id), visible: true }); },
                    hide(id) { return send('statusbar.set', { id: String(id == null ? '' : id), visible: false }); },
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
                http: {
                    fetch(url, options) {
                        const opts = options || {};
                        const payload = { url: String(url) };
                        if (opts.method != null) payload.method = String(opts.method);
                        if (opts.headers) payload.headers = opts.headers;
                        if (opts.body != null) payload.body = String(opts.body);
                        if (opts.timeoutMs != null) payload.timeoutMs = Number(opts.timeoutMs);
                        return send('http.fetch', payload);
                    },
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
                git: {
                    status(o) { return send('git.status', {
                        project: gitProject(o),
                        local: Boolean((o || {}).local),
                        fresh: Boolean((o || {}).fresh),
                    }); },
                    diff(o) { return send('git.diff', {
                        project: gitProject(o),
                        filePath: String((o || {}).filePath || ''),
                        raw: Boolean((o || {}).raw),
                        staged: (o || {}).staged == null ? null : Boolean(o.staged),
                        lineLimit: (o || {}).lineLimit == null ? null : Number(o.lineLimit),
                        fresh: Boolean((o || {}).fresh),
                    }); },
                    repoInfo(o) { return send('git.repoInfo', { project: gitProject(o) }); },
                    log(o) { return send('git.log', {
                        project: gitProject(o),
                        maxCount: (o || {}).maxCount == null ? null : Number(o.maxCount),
                        skip: (o || {}).skip == null ? null : Number(o.skip),
                        fresh: Boolean((o || {}).fresh),
                    }); },
                    branches(o) { return send('git.branches', { project: gitProject(o) }); },
                    remoteBranches(o) { return send('git.remoteBranches', { project: gitProject(o) }); },
                    currentBranch(o) { return send('git.currentBranch', { project: gitProject(o) }); },
                    aheadBehind(o) { return send('git.aheadBehind', { project: gitProject(o), fresh: Boolean((o || {}).fresh) }); },
                    init(o) { return send('git.init', { project: gitProject(o) }); },
                    worktrees(o) { return send('git.worktrees', { project: gitProject(o) }); },
                    checkout(o) { return send('git.checkout', { project: gitProject(o), hash: String((o || {}).hash || '') }); },
                    cherryPick(o) { return send('git.cherryPick', { project: gitProject(o), hash: String((o || {}).hash || '') }); },
                    revert(o) { return send('git.revert', { project: gitProject(o), hash: String((o || {}).hash || '') }); },
                    stage(o) { return send('git.stage', { project: gitProject(o), paths: ((o || {}).paths || []).map(String) }); },
                    unstage(o) { return send('git.unstage', { project: gitProject(o), paths: ((o || {}).paths || []).map(String) }); },
                    discard(o) { return send('git.discard', {
                        project: gitProject(o),
                        paths: ((o || {}).paths || []).map(String),
                        untrackedPaths: ((o || {}).untrackedPaths || []).map(String),
                    }); },
                    commit(o) { return send('git.commit', {
                        project: gitProject(o),
                        message: String((o || {}).message || ''),
                        stageAll: Boolean((o || {}).stageAll),
                    }); },
                    push(o) { return send('git.push', { project: gitProject(o), setUpstream: Boolean((o || {}).setUpstream) }); },
                    pull(o) { return send('git.pull', { project: gitProject(o) }); },
                    branch: {
                        create(o) {
                            return send('git.branch.create', { project: gitProject(o), name: String((o || {}).name || '') });
                        },
                        switchTo(o) {
                            return send('git.branch.switch', { project: gitProject(o), branch: String((o || {}).branch || '') });
                        },
                        delete(o) {
                            return send('git.branch.delete', {
                                project: gitProject(o),
                                name: String((o || {}).name || ''),
                                force: Boolean((o || {}).force),
                            });
                        },
                        deleteRemote(o) {
                            return send('git.branch.deleteRemote', { project: gitProject(o), branch: String((o || {}).branch || '') });
                        },
                    },
                    tag: {
                        create(o) { return send('git.tag.create', {
                            project: gitProject(o),
                            name: String((o || {}).name || ''),
                            hash: String((o || {}).hash || ''),
                        }); },
                    },
                    pr: {
                        info(o) { return send('git.pr.info', { project: gitProject(o), fresh: Boolean((o || {}).fresh) }); },
                        number(o) { return send('git.pr.number', { project: gitProject(o), fresh: Boolean((o || {}).fresh) }); },
                        diff(o) { return send('git.pr.diff', {
                            project: gitProject(o),
                            number: Number((o || {}).number),
                            lineLimit: (o || {}).lineLimit == null ? null : Number(o.lineLimit),
                            fresh: Boolean((o || {}).fresh),
                        }); },
                        checkout(o) { return send('git.pr.checkout', { project: gitProject(o), number: Number((o || {}).number) }); },
                        checkoutWorktree(o) { return send('git.pr.checkoutWorktree', {
                            project: gitProject(o),
                            path: String((o || {}).path || ''),
                            number: Number((o || {}).number),
                        }); },
                        list(o) { return send('git.pr.list', {
                            project: gitProject(o),
                            filter: (o || {}).filter == null ? null : String(o.filter),
                            limit: (o || {}).limit == null ? null : Number(o.limit),
                        }); },
                        create(o) { return send('git.pr.create', {
                            project: gitProject(o),
                            title: String((o || {}).title || ''),
                            body: String((o || {}).body || ''),
                            baseBranch: (o || {}).baseBranch == null ? null : String(o.baseBranch),
                            draft: Boolean((o || {}).draft),
                        }); },
                        merge(o) { return send('git.pr.merge', {
                            project: gitProject(o),
                            number: Number((o || {}).number),
                            method: (o || {}).method == null ? null : String(o.method),
                            deleteBranch: (o || {}).deleteBranch == null ? true : Boolean(o.deleteBranch),
                        }); },
                        close(o) { return send('git.pr.close', { project: gitProject(o), number: Number((o || {}).number) }); },
                    },
                    worktree: {
                        add(o) { return send('git.worktree.add', {
                            project: gitProject(o),
                            path: String((o || {}).path || ''),
                            branch: String((o || {}).branch || ''),
                            createBranch: Boolean((o || {}).createBranch),
                            baseBranch: (o || {}).baseBranch == null ? null : String(o.baseBranch),
                        }); },
                        remove(o) { return send('git.worktree.remove', {
                            project: gitProject(o),
                            path: String((o || {}).path || ''),
                            force: Boolean((o || {}).force),
                        }); },
                        switchTo(o) {
                            return send('git.worktree.switch', { project: gitProject(o), identifier: String((o || {}).identifier || '') });
                        },
                    },
                },
                files: {
                    list(path, o) { return send('files.list', { project: filesProject(o), path: String(path == null ? '' : path) }); },
                    read(path, o) { return send('files.read', { project: filesProject(o), path: String(path == null ? '' : path) }); },
                    stat(path, o) { return send('files.stat', { project: filesProject(o), path: String(path == null ? '' : path) }); },
                    write(path, contents, o) {
                        return send('files.write', {
                            project: filesProject(o),
                            path: String(path == null ? '' : path),
                            contents: String(contents == null ? '' : contents),
                        });
                    },
                    mkdir(path, o) { return send('files.mkdir', { project: filesProject(o), path: String(path == null ? '' : path) }); },
                    rename(path, newName, o) {
                        return send('files.rename', {
                            project: filesProject(o),
                            path: String(path == null ? '' : path),
                            newName: String(newName == null ? '' : newName),
                        });
                    },
                    move(paths, into, o) {
                        return send('files.move', {
                            project: filesProject(o),
                            paths: (paths || []).map(String),
                            into: String(into == null ? '' : into),
                        });
                    },
                    delete(paths, o) {
                        return send('files.delete', { project: filesProject(o), paths: (paths || []).map(String) });
                    },
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
                    emit(name, payload) {
                        const key = String(name);
                        if (!isExtensionLocalEvent(key)) {
                            return Promise.reject(new Error('extension events must start with extension.'));
                        }
                        return send('events.emit', { event: key, payload: payload === undefined ? null : payload });
                    },
                },
            };

            Object.freeze(muxy.notifications);
            Object.freeze(muxy.tabs);
            Object.freeze(muxy.panes);
            Object.freeze(muxy.projects);
            Object.freeze(muxy.panels);
            Object.freeze(muxy.popover);
            Object.freeze(muxy.dialog);
            Object.freeze(muxy.modal);
            Object.freeze(muxy.topbar);
            Object.freeze(muxy.statusbar);
            Object.freeze(muxy.worktrees);
            Object.freeze(muxy.git);
            Object.freeze(muxy.git.pr);
            Object.freeze(muxy.git.branch);
            Object.freeze(muxy.git.tag);
            Object.freeze(muxy.git.worktree);
            Object.freeze(muxy.files);
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

    static func dataUpdateScript(data: ExtensionJSON?) -> String {
        let literal = encodeAsLiteral(data)
        return """
        (() => {
            if (typeof window.__muxyApplyData === 'function') {
                window.__muxyApplyData(\(literal));
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
