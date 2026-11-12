((dispatch, initial) => {
  const surface = initial.surface || 'script';
  const background = surface === 'background';
  const page = surface === 'page';
  const text = value => String(value == null ? '' : value);
  const optional = value => (value == null ? null : String(value));
  const pick = (o, fields) => {
    const payload = {};
    for (const field of fields) if (o[field] != null) payload[field] = String(o[field]);
    return payload;
  };
  const muxy = { extensionID: initial.owner };
  const execPayload = (argvOrOptions, maybeOptions) => {
    const opts = Array.isArray(argvOrOptions) ? (maybeOptions || {}) : (argvOrOptions || {});
    const payload = Array.isArray(argvOrOptions) ? { argv: argvOrOptions.map(String) } : {};
    if (!Array.isArray(argvOrOptions)) {
      if (opts.shell != null) payload.shell = String(opts.shell);
      if (opts.argv) payload.argv = opts.argv.map(String);
    }
    if (opts.cwd != null) payload.cwd = String(opts.cwd);
    if (opts.env) payload.env = opts.env;
    if (opts.stdin != null) payload.stdin = String(opts.stdin);
    if (opts.timeoutMs != null) payload.timeoutMs = Number(opts.timeoutMs);
    return payload;
  };
  muxy.notifications = { notify: opts => dispatch('notifications.notify', opts || {}) };
  if (!background) muxy.toast = opts => dispatch('toast', opts || {});
  muxy.exec = (argvOrOptions, maybeOptions) => dispatch('exec', execPayload(argvOrOptions, maybeOptions));
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
  if (!background) {
    muxy.execAsync = (argvOrOptions, maybeOptions) => {
      const id = String(++sequence);
      const result = new Promise((resolve, reject) => jobs.set(id, { resolve, reject }));
      const fail = error => globalThis.__muxyResolveExec(id, { ok: false, error: error.message || String(error), cancelled: error.message === 'cancelled' });
      try {
        Promise.resolve(dispatch('exec.start', { ...execPayload(argvOrOptions, maybeOptions), id })).catch(fail);
      } catch (error) { fail(error); }
      return Object.freeze({ id, result, cancel: () => jobs.has(id) && dispatch('exec.cancel', { id }) });
    };
  }
  muxy.dialog = {
    confirm(opts) {
      const o = opts || {};
      const payload = pick(o, ['title', 'message', 'default', 'cancel', 'style']);
      if (Array.isArray(o.buttons)) payload.buttons = o.buttons.map(String);
      return dispatch('dialog.confirm', payload);
    },
    alert: opts => dispatch('dialog.alert', pick(opts || {}, ['title', 'message', 'style'])),
    prompt: opts => dispatch('dialog.prompt', pick(opts || {}, ['title', 'message', 'default', 'placeholder', 'confirm', 'cancel'])),
    pickFolder: opts => dispatch('dialog.pickFolder', pick(opts || {}, ['title', 'message', 'default'])),
  };
  muxy.storage = {
    get: key => dispatch('storage.get', { key: String(key) }),
    set: (key, value) => dispatch('storage.set', { key: String(key), value: value === undefined ? null : value }),
    delete: key => dispatch('storage.delete', { key: String(key) }),
    keys: () => dispatch('storage.keys', {}),
  };
  muxy.shortcuts = {
    register: opts => dispatch('shortcuts.register', { id: text((opts || {}).id), combo: text((opts || {}).combo) }),
    unregister: id => dispatch('shortcuts.unregister', { id: text(id) }),
    list: () => dispatch('shortcuts.list', {}),
  };
  const setItem = (verb, withText) => opts => {
    const o = opts || {};
    const payload = { id: text(o.id) };
    if (o.icon != null) payload.icon = o.icon;
    if (withText && 'text' in o) payload.text = o.text == null ? null : String(o.text);
    if ('visible' in o) payload.visible = !!o.visible;
    return dispatch(verb, payload);
  };
  const items = (verb, withText) => ({
    set: setItem(verb, withText),
    show: id => dispatch(verb, { id: text(id), visible: true }),
    hide: id => dispatch(verb, { id: text(id), visible: false }),
  });
  muxy.topbar = items('topbar.set', false);
  muxy.statusbar = items('statusbar.set', true);
  if (background) {
    muxy.tabs = { open: request => dispatch('tabs.open', request || {}) };
  } else {
    muxy.tabs = {
      list: o => dispatch('tabs.list', { project: optional((o || {}).project) }),
      switchTo: identifier => dispatch('tabs.switch', { identifier: String(identifier) }),
      new: o => dispatch('tabs.new', { beside: optional((o || {}).beside), side: optional((o || {}).side) }),
      next: () => dispatch('tabs.next', {}),
      previous: () => dispatch('tabs.previous', {}),
      open: request => dispatch('tabs.open', request || {}),
      split: (tab, beside, edge) => dispatch('tabs.split', { tabId: text(tab), besideId: text(beside), edge: optional(edge) }),
      close: (tab, o) => dispatch('tabs.close', { tabId: text(tab), scope: optional((o || {}).scope) }),
      rename: (tab, title) => dispatch('tabs.rename', { tabId: text(tab), title: optional(title) }),
      setColor: (tab, color) => dispatch('tabs.setColor', { tabId: text(tab), color: optional(color) }),
      pin: (tab, pinned) => dispatch('tabs.pin', { tabId: text(tab), pinned: pinned == null ? true : Boolean(pinned) }),
      move: (tab, o) => {
        const options = o || {};
        const payload = { tabId: text(tab) };
        if (options.index != null) payload.index = Number(options.index);
        if (options.project != null) payload.destination = String(options.project);
        return dispatch('tabs.move', payload);
      },
      moveToWindow: tab => dispatch('tabs.moveToWindow', { tabId: text(tab) }),
      dock: tab => dispatch('tabs.dock', { tabId: text(tab) }),
    };
    const tabId = (verb, extra) => (tabId, ...rest) => dispatch(verb, { tabId: String(tabId), ...extra(...rest) });
    muxy.browser = {
      open: (url, opts) => dispatch('browser.open', { url: optional(url), split: Boolean((opts || {}).split) }),
      navigate: tabId('browser.navigate', url => ({ url: String(url) })),
      list: () => dispatch('browser.list', {}),
      read: tabId('browser.read', () => ({})),
      close: tabId('browser.close', () => ({})),
    };
    if (!page) {
      for (const verb of ['eval', 'click', 'type', 'waitFor', 'wait', 'fill', 'press', 'select', 'hover', 'scrollIntoView', 'setChecked', 'is', 'getValue', 'getCount', 'find', 'snapshot', 'getText', 'getHTML', 'getAttribute', 'reload', 'back', 'forward', 'waitForNavigation', 'screenshot']) {
        muxy.browser[verb] = (tabId, ...args) => dispatch('browser.' + verb, { tabId: String(tabId), args });
      }
      muxy.browser.storage = Object.freeze(Object.fromEntries(['get', 'set', 'clear'].map(verb => [verb, (tabId, ...args) => dispatch('browser.storage.' + verb, { tabId: String(tabId), args })])));
      muxy.browser.cookies = Object.freeze(Object.fromEntries(['get', 'set', 'delete', 'clear'].map(verb => [verb, (tabId, ...args) => dispatch('browser.cookies.' + verb, { tabId: String(tabId), args })])));
    }
    muxy.panes = {
      list: () => dispatch('panes.list', {}),
      send: (paneID, value) => dispatch('panes.send', { paneID, text: String(value) }),
      sendKeys: (paneID, key) => dispatch('panes.sendKeys', { paneID, key: String(key) }),
      readScreen: (paneID, lines) => dispatch('panes.readScreen', { paneID, lines: lines == null ? 50 : Number(lines) }),
      close: paneID => dispatch('panes.close', { paneID }),
      rename: (paneID, title) => dispatch('panes.rename', { paneID, title: String(title) }),
      split: (paneID, direction) => dispatch('panes.split', { paneID, direction: optional(direction) }),
      focus: paneID => dispatch('panes.focus', { paneID }),
      zoom: (paneID, zoomed) => dispatch('panes.zoom', { paneID, zoomed: zoomed == null ? null : Boolean(zoomed) }),
      equalize: paneID => dispatch('panes.equalize', { paneID }),
      detach: paneID => dispatch('panes.detach', { paneID }),
      restart: (paneID, o) => dispatch('panes.restart', { paneID, proxy: (o || {}).proxy == null ? null : Boolean(o.proxy) }),
      scrollToBottom: paneID => dispatch('panes.scrollToBottom', { paneID }),
      float: (paneID, o) => dispatch('panes.float', { paneID, scope: optional((o || {}).scope) }),
      moveToTab: (paneID, o) => dispatch('panes.moveToTab', { paneID, destination: optional((o || {}).project) }),
      moveToWindow: paneID => dispatch('panes.moveToWindow', { paneID }),
    };
    muxy.projects = {
      list: () => dispatch('projects.list', {}),
      switchTo: identifier => dispatch('projects.switch', { identifier: String(identifier) }),
      delete: identifier => dispatch('projects.delete', { identifier: String(identifier) }),
      add: path => dispatch('projects.add', { path: String(path) }),
      create: (path, opts) => {
        const o = opts || {};
        const payload = { path: String(path), createIfMissing: Boolean(o.createIfMissing) };
        if (o.name != null) payload.name = String(o.name);
        if (o.workspace != null) payload.workspace = String(o.workspace);
        return dispatch('projects.create', payload);
      },
      attach: (identifier, workspace) => dispatch('projects.attach', { identifier: String(identifier), workspace: String(workspace) }),
      detach: identifier => dispatch('projects.detach', { identifier: String(identifier) }),
      rename: (identifier, name) => dispatch('projects.rename', { identifier: String(identifier), name: String(name) }),
      setColor: (identifier, color) => dispatch('projects.setColor', { identifier: String(identifier), color: optional(color) }),
      setIcon: (identifier, icon) => dispatch('projects.setIcon', { identifier: String(identifier), icon: optional(icon) }),
      setLogo: (identifier, logo) => dispatch('projects.setLogo', { identifier: String(identifier), logo: optional(logo) }),
      reorder: identifiers => dispatch('projects.reorder', { identifiers: (identifiers || []).map(String) }),
      next: () => dispatch('projects.next', {}),
      previous: () => dispatch('projects.previous', {}),
      focus: identifier => dispatch('projects.focus', { identifier: optional(identifier) }),
      sort: order => dispatch('projects.sort', { order: text(order) }),
      removed: () => dispatch('projects.removed', {}),
      restore: index => dispatch('projects.restore', { index: Number(index) }),
      apps: () => dispatch('projects.apps', {}),
      openIn: (identifier, app) => dispatch('projects.openIn', { identifier: text(identifier), app: optional(app) }),
      reveal: identifier => dispatch('projects.reveal', { identifier: text(identifier) }),
    };
    muxy.workspaces = {
      list: () => dispatch('workspaces.list', {}),
      create: name => dispatch('workspaces.create', { name: String(name) }),
      switchTo: identifier => dispatch('workspaces.switch', { identifier: String(identifier) }),
      rename: (identifier, name) => dispatch('workspaces.rename', { identifier: String(identifier), name: String(name) }),
      delete: identifier => dispatch('workspaces.delete', { identifier: String(identifier) }),
    };
    muxy.worktrees = {
      list: project => dispatch('worktrees.list', { project: optional(project) }),
      switchTo: (identifier, project) => dispatch('worktrees.switch', { identifier: String(identifier), project: optional(project) }),
      refresh: project => dispatch('worktrees.refresh', { project: optional(project) }),
    };
    const filesProject = o => (o && o.project != null ? String(o.project) : null);
    muxy.files = {
      list: (path, o) => dispatch('files.list', { project: filesProject(o), path: text(path) }),
      read: (path, o) => dispatch('files.read', { project: filesProject(o), path: text(path) }),
      stat: (path, o) => dispatch('files.stat', { project: filesProject(o), path: text(path) }),
      write: (path, contents, o) => dispatch('files.write', { project: filesProject(o), path: text(path), contents: text(contents) }),
      mkdir: (path, o) => dispatch('files.mkdir', { project: filesProject(o), path: text(path) }),
      rename: (path, newName, o) => dispatch('files.rename', { project: filesProject(o), path: text(path), newName: text(newName) }),
      move: (paths, into, o) => dispatch('files.move', { project: filesProject(o), paths: (paths || []).map(String), into: text(into) }),
      delete: (paths, o) => dispatch('files.delete', { project: filesProject(o), paths: (paths || []).map(String) }),
    };
  }
  const panelArgs = (panelID, extra) => {
    const payload = { panelID: text(panelID) };
    for (const [key, value] of Object.entries(extra)) if (value !== undefined) payload[key] = value;
    return payload;
  };
  const instanceOf = instance => (instance == null ? undefined : String(instance));
  muxy.panels = {
    open: (panelID, data, options) => dispatch('panels.open', panelArgs(panelID, { data, options: options ?? undefined })),
    toggle: (panelID, data, options) => dispatch('panels.toggle', panelArgs(panelID, { data, options: options ?? undefined })),
    close: (panelID, instance) => dispatch('panels.close', panelArgs(panelID, { instance: instanceOf(instance) })),
    update: (panelID, options, instance) => dispatch('panels.update', panelArgs(panelID, { options: options || {}, instance: instanceOf(instance) })),
    list: () => dispatch('panels.list', {}),
  };
  muxy.agents = {
    list: () => dispatch('agents.list', {}),
    focus: paneID => dispatch('agents.focus', { paneID: text(paneID) }),
  };
  muxy.app = {
    info: () => dispatch('app.info', {}),
    openExternal: url => dispatch('app.openExternal', { url: text(url) }),
  };
  const visibility = verb => visible => dispatch(verb, { visible: visible == null ? null : Boolean(visible) });
  muxy.ui = {
    state: () => dispatch('ui.state', {}),
    setSidebar: visibility('ui.setSidebar'),
    setAgentsPanel: visibility('ui.setAgentsPanel'),
    setAgentSessions: visibility('ui.setAgentSessions'),
    setLayout: layout => dispatch('ui.setLayout', { layout: text(layout) }),
    openCommandPalette: () => dispatch('ui.openCommandPalette', {}),
    openSettings: section => dispatch('ui.openSettings', { section: optional(section) }),
    back: () => dispatch('ui.back', {}),
    forward: () => dispatch('ui.forward', {}),
    toggleFloats: () => dispatch('ui.toggleFloats', {}),
    quickTerminal: Object.freeze({
      show: () => dispatch('ui.quickTerminal', { action: 'show' }),
      hide: () => dispatch('ui.quickTerminal', { action: 'hide' }),
      toggle: () => dispatch('ui.quickTerminal', { action: 'toggle' }),
      pin: pinned => dispatch('ui.quickTerminal', { action: 'pin', pinned: pinned == null ? true : Boolean(pinned) }),
    }),
  };
  muxy.commands = {
    list: () => dispatch('commands.list', {}),
    run: id => dispatch('commands.run', { id: text(id) }),
  };
  muxy.composer = {
    open: () => dispatch('composer.open', {}),
    close: () => dispatch('composer.close', {}),
    getDraft: () => dispatch('composer.getDraft', {}),
    setDraft: value => dispatch('composer.setDraft', { text: text(value) }),
    submit: () => dispatch('composer.submit', {}),
    insert: () => dispatch('composer.insert', {}),
  };
  muxy.layouts = {
    list: o => dispatch('layouts.list', { project: optional((o || {}).project) }),
    apply: (name, o) => dispatch('layouts.apply', { name: text(name), project: optional((o || {}).project) }),
  };
  muxy.preferences = {
    get: () => dispatch('preferences.get', {}),
    themes: () => dispatch('preferences.themes', {}),
    set: (key, value) => dispatch('preferences.set', { key: text(key), value: value === undefined ? null : value }),
  };
  muxy.settings = {
    get: key => dispatch('settings.get', { key: text(key) }),
    all: () => dispatch('settings.all', {}),
    set: (key, value) => dispatch('settings.set', { key: text(key), value: value === undefined ? null : value }),
    reset: key => dispatch('settings.reset', { key: text(key) }),
  };
  muxy.clipboard = {
    readText: () => dispatch('clipboard.readText', {}),
    writeText: value => dispatch('clipboard.writeText', { text: text(value) }),
  };
  muxy.gh = { user: () => dispatch('gh.user', {}) };
  const gitProject = o => (o && o.project != null ? String(o.project) : null);
  const git = (verb, build = () => ({})) => o => dispatch('git.' + verb, { project: gitProject(o), ...build(o || {}) });
  const fresh = o => ({ fresh: Boolean(o.fresh) });
  muxy.git = {
    status: git('status', o => ({ local: Boolean(o.local), fresh: Boolean(o.fresh) })),
    diff: git('diff', o => ({ filePath: String(o.filePath || ''), raw: Boolean(o.raw), staged: o.staged == null ? null : Boolean(o.staged), lineLimit: o.lineLimit == null ? null : Number(o.lineLimit), fresh: Boolean(o.fresh) })),
    repoInfo: git('repoInfo'),
    log: git('log', o => ({ maxCount: o.maxCount == null ? null : Number(o.maxCount), skip: o.skip == null ? null : Number(o.skip), fresh: Boolean(o.fresh) })),
    branches: git('branches'),
    remoteBranches: git('remoteBranches'),
    currentBranch: git('currentBranch'),
    aheadBehind: git('aheadBehind', fresh),
    init: git('init'),
    worktrees: git('worktrees'),
    stage: git('stage', o => ({ paths: (o.paths || []).map(String) })),
    unstage: git('unstage', o => ({ paths: (o.paths || []).map(String) })),
    discard: git('discard', o => ({ paths: (o.paths || []).map(String), untrackedPaths: (o.untrackedPaths || []).map(String) })),
    commit: git('commit', o => ({ message: String(o.message || ''), stageAll: Boolean(o.stageAll) })),
    push: git('push', o => ({ setUpstream: Boolean(o.setUpstream) })),
    pull: git('pull'),
    checkout: git('checkout', o => ({ hash: String(o.hash || '') })),
    cherryPick: git('cherryPick', o => ({ hash: String(o.hash || '') })),
    revert: git('revert', o => ({ hash: String(o.hash || '') })),
    branch: {
      create: git('branch.create', o => ({ name: String(o.name || '') })),
      switchTo: git('branch.switch', o => ({ branch: String(o.branch || '') })),
      delete: git('branch.delete', o => ({ name: String(o.name || ''), force: Boolean(o.force) })),
      deleteRemote: git('branch.deleteRemote', o => ({ branch: String(o.branch || '') })),
    },
    tag: { create: git('tag.create', o => ({ name: String(o.name || ''), hash: String(o.hash || '') })) },
    pr: {
      info: git('pr.info', fresh),
      number: git('pr.number', fresh),
      diff: git('pr.diff', o => ({ number: Number(o.number), lineLimit: o.lineLimit == null ? null : Number(o.lineLimit), fresh: Boolean(o.fresh) })),
      checkout: git('pr.checkout', o => ({ number: Number(o.number) })),
      checkoutWorktree: git('pr.checkoutWorktree', o => ({ path: String(o.path || ''), number: Number(o.number) })),
      list: git('pr.list', o => ({ filter: optional(o.filter), limit: o.limit == null ? null : Number(o.limit), checks: o.checks == null ? null : Boolean(o.checks) })),
      create: git('pr.create', o => ({ title: String(o.title || ''), body: String(o.body || ''), baseBranch: optional(o.baseBranch), draft: Boolean(o.draft) })),
      merge: git('pr.merge', o => ({ number: Number(o.number), method: optional(o.method), deleteBranch: o.deleteBranch == null ? true : Boolean(o.deleteBranch) })),
      close: git('pr.close', o => ({ number: Number(o.number) })),
    },
    worktree: {
      add: git('worktree.add', o => ({ path: String(o.path || ''), branch: String(o.branch || ''), createBranch: Boolean(o.createBranch), baseBranch: optional(o.baseBranch) })),
      remove: git('worktree.remove', o => ({ path: String(o.path || ''), force: Boolean(o.force), timeoutMs: o.timeoutMs == null ? null : Number(o.timeoutMs) })),
      switchTo: git('worktree.switch', o => ({ identifier: String(o.identifier || '') })),
    },
  };
  const normalizeModalItems = raw => (Array.isArray(raw) ? raw : (raw && raw.items) || [])
    .map(it => (it && it.id != null && it.title != null
      ? { id: String(it.id), title: String(it.title), subtitle: it.subtitle == null ? null : String(it.subtitle) }
      : null))
    .filter(Boolean);
  const modalLabels = o => {
    const labels = pick(o, ['placeholder', 'emptyLabel', 'noMatchLabel']);
    if ('searchToolbar' in o) labels.searchToolbar = !!o.searchToolbar;
    if (typeof o.onQuery === 'function' || typeof o.onQueryChange === 'function') labels.dynamic = true;
    return labels;
  };
  const modalResultHandlers = {};
  const modalWebviewResultHandlers = {};
  const modalQueryHandlers = {};
  let activeModalQueryID = null;
  globalThis.__muxiDeliverModalResult = (requestID, item) => {
    const handlers = String(requestID).indexOf('webview:') === 0 ? modalWebviewResultHandlers : modalResultHandlers;
    const handler = handlers[requestID];
    delete handlers[requestID];
    delete modalQueryHandlers[requestID];
    if (typeof handler === 'function') {
      try { handler(item == null ? null : item); } catch (error) { console.error(error); }
    }
  };
  globalThis.__muxyDeliverModalQuery = (requestID, queryID, query, options) => {
    const emit = batch => dispatch('modal.feed', { items: normalizeModalItems(batch), queryID });
    const finish = () => dispatch('modal.finish', { queryID });
    try {
      const handler = modalQueryHandlers[requestID];
      if (typeof handler !== 'function') { finish(); return; }
      let produced;
      const previous = activeModalQueryID;
      activeModalQueryID = queryID;
      try { produced = handler(query, emit, options || {}); } finally { activeModalQueryID = previous; }
      const done = value => { if (value != null) emit(value); finish(); };
      if (produced && typeof produced.then === 'function') {
        produced.then(value => { try { done(value); } catch (error) { console.error(error); finish(); } }, error => { console.error(error); finish(); });
        return;
      }
      done(produced);
    } catch (error) {
      try { console.error(error); finish(); } catch (_) {}
    }
  };
  muxy.modal = {
    open(opts) {
      const o = opts || {};
      const opened = dispatch('modal.open', modalLabels(o));
      const requestID = opened && opened.requestID;
      if (requestID != null) {
        if (typeof o.onSelect === 'function') modalResultHandlers[requestID] = o.onSelect;
        if (typeof o.onQuery === 'function') modalQueryHandlers[requestID] = o.onQuery;
        else if (typeof o.onQueryChange === 'function') modalQueryHandlers[requestID] = (query, emit, options) => o.onQueryChange(query, options || {});
      }
      const emit = batch => dispatch('modal.feed', { items: normalizeModalItems(batch) });
      if (typeof o.items === 'function') {
        const produced = o.items(emit);
        if (produced != null) emit(produced);
      } else {
        emit(o.items);
      }
      dispatch('modal.finish', {});
      return requestID;
    },
    feed(batch) {
      const payload = { items: normalizeModalItems(batch) };
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
      const payload = { entry: text(o.entry) };
      if (o.width != null) payload.width = Number(o.width);
      if (o.height != null) payload.height = Number(o.height);
      if (o.dismissOnOutsideClick != null) payload.dismissOnOutsideClick = !!o.dismissOnOutsideClick;
      if (o.data !== undefined) payload.data = o.data == null ? null : o.data;
      const opened = dispatch('modal.openWebview', payload);
      const requestID = opened && opened.requestID;
      return new Promise(resolve => {
        if (requestID == null) { resolve(null); return; }
        modalWebviewResultHandlers[requestID] = resolve;
      });
    },
    closeWebview: () => dispatch('modal.closeWebview', {}),
  };
  if (background) {
    const handlers = new Map();
    globalThis.__muxyEvent = (name, payload) => {
      for (const handler of [...(handlers.get(name) || [])]) {
        try { handler(payload); } catch (error) { console.error(error); }
      }
    };
    muxy.events = {
      subscribe(name, handler) {
        if (typeof handler !== 'function') return () => {};
        const key = String(name);
        if (!handlers.has(key)) {
          handlers.set(key, []);
          try { dispatch('events.subscribe', { event: key }); } catch (error) { console.error(error); }
        }
        handlers.get(key).push(handler);
        return () => muxy.events.unsubscribe(key, handler);
      },
      unsubscribe(name, handler) {
        const list = handlers.get(String(name));
        const index = list ? list.indexOf(handler) : -1;
        if (index >= 0) list.splice(index, 1);
      },
      emit(name, payload) {
        const key = String(name);
        if (!key.startsWith('extension.') || key.length <= 'extension.'.length) throw new Error('extension events must start with extension.');
        return dispatch('events.emit', { event: key, payload: payload === undefined ? null : payload });
      },
    };
    const remote = {};
    muxy.remote = {
      handle(action, handler) { remote[String(action)] = handler; },
      unhandle(action) { delete remote[String(action)]; },
    };
  }
  globalThis.__muxyHasCallbacks = () => jobs.size > 0 || Object.keys(modalResultHandlers).length > 0 || Object.keys(modalQueryHandlers).length > 0 || Object.keys(modalWebviewResultHandlers).length > 0;
  for (const value of Object.values(muxy)) if (value && typeof value === 'object') Object.freeze(value);
  for (const value of [muxy.git.branch, muxy.git.tag, muxy.git.pr, muxy.git.worktree]) Object.freeze(value);
  return muxy;
})
