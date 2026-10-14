// test_link_updates.cjs v1.8.60
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const text = fs.readFileSync(require('node:path').join(__dirname, '../ui/map.js'), 'utf8');
const fn = text.slice(text.indexOf('function renderLink('), text.indexOf('// Split the original cubic'));
let layers = [], draws = 0;
const points = { master: 0, a: 100, b: 200 };
const colors = { one: 'blue', two: 'green' };
const context = {
  chatLinks: [{ entry: { id: 'one' }, source: 'master', target: 'a', operation: 'sync' }, { entry: { id: 'two' }, source: 'master', target: 'b', operation: 'sync' }],
  link: null, drawingMap: false, linkPositions: {}, appearance: entry => ({ color: colors[entry.id] }),
  document: {
    querySelector: () => null,
    querySelectorAll: selector => selector === '.chat-link' ? layers : Object.entries(points).map(([id, x]) => ({ dataset: { id }, getBoundingClientRect: () => ({ x, y: 0, width: 200, height: 100 }) })),
  },
  disposeChatLink: layer => layer.remove(),
  drawChatLink(value, index, lane, existing) {
    draws++;
    if (existing) return existing;
    const layer = { dataset: {}, remove() { layers = layers.filter(current => current !== layer); } };
    layers.push(layer); return layer;
  },
};
vm.createContext(context); vm.runInContext(fn, context);
context.renderLink(); assert.equal(draws, 2);
const originals = [...layers];
context.drawingMap = true;
context.renderLink(); assert.equal(draws, 2, 'map construction preserves badges');
context.drawingMap = false;
context.renderLink(); assert.equal(draws, 2, 'unchanged polling does not reconstruct links');
assert.deepEqual(layers, originals);
points.a += 20;
context.renderLink(); assert.equal(draws, 3, 'moving one endpoint only updates its link');
assert.ok(layers.includes(originals[1]));
colors.two = 'red';
context.renderLink(); assert.equal(draws, 4);
context.chatLinks.pop();
context.renderLink(); assert.equal(layers.length, 1); assert.equal(draws, 4);
console.log('Links: unchanged elements preserved; affected links updated and deleted.');
// The adapter does not invent transfer work and does not animate stale checks while paused.
context.data = { paused: true, analyses: { 'master:one': { status: 'pending' } }, connections: {}, job: { status: 'idle' } };
let visual;
const marker = { badgeMotion: { update: state => { visual = state; } } };
const adapter = text.slice(text.indexOf('function updateBadgeMotion('), text.indexOf('function renderLink('));
vm.runInContext(adapter, context);
context.updateBadgeMotion(marker, context.chatLinks[0]);
assert.equal(visual.phase, 'idle');
context.data.paused = false;
context.updateBadgeMotion(marker, context.chatLinks[0]);
assert.equal(visual.phase, 'checking');
context.data.analyses = {};
context.data.job = { chat_id: 'one', source_device_id: 'master', device_id: 'a', status: 'running', module_progress: { chat: { changed: true, progress: .4 } } };
context.updateBadgeMotion(marker, context.chatLinks[0]);
assert.equal(visual.phase, 'transferring');
context.data.paused = true;
context.updateBadgeMotion(marker, context.chatLinks[0]);
assert.equal(visual.phase, 'paused');
console.log('Adapter: paused checks stay still; only real module progress activates gauges.');
