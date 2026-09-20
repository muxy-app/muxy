((owner, makeAPI) => {
  const dispatch = (verb, args = {}) => {
    const reply = JSON.parse(__muxyNative(JSON.stringify({ verb, args })));
    if (reply?.ok) return reply.value;
    throw new Error(reply?.error || 'extension API error');
  };
  globalThis.console = Object.fromEntries(['log', 'warn', 'error'].map(level => [level, (...args) => {
    dispatch('console', { level, message: args.map(value => String(value)).join(' ') });
  }]));
  globalThis.muxy = Object.freeze(makeAPI(dispatch, { owner }));
  let next = 0;
  const timers = new Map();
  globalThis.setTimeout = (callback, delay = 0, ...args) => {
    if (typeof callback !== 'function' || timers.size >= 256) throw new Error('invalid timer');
    const id = ++next;
    timers.set(id, { callback, args, time: Date.now() + Math.max(0, Math.min(Number(delay) || 0, 300000)) });
    return id;
  };
  globalThis.clearTimeout = id => timers.delete(id);
  globalThis.setImmediate = callback => setTimeout(callback, 0);
  globalThis.clearImmediate = globalThis.clearTimeout;
  globalThis.queueMicrotask = callback => Promise.resolve().then(callback);
  globalThis.__muxyTick = () => {
    if (timers.size === 0 && !globalThis.__muxyHasCallbacks?.()) { dispatch("script.finished"); return; }
    const now = Date.now();
    for (const [id, timer] of [...timers]) {
      if (timer.time > now || !timers.delete(id)) continue;
      try { timer.callback(...timer.args); } catch (error) { console.error(error); }
    }
  };
})
