((initial) => {
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
  window.muxy = Object.freeze({
    extensionID: initial.owner,
    tabInstanceID: initial.id,
    panelID: initial.surface === "panel" ? initial.id : null,
    get data() { return data; },
    get theme() { return theme; },
    get focused() { return focused; },
    onDataChange: callback => subscribe(listeners.data, callback),
    onThemeChange: callback => subscribe(listeners.theme, callback),
    onFocus: callback => subscribe(listeners.focus, callback),
    tabs: {
      open: request => send('tabs.open', request),
      setTitle: title => send('tabs.setTitle', { tabInstanceID: initial.id, title: String(title ?? '') }),
      setIcon: icon => send('tabs.setIcon', { tabInstanceID: initial.id, icon: icon ?? null }),
    },
    panels: {
      open: (panelID, data) => send('panels.open', { panelID, ...(data === undefined ? {} : { data }) }),
      toggle: (panelID, data) => send('panels.toggle', { panelID, ...(data === undefined ? {} : { data }) }),
      close: (panelID = initial.id) => send('panels.close', { panelID }),
    },
    modal: {
      openWebview: async options => {
        const { requestID } = await send('modal.openWebview', options);
        return send('modal.awaitWebview', { requestID });
      },
      submitWebview: result => send('modal.submitWebview', { requestID: initial.id, result: result ?? null }),
      closeWebview: () => send('modal.closeWebview'),
    },
    lifecycle: {
      onBeforeClose: callback => {
        beforeClose = typeof callback === 'function' ? callback : null;
        return () => { if (beforeClose === callback) beforeClose = null; };
      },
      close: () => send('lifecycle.closeSelf'),
    },
  });
  document.addEventListener('pointerdown', () => send('surface.focus').catch(() => {}), true);
  if (!paintTheme()) {
    const observer = new MutationObserver(() => {
      if (paintTheme()) observer.disconnect();
    });
    observer.observe(document, { childList: true });
  }
})
