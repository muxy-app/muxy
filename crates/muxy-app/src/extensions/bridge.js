((dispatch, initial) => {
  const muxy = { extensionID: initial.owner };
  const execPayload = (argv, options) => Array.isArray(argv) ? { ...(options || {}), argv: argv.map(String) } : { ...(argv || {}) };
  muxy.exec = (argv, options) => dispatch('exec', execPayload(argv, options));
  let sequence = 0;
  const jobs = new Map();
  globalThis.__muxyResolveExec = (id, reply) => {
    const job = jobs.get(id);
    if (!job) return;
    jobs.delete(id);
    if (reply.ok) job.resolve(reply.value);
    else {
      const error = new Error(reply.error || 'exec failed');
      error.cancelled = !!reply.cancelled;
      error.code = error.cancelled ? 'cancelled' : 'error';
      job.reject(error);
    }
  };
  muxy.execAsync = (argv, options) => {
    const id = String(++sequence);
    const result = new Promise((resolve, reject) => jobs.set(id, { resolve, reject }));
    try {
      Promise.resolve(dispatch('exec.start', { ...execPayload(argv, options), id })).catch(error => {
        globalThis.__muxyResolveExec(id, { ok: false, error: error.message || String(error), cancelled: error.message === "cancelled" });
      });
    } catch (error) { globalThis.__muxyResolveExec(id, { ok: false, error: error.message || String(error), cancelled: error.message === "cancelled" }); }
    return Object.freeze({ id, result, cancel: () => jobs.has(id) && dispatch('exec.cancel', { id }) });
  };
  muxy.dialog = Object.fromEntries(['confirm','alert','prompt','pickFolder'].map(method => [method, options => dispatch('dialog.' + method, options || {})]));
  muxy.storage = {
    get: key => dispatch('storage.get', { key: String(key) }),
    set: (key, value) => dispatch('storage.set', { key: String(key), value: value ?? null }),
    delete: key => dispatch('storage.delete', { key: String(key) }),
    keys: () => dispatch('storage.keys'),
  };
  muxy.tabs = {
    list: () => dispatch('tabs.list'),
    switchTo: identifier => dispatch('tabs.switch', { identifier: String(identifier) }),
    open: request => dispatch('tabs.open', request || {}),
  };
  muxy.worktrees = {
    list: project => dispatch('worktrees.list', { project: project ?? null }),
    refresh: project => dispatch('worktrees.refresh', { project: project ?? null }),
    switchTo: (identifier, project) => dispatch('worktrees.switch', { identifier: String(identifier), project: project ?? null }),
  };
  muxy.toast = options => dispatch('toast', options || {});
  muxy.notifications = { notify: options => dispatch('notifications.notify', options || {}) };
  muxy.panels = Object.fromEntries(['open','toggle','close'].map(method => [method, (panelID, data) => dispatch('panels.' + method, { panelID, ...(data === undefined ? {} : { data }) })]));
  const events = new Map();
  globalThis.__muxyEvent = (name, payload) => {
    for (const handler of [...(events.get(name) || [])]) { try { handler(payload); } catch (error) { console.error(error); } }
  };
  muxy.events = {
    subscribe(name, handler) {
      if (typeof handler !== 'function') return () => {};
      name = String(name);
      if (!events.has(name)) { events.set(name, new Set()); Promise.resolve(dispatch('events.subscribe', { event: name })).catch(error => console.error(error)); }
      events.get(name).add(handler);
      return () => muxy.events.unsubscribe(name, handler);
    },
    unsubscribe(name, handler) { events.get(String(name))?.delete(handler); },
    emit: (name, payload) => dispatch('events.emit', { event: String(name), payload: payload ?? null }),
  };
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
            timeoutMs: (o || {}).timeoutMs == null ? null : Number(o.timeoutMs),
        }),
        switchTo: (o) => dispatch('git.worktree.switch', { project: gitProject(o), identifier: String((o || {}).identifier || '') }),
    },
};
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
const modalWebviewResultHandlers = {};
const modalQueryHandlers = {};
let activeModalQueryID = null;
const webviewModalPrefix = 'webview:';
globalThis.__muxiDeliverModalResult = (requestID, item) => {
    if (String(requestID).indexOf(webviewModalPrefix) === 0) {
        const webviewHandler = modalWebviewResultHandlers[requestID];
        delete modalWebviewResultHandlers[requestID];
        if (typeof webviewHandler === 'function') {
            try { webviewHandler(item == null ? null : item); } catch (error) { console.error(error); }
        }
        return;
    }
    const handler = modalResultHandlers[requestID];
    delete modalResultHandlers[requestID];
    delete modalQueryHandlers[requestID];
    if (typeof handler === 'function') {
        try { handler(item == null ? null : item); } catch (error) { console.error(error); }
    }
};
globalThis.__muxyDeliverModalQuery = (requestID, queryID, query, options) => {
    try {
        const handler = modalQueryHandlers[requestID];
        const emit = (batch) => dispatch('modal.feed', { items: normalizeModalItems(batch), queryID });
        const finish = () => dispatch('modal.finish', { queryID });
        const finishProduced = (produced) => {
            if (produced != null) emit(produced);
            finish();
        };
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
        if (produced && typeof produced.then === 'function') {
            produced.then(
                (value) => {
                    try { finishProduced(value); } catch (error) { console.error(error); finish(); }
                },
                (error) => { console.error(error); finish(); }
            );
            return;
        }
        finishProduced(produced);
    } catch (error) {
        try { console.error(error); } catch (ignored) {}
    }
};

  muxy.modal = {
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
    openWebview(opts) {
        const o = opts || {};
        const payload = { entry: String(o.entry == null ? '' : o.entry) };
        if (o.width != null) payload.width = Number(o.width);
        if (o.height != null) payload.height = Number(o.height);
        if (o.dismissOnOutsideClick != null) payload.dismissOnOutsideClick = !!o.dismissOnOutsideClick;
        if (o.data !== undefined) payload.data = o.data == null ? null : o.data;
        const opened = dispatch('modal.openWebview', payload);
        const requestID = opened && opened.requestID;
        return new Promise((resolve) => {
            if (requestID == null) { resolve(null); return; }
            modalWebviewResultHandlers[requestID] = resolve;
        });
    },
    closeWebview() { return dispatch('modal.closeWebview', {}); },
};
  globalThis.__muxyHasCallbacks = () => jobs.size > 0 || Object.keys(modalResultHandlers).length > 0 || Object.keys(modalQueryHandlers).length > 0 || Object.keys(modalWebviewResultHandlers).length > 0;
  return muxy;
})
