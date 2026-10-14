// test_delete_ui.cjs v1.15.47
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const script = fs.readFileSync(require('node:path').join(__dirname, '../ui/app.js'), 'utf8');
const fn = script.slice(script.indexOf('async function removePresentation('), script.indexOf('function positionContext('));
async function test(kind, fails) {
  const connection = { entry: { id: 'chat', name: 'Chat' }, source: 'master', target: 'client' };
  let removed = false, nodeRemoved = false, resolve, reject;
  const response = new Promise((yes, no) => { resolve = yes; reject = no; });
  const calls = [];
  const context = {
    removingPresentation: false,
    visibleChats: [connection.entry], chatLinks: [connection], link: null,
    data: { devices: [{ id: 'client', name: 'Client' }], chat_settings: {}, appearance: {} },
    positions: {}, sourceId: null, saveLinks: Promise.resolve(),
    window: { confirm: () => true }, chatLabel: entry => entry.name, deviceName: () => 'Client',
    document: { querySelectorAll: selector => selector === '.link-badge' ? [{ dataset: { linkKey: JSON.stringify(['chat', 'master', 'client']) }, closest: () => ({ remove: () => { removed = true; } }) }] : selector === '.map-node' ? [{ dataset: { id: 'client' }, remove: () => { nodeRemoved = true; } }] : [] },
    disposeChatLink: layer => layer?.remove(),
    renderLibrary() {}, renderMap() {}, closeDeviceSettings() {}, closeConnectionPopover() {}, savePositions() {},
    api: (path, body) => { calls.push([path, body]); return response; },
  };
  vm.createContext(context); vm.runInContext(fn, context);
  const id = kind === 'chat' ? 'chat' : 'client';
  const done = context.removePresentation(kind, id);
  assert.equal(removed, true, 'arc and badge disappear before the API finishes');
  assert.equal(context.chatLinks.length, 0);
  if (kind === 'device') assert.equal(nodeRemoved, true);
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(calls.length, 1);
  if (fails) {
    reject(new Error('Database unavailable'));
    await assert.rejects(done, /Database unavailable/);
    assert.equal(context.chatLinks.length, 1);
    assert.equal(context.data.devices.length, 1);
    assert.equal(context.data.chat_settings.chat, undefined);
  } else {
    resolve({ preferences: kind === 'chat' ? { chat: { hidden: true } } : {}, links: [] });
    await done;
    assert.equal(context.chatLinks.length, 0);
    assert.equal(calls.length, 1, 'no state reload delays deletion');
  }
  assert.equal(context.removingPresentation, false);
}
(async () => {
  for (const kind of ['chat', 'device']) for (const fails of [false, true]) await test(kind, fails);
  console.log('Deletion: immediate display and rollback passed for chats and devices.');
})().catch(error => { console.error(error); process.exitCode = 1; });
