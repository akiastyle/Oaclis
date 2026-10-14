// test_delete_connection.cjs v1.0.30
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync(require('node:path').join(__dirname, '../ui/panels.js'), 'utf8');
const fn = source.slice(source.indexOf('async function removeConnection('), source.indexOf('function positionContext('));
(async () => {
  for (const fails of [false, true]) {
    const connection = { entry: { id: 'chat' }, source: 'master', target: 'jetson' };
    const other = { ...connection, target: 'dell' };
    const preferences = { chat: { hidden: false } };
    const calls = [];
    const context = {
      removingPresentation: false, chatLinks: [connection, other],
      data: { chat_settings: preferences }, saveLinks: Promise.resolve(),
      window: { confirm: () => true }, renderLink() {}, closeChatEditor() {},
      api: async (path, body) => { calls.push([path, body]); if (fails) throw Error('offline'); },
    };
    vm.createContext(context); vm.runInContext(fn, context);
    const pending = context.removeConnection(connection, {});
    assert.equal(context.chatLinks.length, 1);
    if (fails) await assert.rejects(pending, /offline/); else await pending;
    assert.equal(context.chatLinks.length, fails ? 2 : 1);
    assert.equal(context.chatLinks.includes(other), true);
    assert.equal(context.data.chat_settings, preferences);
    assert.equal(preferences.chat.hidden, false);
    assert.equal(calls[0][0], 'links/delete');
    assert.equal(context.removingPresentation, false);
  }
  console.log('Connection deletion preserves visibility and other links; rollback passed.');
})().catch(error => { console.error(error); process.exitCode = 1; });
