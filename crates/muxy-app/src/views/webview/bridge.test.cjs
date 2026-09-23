const { test } = require('node:test');
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const vm = require('node:vm');
const source = readFileSync(`${__dirname}/bridge.js`, 'utf8');
const apiSource = readFileSync(`${__dirname}/../../extensions/bridge.js`, 'utf8');

function page(reply = () => null, { rootPresent = true, surface = "tab", console = false } = {}) {
  const messages = [], css = new Map(), events = new Map(), observers = [];
  const authoredStyle = { tagName: 'STYLE', textContent: 'html { background: pink; }' };
  const root = {
    style: { setProperty: (k, v) => css.set(k, v) },
    children: [authoredStyle],
  };
  const document = {
    documentElement: rootPresent ? root : null,
    addEventListener: (name, handler) => events.set(name, handler),
  };
  class MutationObserver {
    constructor(callback) { this.callback = callback; this.active = false; observers.push(this); }
    observe(target, options) { this.target = target; this.options = options; this.active = true; }
    disconnect() { this.active = false; }
  }
  const attachRoot = () => {
    document.documentElement = root;
    for (const observer of observers) if (observer.active) observer.callback();
  };
  const context = vm.createContext({ document, MutationObserver, setTimeout });
  context.window = context;
  if (console) context.console = { log() {}, warn() {}, error() {} };
  context.webkit = { messageHandlers: { muxy: { postMessage: async message => {
    messages.push(JSON.parse(JSON.stringify(message)));
    return { ok: true, value: await reply(message) };
  } } } };
  vm.runInContext(`${source}(${JSON.stringify({ owner: 'test', id: 'instance', surface, data: { saved: true }, theme: { background: '#123456', foregroundMuted: '#888888', colorScheme: 'dark', topbarHeight: '34px' } })}, ${apiSource});`, context);
  return { context, muxy: context.muxy, messages, css, events, root, authoredStyle, observers, attachRoot };
}
const settled = () => new Promise(resolve => setImmediate(resolve));

test('identity, initial theme/data, listener disposal and focus deduplication', () => {
  const { context, muxy, css } = page();
  assert.equal(muxy.extensionID, 'test');
  assert.equal(muxy.tabInstanceID, 'instance');
  assert.equal(muxy.data.saved, true);
  assert.equal(css.get('--muxy-foreground-muted'), '#888888');
  assert.equal(css.get('--muxy-topbar-height'), '34px');
  assert.equal(context.document.documentElement.style.colorScheme, 'dark');
  const changes = [], focus = [];
  const unsubscribe = muxy.onDataChange(value => changes.push(value));
  muxy.onDataChange(() => { throw Error('listener failure'); });
  context.__muxyApplyData(2); unsubscribe(); context.__muxyApplyData(3);
  assert.deepEqual(changes, [2]); assert.equal(muxy.data, 3);
  muxy.onFocus(value => focus.push(value));
  context.__muxyApplyFocus(true); context.__muxyApplyFocus(true); context.__muxyApplyFocus(false);
  assert.deepEqual(focus, [true, false]);
  context.__muxyApplyTheme({ colorScheme: 'light', topbarHeight: '40px' });
  assert.equal(css.get('--muxy-topbar-height'), '40px');
  assert.ok(Object.isFrozen(muxy.theme));
  assert.equal(typeof muxy.git.status, 'function'); assert.equal(typeof muxy.files.read, 'function');
});

test('theme initialization and updates leave authored backgrounds untouched', () => {
  const { context, root, authoredStyle, observers } = page();
  assert.deepEqual(root.children, [authoredStyle]);
  assert.equal(root.style.backgroundColor, undefined);
  assert.equal(observers.length, 0);
  context.__muxyApplyTheme({ background: '#fafafa', colorScheme: 'light' });
  assert.equal(root.children.length, 1);
  assert.equal(root.style.colorScheme, 'light');
});

test('a late document root receives the latest theme without waiting for DOMContentLoaded', () => {
  const { context, css, events, root, observers, attachRoot } = page(undefined, { rootPresent: false });
  assert.equal(css.size, 0);
  context.__muxyApplyTheme({ background: '#eeeedd', colorScheme: 'light' });
  attachRoot();
  assert.equal(css.get('--muxy-background'), '#eeeedd');
  assert.equal(root.style.colorScheme, 'light');
  assert.equal(root.children.length, 1);
  assert.equal(events.has('DOMContentLoaded'), false);
  assert.equal(observers.length, 1);
  assert.equal(observers[0].target, context.document);
  assert.equal(observers[0].options.childList, true);
  assert.equal(observers[0].active, false);
});

test('theme updates racing the root observer still disconnect it', () => {
  const { context, css, root, observers, attachRoot } = page(undefined, { rootPresent: false });
  context.document.documentElement = root;
  context.__muxyApplyTheme({ background: '#182838', colorScheme: 'dark' });
  attachRoot();
  assert.equal(css.get('--muxy-background'), '#182838');
  assert.equal(root.children.length, 1);
  assert.equal(observers[0].active, false);
});

for (const surface of ['tab', 'panel']) test(`${surface} close defaults, acknowledgements, async veto, errors, and unsubscribe`, async () => {
  const { context, muxy, messages } = page(undefined, { surface });
  context.__muxyBeforeClose(1, surface, 'instance'); await settled();
  assert.deepEqual(messages.at(-1).args, { callID: '1', prevent: false });
  let release;
  const unsubscribe = muxy.lifecycle.onBeforeClose(info => {
    assert.equal(info.surface, surface); assert.equal(info.instanceID, 'instance');
    return new Promise(resolve => { release = resolve; });
  });
  context.__muxyBeforeClose(2, surface, 'instance'); await settled();
  assert.equal(messages.at(-1).verb, 'lifecycle.ackBeforeClose');
  release({ prevent: true }); await settled();
  assert.deepEqual(messages.at(-1).args, { callID: '2', prevent: true });
  unsubscribe(); context.__muxyBeforeClose(3, surface, 'instance'); await settled();
  assert.equal(messages.at(-1).args.prevent, false);
  muxy.lifecycle.onBeforeClose(() => { throw Error('broken handler'); });
  context.__muxyBeforeClose(4, surface, 'instance'); await settled();
  assert.equal(messages.at(-1).args.prevent, false);
  muxy.lifecycle.onBeforeClose(() => Promise.reject(Error('rejected handler')));
  context.__muxyBeforeClose(5, surface, 'instance'); await settled();
  assert.equal(messages.at(-1).args.prevent, false);
});

test('modal open awaits its correlated result and submission identifies its own surface', async () => {
  const { muxy, messages } = page(message => {
    if (message.verb === 'modal.openWebview') return { requestID: 'modal-1' };
    if (message.verb === 'modal.awaitWebview') return { accepted: true };
  });
  assert.deepEqual(await muxy.modal.openWebview({ entry: 'modal.html', width: 600 }), { accepted: true });
  assert.equal(messages[1].args.requestID, 'modal-1');
  assert.notEqual(messages[0].requestID, messages[1].requestID);
  await muxy.modal.submitWebview({ text: 'answer' });
  assert.deepEqual(messages.at(-1).args, { requestID: 'instance', result: { text: 'answer' } });
  await muxy.tabs.setTitle(null);
  assert.deepEqual(messages.at(-1).args, { tabInstanceID: 'instance', title: '' });
  await muxy.tabs.setIcon({ svg: 'icon.svg' });
  assert.equal(messages.at(-1).args.icon.svg, 'icon.svg');
});

test('host failures reject promises rather than resolving fake successful results', async () => {
  const { context, muxy } = page();
  context.webkit.messageHandlers.muxy.postMessage = async () => ({ ok: false, error: 'not authorized' });
  await assert.rejects(muxy.tabs.open({}), /not authorized/);
  await assert.rejects(muxy.modal.openWebview({ entry: 'modal.html' }), /not authorized/);
});


test('panels forward scoped IDs and preserve omitted versus explicit data', async () => {
  const { muxy, messages } = page(undefined, { surface: 'panel' });
  assert.equal(muxy.panelID, 'instance');
  await muxy.panels.open('review', { draft: 'retained' });
  await muxy.panels.open('review');
  await muxy.panels.toggle('review', null);
  await muxy.panels.close();
  await muxy.panels.close('other');
  assert.deepEqual(messages.map(({verb, args}) => ({verb, args})), [
    {verb:'panels.open', args:{panelID:'review', data:{draft:'retained'}}},
    {verb:'panels.open', args:{panelID:'review'}},
    {verb:'panels.toggle', args:{panelID:'review', data:null}},
    {verb:'panels.close', args:{panelID:'instance'}},
    {verb:'panels.close', args:{panelID:'other'}},
  ]);
  assert.equal(page().muxy.panelID, null);
});

test('panel close handlers receive their surface identity and can veto', async () => {
  const { context, muxy, messages } = page(undefined, { surface: 'panel' });
  muxy.lifecycle.onBeforeClose(({surface, instanceID}) => {
    assert.equal(surface, 'panel');
    assert.equal(instanceID, 'instance');
    return true;
  });
  context.__muxyBeforeClose(10, 'panel', 'instance');
  await settled();
  assert.deepEqual(messages.at(-1).args, {callID:'10', prevent:true});
});

test('pages expose main\'s API surface', () => {
  const { muxy } = page();
  for (const path of [
    'popover.close', 'popover.resize', 'http.fetch', 'events.emit', 'events.subscribe',
    'tabs.new', 'tabs.next', 'tabs.previous', 'tabs.open', 'tabs.setTitle', 'panes.list',
    'panes.readScreen', 'projects.list', 'projects.reorder', 'worktrees.list', 'agents.list',
    'gh.user', 'shortcuts.register', 'shortcuts.list', 'statusbar.set', 'topbar.hide',
    'browser.list', 'workspaces.list', 'dialog.pickFolder', 'modal.open', 'storage.keys',
  ]) {
    const value = path.split('.').reduce((object, key) => object && object[key], muxy);
    assert.equal(typeof value, 'function', path);
  }
});

test('http, popover, and item calls forward main\'s payloads', async () => {
  const { muxy, messages } = page();
  await muxy.http.fetch('https://example.com', { method: 'post', headers: { a: 'b' }, body: 1, timeoutMs: '5' });
  await muxy.popover.resize('100', 50);
  await muxy.statusbar.set({ id: 3, text: null, visible: 0 });
  await muxy.topbar.show('star');
  assert.deepEqual(messages.map(message => [message.verb, message.args]), [
    ['http.fetch', { url: 'https://example.com', method: 'post', headers: { a: 'b' }, body: '1', timeoutMs: 5 }],
    ['popover.resize', { width: 100, height: 50 }],
    ['statusbar.set', { id: '3', text: null, visible: false }],
    ['topbar.set', { id: 'star', visible: true }],
  ]);
});

test('extension events are checked before they reach the host', async () => {
  const { muxy, messages } = page();
  await assert.rejects(muxy.events.emit('ready', {}), /extension events must start with extension\./);
  await muxy.events.emit('extension.ready', { n: 1 });
  await muxy.events.emit('extension.empty');
  assert.deepEqual(messages.map(message => message.args), [
    { event: 'extension.ready', payload: { n: 1 } },
    { event: 'extension.empty', payload: null },
  ]);
});

test('page modals resolve the choice and call onSelect', async () => {
  const { muxy, messages } = page(message => ({
    'modal.open': { requestID: 'native:1' },
    'modal.await': { id: 'b', title: 'Beta' },
  })[message.verb] ?? null);
  const selected = [];
  const choice = await muxy.modal.open({
    placeholder: 'Pick',
    items: [{ id: 'a', title: 'Alpha' }, { id: 'b', title: 'Beta', subtitle: 2 }, { title: 'missing id' }],
    onSelect: item => selected.push(item),
  });
  assert.deepEqual(choice, { id: 'b', title: 'Beta' });
  assert.deepEqual(selected, [{ id: 'b', title: 'Beta' }]);
  assert.deepEqual(messages.map(message => message.verb), ['modal.open', 'modal.feed', 'modal.finish', 'modal.await']);
  assert.deepEqual(messages[1].args.items, [
    { id: 'a', title: 'Alpha', subtitle: null },
    { id: 'b', title: 'Beta', subtitle: '2' },
  ]);
  assert.deepEqual(messages[3].args, { requestID: 'native:1' });
});

test('page modals run async onQuery handlers for each query', async () => {
  const { context, muxy, messages } = page(message => ({
    'modal.open': { requestID: 'native:3' },
    'modal.await': new Promise(() => {}),
  })[message.verb] ?? null);
  const queries = [];
  const opened = muxy.modal.open({
    items: [],
    onQuery: async query => { queries.push(query); return [{ id: query, title: query.toUpperCase() }]; },
  });
  await settled();
  await context.__muxyDeliverModalQuery('native:3', '2', 'needle', {});
  assert.deepEqual(queries, ['needle']);
  const verbs = messages.map(message => [message.verb, message.args]);
  assert.deepEqual(verbs.slice(-2), [
    ['modal.feed', { items: [{ id: 'needle', title: 'NEEDLE', subtitle: null }], queryID: '2' }],
    ['modal.finish', { queryID: '2' }],
  ]);
  void opened;
});

test('page console lines reach the host in one batch per tick', async () => {
  const { context, messages } = page(undefined, { console: true });
  for (let index = 0; index < 300; index++) context.console.log('line', index);
  context.console.warn({ a: 1 });
  await new Promise(resolve => setTimeout(resolve, 5));
  const batches = messages.filter(message => message.verb === 'console');
  assert.equal(batches.length, 1);
  assert.equal(batches[0].args.lines.length, 301);
  assert.deepEqual(batches[0].args.lines[300], { level: 'warn', message: '{"a":1}' });
});
