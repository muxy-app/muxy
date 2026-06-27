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
            const mapResult = (value, fn) => (value && typeof value.then === 'function') ? value.then(fn) : fn(value);
            const parseJSON = (value) => { try { return JSON.parse(value); } catch (e) { return value; } };
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
                if ('searchToolbar' in o) labels.searchToolbar = !!o.searchToolbar;
                if (typeof o.onQuery === 'function' || typeof o.onQueryChange === 'function') labels.dynamic = true;
                return labels;
            };
            const feedModalItems = (o) => {
                const emit = (batch) => dispatch('modal.feed', { items: normalizeModalItems(batch) });
                if (typeof o.items === 'function') {
                    const produced = o.items(emit);
                    if (produced != null) emit(produced);
                } else {
                    emit(o.items);
                }
                dispatch('modal.finish', {});
            };
            const modalResultHandlers = {};
            const modalQueryHandlers = {};
            let activeModalQueryID = null;
            this.__muxiDeliverModalResult = (requestID, item) => {
                const handler = modalResultHandlers[requestID];
                delete modalResultHandlers[requestID];
                delete modalQueryHandlers[requestID];
                if (typeof handler === 'function') {
                    try { handler(item == null ? null : item); } catch (error) { console.error(error); }
                }
            };
            this.__muxyDeliverModalQuery = (requestID, queryID, query, options) => {
                const handler = modalQueryHandlers[requestID];
                const emit = (batch) => dispatch('modal.feed', { items: normalizeModalItems(batch), queryID });
                const finish = () => dispatch('modal.finish', { queryID });
                if (typeof handler !== 'function') { finish(); return; }
                let produced;
                const previousModalQueryID = activeModalQueryID;
                activeModalQueryID = queryID;
                try {
                    produced = handler(query, emit, options || {});
                } catch (error) {
                    console.error(error);
                    finish();
                    return;
                } finally {
                    activeModalQueryID = previousModalQueryID;
                }
                if (produced != null) emit(produced);
                finish();
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
                    prompt(opts) {
                        const o = opts || {};
                        const payload = {};
                        if (o.title != null) payload.title = String(o.title);
                        if (o.message != null) payload.message = String(o.message);
                        if (o.default != null) payload.default = String(o.default);
                        if (o.placeholder != null) payload.placeholder = String(o.placeholder);
                        if (o.confirm != null) payload.confirm = String(o.confirm);
                        if (o.cancel != null) payload.cancel = String(o.cancel);
                        return dispatch('dialog.prompt', payload);
                    },
                    pickFolder(opts) {
                        const o = opts || {};
                        const payload = {};
                        if (o.title != null) payload.title = String(o.title);
                        if (o.message != null) payload.message = String(o.message);
                        if (o.default != null) payload.default = String(o.default);
                        return dispatch('dialog.pickFolder', payload);
                    },
                },
                storage: {
                    get(key) { return dispatch('storage.get', { key: String(key) }); },
                    set(key, value) { return dispatch('storage.set', { key: String(key), value: value === undefined ? null : value }); },
                    delete(key) { return dispatch('storage.delete', { key: String(key) }); },
                    keys() { return dispatch('storage.keys', {}); },
                },
                shortcuts: {
                    register(opts) {
                        const o = opts || {};
                        return dispatch('shortcuts.register', { id: String(o.id == null ? '' : o.id), combo: String(o.combo == null ? '' : o.combo) });
                    },
                    unregister(id) { return dispatch('shortcuts.unregister', { id: String(id == null ? '' : id) }); },
                    list() { return dispatch('shortcuts.list', {}); },
                },
                modal: {
                    open(opts) {
                        const o = opts || {};
                        const labels = modalLabels(o);
                        const opened = dispatch('modal.open', labels);
                        const requestID = opened && opened.requestID;
                        if (requestID != null) {
                            if (typeof o.onSelect === 'function') modalResultHandlers[requestID] = o.onSelect;
                            if (typeof o.onQuery === 'function') {
                                modalQueryHandlers[requestID] = o.onQuery;
                            } else if (typeof o.onQueryChange === 'function') {
                                modalQueryHandlers[requestID] = (query, emit, options) => o.onQueryChange(query, options || {});
                            }
                        }
                        feedModalItems(o);
                        return requestID;
                    },
                    feed(items) {
                        const payload = { items: normalizeModalItems(items) };
                        if (activeModalQueryID != null) payload.queryID = activeModalQueryID;
                        return dispatch('modal.feed', payload);
                    },
                    finish() {
                        const payload = {};
                        if (activeModalQueryID != null) payload.queryID = activeModalQueryID;
                        return dispatch('modal.finish', payload);
                    },
                },
                topbar: {
                    set(opts) {
                        const o = opts || {};
                        const payload = { id: String(o.id == null ? '' : o.id) };
                        if (o.icon != null) payload.icon = o.icon;
                        if ('visible' in o) payload.visible = !!o.visible;
                        return dispatch('topbar.set', payload);
                    },
                    show(id) { return dispatch('topbar.set', { id: String(id == null ? '' : id), visible: true }); },
                    hide(id) { return dispatch('topbar.set', { id: String(id == null ? '' : id), visible: false }); },
                },
                statusbar: {
                    set(opts) {
                        const o = opts || {};
                        const payload = { id: String(o.id == null ? '' : o.id) };
                        if (o.icon != null) payload.icon = o.icon;
                        if ('text' in o) payload.text = o.text == null ? null : String(o.text);
                        if ('visible' in o) payload.visible = !!o.visible;
                        return dispatch('statusbar.set', payload);
                    },
                    show(id) { return dispatch('statusbar.set', { id: String(id == null ? '' : id), visible: true }); },
                    hide(id) { return dispatch('statusbar.set', { id: String(id == null ? '' : id), visible: false }); },
                },
            };
        \(surface == .inProcess ? workspaceBlock : "")
        \(surface == .background ? backgroundTabsBlock : "")
        \(surface == .inProcess ? filesBlock : "")
        \(surface == .background ? eventsBlock : "")
        \(surface == .background ? remoteBlock : "")
        \(gitBlock)
        \(agentsBlock)
            \(surface == .inProcess ?
            "Object.freeze(muxy.tabs); Object.freeze(muxy.browser); Object.freeze(muxy.panes); Object.freeze(muxy.projects); Object.freeze(muxy.worktrees); Object.freeze(muxy.files);" :
            "")
            \(surface == .background ? "Object.freeze(muxy.tabs);" : "")
            Object.freeze(muxy.git); Object.freeze(muxy.git.pr); Object.freeze(muxy.git.branch); Object.freeze(muxy.git.worktree);
            Object.freeze(muxy.agents);
            Object.freeze(muxy.notifications);
            Object.freeze(muxy.dialog);
            Object.freeze(muxy.shortcuts);
            Object.freeze(muxy.storage);
            Object.freeze(muxy.modal);
            Object.freeze(muxy.topbar);
            Object.freeze(muxy.statusbar);
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
            muxy.browser = {
                open:      (url, opts)        => dispatch('browser.open', { url: url == null ? null : String(url), split: Boolean((opts || {}).split) }),
                navigate:  (tabId, url)       => dispatch('browser.navigate', { tabId: String(tabId), url: String(url) }),
                list:      ()                 => dispatch('browser.list', {}),
                read:      (tabId)            => dispatch('browser.read', { tabId: String(tabId) }),
                close:     (tabId)            => dispatch('browser.close', { tabId: String(tabId) }),
                eval:      (tabId, script)    => mapResult(dispatch('browser.eval', { tabId: String(tabId), script: String(script) }), parseJSON),
                click:     (tabId, selector)  => dispatch('browser.click', { tabId: String(tabId), selector: String(selector) }),
                type:      (tabId, selector, text, opts) => dispatch('browser.type', { tabId: String(tabId), selector: String(selector), text: String(text), submit: Boolean((opts || {}).submit) }),
                waitFor:   (tabId, selector, opts) => dispatch('browser.waitFor', { tabId: String(tabId), selector: String(selector), timeoutMs: Number((opts || {}).timeoutMs == null ? 5000 : (opts || {}).timeoutMs) }),
                wait:      (tabId, opts)      => dispatch('browser.wait', { tabId: String(tabId), selector: (opts || {}).selector == null ? null : String((opts || {}).selector), text: (opts || {}).text == null ? null : String((opts || {}).text), urlContains: (opts || {}).urlContains == null ? null : String((opts || {}).urlContains), function: (opts || {}).function == null ? null : String((opts || {}).function), timeoutMs: Number((opts || {}).timeoutMs == null ? 5000 : (opts || {}).timeoutMs) }),
                fill:      (tabId, selector, text) => dispatch('browser.fill', { tabId: String(tabId), selector: String(selector), text: String(text) }),
                press:     (tabId, key, selector) => dispatch('browser.press', { tabId: String(tabId), key: String(key), selector: selector == null ? null : String(selector) }),
                select:    (tabId, selector, value) => dispatch('browser.select', { tabId: String(tabId), selector: String(selector), value: String(value) }),
                hover:     (tabId, selector)  => dispatch('browser.hover', { tabId: String(tabId), selector: String(selector) }),
                scrollIntoView: (tabId, selector) => dispatch('browser.scrollIntoView', { tabId: String(tabId), selector: String(selector) }),
                setChecked: (tabId, selector, checked) => dispatch('browser.setChecked', { tabId: String(tabId), selector: String(selector), checked: Boolean(checked) }),
                is:        (tabId, property, selector) => dispatch('browser.is', { tabId: String(tabId), property: String(property), selector: String(selector) }),
                getValue:  (tabId, selector)  => dispatch('browser.getValue', { tabId: String(tabId), selector: String(selector) }),
                getCount:  (tabId, selector)  => dispatch('browser.getCount', { tabId: String(tabId), selector: String(selector) }),
                find:      (tabId, kind, value) => mapResult(dispatch('browser.find', { tabId: String(tabId), kind: String(kind), value: String(value) }), parseJSON),
                snapshot:  (tabId, selector)  => mapResult(dispatch('browser.snapshot', { tabId: String(tabId), selector: selector == null ? null : String(selector) }), parseJSON),
                getText:   (tabId, selector)  => dispatch('browser.getText', { tabId: String(tabId), selector: String(selector) }),
                getHTML:   (tabId, selector)  => dispatch('browser.getHTML', { tabId: String(tabId), selector: selector == null ? null : String(selector) }),
                getAttribute: (tabId, selector, name) => dispatch('browser.getAttribute', { tabId: String(tabId), selector: String(selector), attribute: String(name) }),
                reload:    (tabId)            => dispatch('browser.reload', { tabId: String(tabId) }),
                back:      (tabId)            => dispatch('browser.back', { tabId: String(tabId) }),
                forward:   (tabId)            => dispatch('browser.forward', { tabId: String(tabId) }),
                waitForNavigation: (tabId, opts) => dispatch('browser.waitForNavigation', { tabId: String(tabId), timeoutMs: Number((opts || {}).timeoutMs == null ? 10000 : (opts || {}).timeoutMs) }),
                screenshot: (tabId)           => mapResult(dispatch('browser.screenshot', { tabId: String(tabId) }), r => (r || {}).png),
                storage: {
                    get:   (tabId, key, kind) => dispatch('browser.storage.get', { tabId: String(tabId), key: String(key), kind: kind || 'local' }),
                    set:   (tabId, key, value, kind) => dispatch('browser.storage.set', { tabId: String(tabId), key: String(key), value: String(value), kind: kind || 'local' }),
                    clear: (tabId, kind)      => dispatch('browser.storage.clear', { tabId: String(tabId), kind: kind || 'local' }),
                },
                cookies: {
                    get:    (tabId, url)      => dispatch('browser.cookies.get', { tabId: String(tabId), url: url == null ? null : String(url) }),
                    set:    (tabId, cookie)   => dispatch('browser.cookies.set', Object.assign({ tabId: String(tabId) }, cookie || {})),
                    delete: (tabId, name, domain) => dispatch('browser.cookies.delete', { tabId: String(tabId), name: String(name), domain: domain == null ? null : String(domain) }),
                    clear:  (tabId)           => dispatch('browser.cookies.clear', { tabId: String(tabId) }),
                },
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
                delete:   (identifier) => dispatch('projects.delete', { identifier: String(identifier) }),
                add:      (path)               => dispatch('projects.add', { path: String(path) }),
                rename:   (identifier, name)   => dispatch('projects.rename', { identifier: String(identifier), name: String(name) }),
                setColor: (identifier, color)  => dispatch('projects.setColor', { identifier: String(identifier), color: color == null ? null : String(color) }),
                setIcon:  (identifier, icon)   => dispatch('projects.setIcon', { identifier: String(identifier), icon: icon == null ? null : String(icon) }),
                setLogo:  (identifier, logo)   => dispatch('projects.setLogo', { identifier: String(identifier), logo: logo == null ? null : String(logo) }),
                reorder:  (identifiers)        => dispatch('projects.reorder', { identifiers: (identifiers || []).map(String) }),
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

    private static let backgroundTabsBlock = """
            muxy.tabs = {
                open: (request) => dispatch('tabs.open', request || {}),
            };
    """

    private static let gitBlock = """
            const gitProject = (o) => (o && o.project != null ? String(o.project) : null);
            muxy.git = {
                status:        (o) => dispatch('git.status', {
                    project: gitProject(o),
                    local: Boolean((o || {}).local),
                    fresh: Boolean((o || {}).fresh),
                }),
                diff:          (o) => dispatch('git.diff', {
                    project: gitProject(o),
                    filePath: String((o || {}).filePath || ''),
                    raw: Boolean((o || {}).raw),
                    staged: (o || {}).staged == null ? null : Boolean(o.staged),
                    lineLimit: (o || {}).lineLimit == null ? null : Number(o.lineLimit),
                    fresh: Boolean((o || {}).fresh),
                }),
                repoInfo:      (o) => dispatch('git.repoInfo', { project: gitProject(o) }),
                log:           (o) => dispatch('git.log', {
                    project: gitProject(o),
                    maxCount: (o || {}).maxCount == null ? null : Number(o.maxCount),
                    skip: (o || {}).skip == null ? null : Number(o.skip),
                    fresh: Boolean((o || {}).fresh),
                }),
                branches:      (o) => dispatch('git.branches', { project: gitProject(o) }),
                remoteBranches:(o) => dispatch('git.remoteBranches', { project: gitProject(o) }),
                currentBranch: (o) => dispatch('git.currentBranch', { project: gitProject(o) }),
                aheadBehind:   (o) => dispatch('git.aheadBehind', { project: gitProject(o), fresh: Boolean((o || {}).fresh) }),
                init:          (o) => dispatch('git.init', { project: gitProject(o) }),
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
                push:          (o) => dispatch('git.push', { project: gitProject(o), setUpstream: Boolean((o || {}).setUpstream) }),
                pull:          (o) => dispatch('git.pull', { project: gitProject(o) }),
                checkout:      (o) => dispatch('git.checkout', { project: gitProject(o), hash: String((o || {}).hash || '') }),
                cherryPick:    (o) => dispatch('git.cherryPick', { project: gitProject(o), hash: String((o || {}).hash || '') }),
                revert:        (o) => dispatch('git.revert', { project: gitProject(o), hash: String((o || {}).hash || '') }),
                branch: {
                    create: (o) => dispatch('git.branch.create', { project: gitProject(o), name: String((o || {}).name || '') }),
                    switchTo: (o) => dispatch('git.branch.switch', { project: gitProject(o), branch: String((o || {}).branch || '') }),
                    delete: (o) => dispatch('git.branch.delete', {
                        project: gitProject(o),
                        name: String((o || {}).name || ''),
                        force: Boolean((o || {}).force),
                    }),
                    deleteRemote: (o) => dispatch('git.branch.deleteRemote', { project: gitProject(o), branch: String((o || {}).branch || '') }),
                },
                tag: {
                    create: (o) => dispatch('git.tag.create', {
                        project: gitProject(o),
                        name: String((o || {}).name || ''),
                        hash: String((o || {}).hash || ''),
                    }),
                },
                pr: {
                    info:   (o) => dispatch('git.pr.info', { project: gitProject(o), fresh: Boolean((o || {}).fresh) }),
                    number: (o) => dispatch('git.pr.number', { project: gitProject(o), fresh: Boolean((o || {}).fresh) }),
                    diff:   (o) => dispatch('git.pr.diff', {
                        project: gitProject(o),
                        number: Number((o || {}).number),
                        lineLimit: (o || {}).lineLimit == null ? null : Number(o.lineLimit),
                        fresh: Boolean((o || {}).fresh),
                    }),
                    checkout: (o) => dispatch('git.pr.checkout', { project: gitProject(o), number: Number((o || {}).number) }),
                    checkoutWorktree: (o) => dispatch('git.pr.checkoutWorktree', {
                        project: gitProject(o),
                        path: String((o || {}).path || ''),
                        number: Number((o || {}).number),
                    }),
                    list:   (o) => dispatch('git.pr.list', {
                        project: gitProject(o),
                        filter: (o || {}).filter == null ? null : String(o.filter),
                        limit: (o || {}).limit == null ? null : Number(o.limit),
                        checks: (o || {}).checks == null ? null : Boolean(o.checks),
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
                    switchTo: (o) => dispatch('git.worktree.switch', { project: gitProject(o), identifier: String((o || {}).identifier || '') }),
                },
            };
    """

    private static let agentsBlock = """
            muxy.agents = {
                list: () => dispatch('agents.list', {}),
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
            const isExtensionLocalEvent = (name) => {
                const key = String(name);
                return key.startsWith('extension.') && key.length > 'extension.'.length;
            };
            const handlerStore = {};
            this.__muxyEventHandlers = handlerStore;
            muxy.events = {
                subscribe(name, handler) {
                    if (typeof handler !== 'function') return () => {};
                    const key = String(name);
                    if (!handlerStore[key]) {
                        handlerStore[key] = [];
                        if (!isExtensionLocalEvent(key)) __muxySubscribe(key);
                    }
                    handlerStore[key].push(handler);
                    return () => muxy.events.unsubscribe(key, handler);
                },
                unsubscribe(name, handler) {
                    const key = String(name);
                    const list = handlerStore[key];
                    if (!list) return;
                    const index = list.indexOf(handler);
                    if (index >= 0) list.splice(index, 1);
                },
                emit(name, payload) {
                    const key = String(name);
                    if (!isExtensionLocalEvent(key)) throw new Error('extension events must start with extension.');
                    return dispatch('events.emit', { event: key, payload: payload === undefined ? null : payload });
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
