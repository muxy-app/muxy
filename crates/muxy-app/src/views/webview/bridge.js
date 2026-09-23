((initial, makeAPI) => {
  const handler = window.webkit?.messageHandlers?.muxy;
  if (!handler) return;
  let nextID = 1;
  const send = async (verb, args = {}) => {
    const reply = await handler.postMessage({ verb, args, requestID: String(nextID++) });
    if (reply?.ok) return reply.value;
    throw new Error(reply?.error || 'webview API unavailable');
  };
  const listeners = { data: new Set(), theme: new Set(), focus: new Set() };
  let data = initial.data, theme = initial.theme, focused = false;
  const subscribe = (set, callback) => {
    if (typeof callback !== 'function') return () => {};
    set.add(callback);
    return () => set.delete(callback);
  };
  const emit = (set, value) => { for (const listener of set) { try { listener(value); } catch (_) {} } };
  const paintTheme = () => {
    const root = document.documentElement;
    if (!root) return false;
    for (const [key, value] of Object.entries(theme)) {
      root.style.setProperty('--muxy-' + key.replace(/[A-Z]/g, m => '-' + m.toLowerCase()), value);
    }
    root.style.colorScheme = theme.colorScheme || 'light';
    return true;
  };
  window.__muxyApplyData = value => { data = value ?? null; emit(listeners.data, data); };
  window.__muxyApplyTheme = value => { theme = Object.freeze({ ...value }); paintTheme(); emit(listeners.theme, theme); };
  window.__muxyApplyFocus = value => {
    value = !!value;
    if (focused === value) return;
    focused = value;
    emit(listeners.focus, focused);
  };
  let beforeClose = null;
  window.__muxyBeforeClose = (callID, surface, instanceID) => {
    const resolve = prevent => send('lifecycle.resolveBeforeClose', { callID: String(callID), prevent }).catch(() => {});
    if (!beforeClose) { resolve(false); return; }
    send('lifecycle.ackBeforeClose', { callID: String(callID) }).catch(() => {});
    try {
      Promise.resolve(beforeClose({ surface, instanceID })).then(
        value => resolve(value === true || value?.prevent === true), () => resolve(false));
    } catch (_) { resolve(false); }
  };
  const eventListeners = new Map();
  window.__muxyEvent = (name, payload) => {
    for (const callback of [...(eventListeners.get(name) || [])]) {
      try { callback(payload || {}); } catch (_) {}
    }
  };
  const events = {
    subscribe(name, callback) {
      if (typeof name !== 'string' || typeof callback !== 'function') return () => {};
      let set = eventListeners.get(name);
      if (!set) {
        set = new Set();
        eventListeners.set(name, set);
        send('events.subscribe', { event: name }).catch(error => {
          eventListeners.delete(name);
          try { console.error('muxy.events.subscribe failed:', error.message || error); } catch (_) {}
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
      if (!key.startsWith('extension.') || key.length <= 'extension.'.length) {
        return Promise.reject(new Error('extension events must start with extension.'));
      }
      return send('events.emit', { event: key, payload: payload === undefined ? null : payload });
    },
  };
  const normalizeModalItems = raw => (Array.isArray(raw) ? raw : (raw && raw.items) || [])
    .map(it => (it && it.id != null && it.title != null
      ? { id: String(it.id), title: String(it.title), subtitle: it.subtitle == null ? null : String(it.subtitle) }
      : null))
    .filter(Boolean);
  const modalQueryHandlers = new Map();
  let activeModalQueryID = null;
  const deliverModalQuery = async (requestID, queryID, query, options) => {
    const queryHandler = modalQueryHandlers.get(String(requestID));
    const feed = batch => send('modal.feed', { items: normalizeModalItems(batch), queryID });
    try {
      if (typeof queryHandler === 'function') {
        const previous = activeModalQueryID;
        activeModalQueryID = queryID;
        let produced;
        try { produced = await queryHandler(query, feed, options || {}); } finally { activeModalQueryID = previous; }
        if (produced != null) await feed(produced);
      }
    } catch (error) {
      console.error(error);
    }
    await send('modal.finish', { queryID });
  };
  const modal = {
    async open(opts) {
      const o = opts || {};
      const labels = {};
      for (const field of ['placeholder', 'emptyLabel', 'noMatchLabel']) if (o[field] != null) labels[field] = String(o[field]);
      if (typeof o.onQuery === 'function' || typeof o.onQueryChange === 'function') labels.dynamic = true;
      const opened = await send('modal.open', labels);
      const requestID = opened && opened.requestID;
      if (requestID != null) {
        if (typeof o.onQuery === 'function') modalQueryHandlers.set(String(requestID), o.onQuery);
        else if (typeof o.onQueryChange === 'function') modalQueryHandlers.set(String(requestID), (query, feed, options) => o.onQueryChange(query, options || {}));
      }
      const feed = batch => send('modal.feed', { items: normalizeModalItems(batch) });
      try {
        if (typeof o.items === 'function') {
          const produced = await o.items(feed);
          if (produced != null) await feed(produced);
        } else {
          await feed(o.items);
        }
        await send('modal.finish', {});
        const choice = await send('modal.await', { requestID });
        if (typeof o.onSelect === 'function') o.onSelect(choice);
        return choice;
      } finally {
        if (requestID != null) modalQueryHandlers.delete(String(requestID));
      }
    },
    feed(items) {
      const payload = { items: normalizeModalItems(items) };
      if (activeModalQueryID != null) payload.queryID = activeModalQueryID;
      return send('modal.feed', payload);
    },
    finish() {
      const payload = {};
      if (activeModalQueryID != null) payload.queryID = activeModalQueryID;
      return send('modal.finish', payload);
    },
    openWebview: async options => {
      const { requestID } = await send('modal.openWebview', options || {});
      return send('modal.awaitWebview', { requestID });
    },
    submitWebview: result => send('modal.submitWebview', { requestID: initial.id, result: result ?? null }),
    closeWebview: () => send('modal.closeWebview'),
  };
  const api = makeAPI(send, { ...initial, surface: 'page' });
  window.__muxyDeliverModalQuery = deliverModalQuery;
  window.muxy = Object.freeze({
    ...api,
    extensionID: initial.owner,
    tabInstanceID: initial.id,
    panelID: initial.surface === 'panel' ? initial.id : null,
    get data() { return data; },
    get theme() { return theme; },
    get focused() { return focused; },
    onDataChange: callback => subscribe(listeners.data, callback),
    onThemeChange: callback => subscribe(listeners.theme, callback),
    onFocus: callback => subscribe(listeners.focus, callback),
    tabs: Object.freeze({
      ...api.tabs,
      setTitle: title => send('tabs.setTitle', { tabInstanceID: initial.id, title: String(title ?? '') }),
      setIcon: icon => send('tabs.setIcon', { tabInstanceID: initial.id, icon: icon ?? null }),
    }),
    panels: Object.freeze({
      open: (panelID, data) => send('panels.open', { panelID, ...(data === undefined ? {} : { data }) }),
      toggle: (panelID, data) => send('panels.toggle', { panelID, ...(data === undefined ? {} : { data }) }),
      close: (panelID = initial.id) => send('panels.close', { panelID }),
    }),
    popover: Object.freeze({
      close: () => send('popover.close', {}),
      resize: (width, height) => send('popover.resize', { width: Number(width), height: Number(height) }),
    }),
    http: Object.freeze({
      fetch(url, options) {
        const opts = options || {};
        const payload = { url: String(url) };
        if (opts.method != null) payload.method = String(opts.method);
        if (opts.headers) payload.headers = opts.headers;
        if (opts.body != null) payload.body = String(opts.body);
        if (opts.timeoutMs != null) payload.timeoutMs = Number(opts.timeoutMs);
        return send('http.fetch', payload);
      },
    }),
    events: Object.freeze(events),
    modal: Object.freeze(modal),
    lifecycle: Object.freeze({
      onBeforeClose: callback => {
        beforeClose = typeof callback === 'function' ? callback : null;
        return () => { if (beforeClose === callback) beforeClose = null; };
      },
      close: () => send('lifecycle.closeSelf'),
    }),
  });
  const format = value => {
    if (value === null) return 'null';
    if (value === undefined) return 'undefined';
    if (typeof value === 'string') return value;
    if (value instanceof Error) return value.stack || value.message;
    try { return JSON.stringify(value); } catch (_) { return String(value); }
  };
  let lines = [];
  const flush = () => {
    const batch = lines;
    lines = [];
    send('console', { lines: batch }).catch(() => {});
  };
  const log = (level, values) => {
    const message = Array.prototype.map.call(values, format).join(' ');
    if (message && lines.push({ level, message }) === 1) setTimeout(flush, 0);
  };
  if (window.console) {
    for (const [method, level] of [['log', 'log'], ['warn', 'warn'], ['error', 'err']]) {
      const original = window.console[method];
      window.console[method] = function () {
        log(level, arguments);
        if (original) { try { original.apply(window.console, arguments); } catch (_) {} }
      };
    }
  }
  window.addEventListener?.('error', event => log('err', [event.error ?? event.message ?? 'unknown error']));
  window.addEventListener?.('unhandledrejection', event => log('err', [event.reason ?? 'unhandledrejection']));
  document.addEventListener('pointerdown', () => send('surface.focus').catch(() => {}), true);
  if (!paintTheme()) {
    const observer = new MutationObserver(() => {
      if (paintTheme()) observer.disconnect();
    });
    observer.observe(document, { childList: true });
  }
})
