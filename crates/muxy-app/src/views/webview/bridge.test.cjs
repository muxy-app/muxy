const { test } = require('node:test');
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const vm = require('node:vm');
const source = readFileSync(`${__dirname}/bridge.js`, 'utf8');

function page(reply = () => null, { rootPresent = true, surface = "tab" } = {}) {
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
  const context = vm.createContext({ document, MutationObserver });
  context.window = context;
  context.webkit = { messageHandlers: { muxy: { postMessage: async message => {
    messages.push(JSON.parse(JSON.stringify(message)));
    return { ok: true, value: await reply(message) };
  } } } };
  vm.runInContext(`${source}(${JSON.stringify({ owner: 'test', id: 'instance', surface, data: { saved: true }, theme: { background: '#123456', foregroundMuted: '#888888', colorScheme: 'dark', topbarHeight: '34px' } })});`, context);
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
  assert.equal(muxy.git, undefined); assert.equal(muxy.files, undefined);
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

test('close defaults, acknowledgements, async veto, errors, and unsubscribe', async () => {
  const { context, muxy, messages } = page();
  context.__muxyBeforeClose(1, 'tab', 'instance'); await settled();
  assert.deepEqual(messages.at(-1).args, { callID: '1', prevent: false });
  let release;
  const unsubscribe = muxy.lifecycle.onBeforeClose(info => {
    assert.equal(info.surface, 'tab'); assert.equal(info.instanceID, 'instance');
    return new Promise(resolve => { release = resolve; });
  });
  context.__muxyBeforeClose(2, 'tab', 'instance'); await settled();
  assert.equal(messages.at(-1).verb, 'lifecycle.ackBeforeClose');
  release({ prevent: true }); await settled();
  assert.deepEqual(messages.at(-1).args, { callID: '2', prevent: true });
  unsubscribe(); context.__muxyBeforeClose(3, 'tab', 'instance'); await settled();
  assert.equal(messages.at(-1).args.prevent, false);
  muxy.lifecycle.onBeforeClose(() => { throw Error('broken handler'); });
  context.__muxyBeforeClose(4, 'tab', 'instance'); await settled();
  assert.equal(messages.at(-1).args.prevent, false);
  muxy.lifecycle.onBeforeClose(() => Promise.reject(Error('rejected handler')));
  context.__muxyBeforeClose(5, 'tab', 'instance'); await settled();
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
