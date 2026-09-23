((owner, makeAPI, options = {}) => {
  const dispatch = (verb, args = {}) => {
    const reply = JSON.parse(__muxyNative(JSON.stringify({ verb, args })));
    if (reply?.ok) return reply.value;
    throw new Error(reply?.error || 'extension API error');
  };
  const format = value => {
    if (value === null) return 'null';
    if (value === undefined) return 'undefined';
    if (typeof value === 'string') return value;
    if (value instanceof Error) return value.stack || value.message;
    try { return JSON.stringify(value); } catch (_) { return String(value); }
  };
  globalThis.console = Object.fromEntries([['log', 'log'], ['warn', 'warn'], ['error', 'err']].map(([method, level]) => [method, (...args) => {
    dispatch('console', { level, message: args.map(format).join(' ') });
  }]));
  globalThis.muxy = Object.freeze(makeAPI(dispatch, { owner, surface: options.surface || 'script' }));
  let next = 0;
  const timers = new Map();
  const schedule = (callback, delay, args, repeat) => {
    if (typeof callback !== 'function' || timers.size >= 256) throw new Error('invalid timer');
    const id = ++next;
    const interval = Math.max(0, Math.min(Number(delay) || 0, 86400000));
    timers.set(id, { callback, args, interval, repeat, time: Date.now() + interval });
    return id;
  };
  globalThis.setTimeout = (callback, delay = 0, ...args) => schedule(callback, delay, args, false);
  globalThis.setInterval = (callback, delay = 0, ...args) => schedule(callback, delay, args, true);
  globalThis.clearTimeout = id => { timers.delete(id); };
  globalThis.clearInterval = globalThis.clearTimeout;
  globalThis.setImmediate = callback => setTimeout(callback, 0);
  globalThis.clearImmediate = globalThis.clearTimeout;
  globalThis.queueMicrotask = callback => Promise.resolve().then(callback);
  // Runs due timers and answers the milliseconds until the next one, or -1
  // when only host input can wake the script.
  globalThis.__muxyTick = () => {
    if (!options.persistent && timers.size === 0 && !globalThis.__muxyHasCallbacks?.()) { dispatch('script.finished'); return -1; }
    const now = Date.now();
    for (const [id, timer] of [...timers]) {
      if (timer.time > now || !timers.has(id)) continue;
      if (timer.repeat) timer.time = now + Math.max(timer.interval, 1);
      else timers.delete(id);
      try { timer.callback(...timer.args); } catch (error) { console.error(error); }
    }
    let next = Infinity;
    for (const timer of timers.values()) next = Math.min(next, timer.time);
    return next === Infinity ? -1 : Math.max(0, next - Date.now());
  };
})
