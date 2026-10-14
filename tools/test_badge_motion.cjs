// test_badge_motion.cjs v1.0.47
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('/root/google-images-local/node_modules/playwright');
(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: '/root/.cache/ms-playwright/chromium-1228/chrome-linux64/chrome', args: ['--no-sandbox'] });
  try {
    const page = await browser.newPage({ viewport: { width: 800, height: 600 } });
    const errors = []; page.on('pageerror', e => errors.push(e.message));
    const css = fs.readFileSync(path.join(__dirname, '../ui/badge-motion.css'), 'utf8');
    await page.setContent(`<style>:root{--duration-ui:.5s;--link-badge-size:44px;--panel:#222;--glass-opacity:60%;--glass-blur:3px}body{background:#111}.link-badge{position:absolute;left:300px;top:200px;width:44px;height:44px}.link-main{position:relative;z-index:4;width:44px;height:44px;border-radius:50%;background:#222}.link-edit,.link-open{position:absolute}.link-badge:hover .link-edit{opacity:1}${css}</style><div class="link-badge"><button class="link-main">O</button><button class="link-edit">M</button><button class="link-open">C</button></div>`);
    await page.addScriptTag({ path: path.join(__dirname, '../ui/badge-motion.js') });
    await page.evaluate(() => {
      const host = document.querySelector('.link-badge');
      window.motion = new OaclisBadgeMotion(host, { chat: '', paths: '', apps: '' });
      motion.update({ phase: 'idle', modules: {} });
      window.saved = host.querySelector('.badge-gauges');
    });
    await page.waitForTimeout(900);
    await page.evaluate(() => motion.update({ phase: 'checking', modules: {} }));
    assert.equal(await page.locator('.link-badge').getAttribute('data-motion-phase'), 'checking');
    await page.evaluate(() => {
      window.state = { phase: 'transferring', modules: { chat: { changed: true, progress: .3 }, paths: { changed: false, progress: 1 }, apps: { changed: true, progress: .2 } } };
      motion.update(state);
    });
    await page.waitForTimeout(1500);
    assert.equal(await page.locator('.badge-ring.is-active').count(), 2);
    await page.locator('.link-badge').hover();
    assert.equal(await page.locator('.link-edit').evaluate(el => getComputedStyle(el).opacity), '0', await page.locator('.link-badge').getAttribute('data-motion-phase'));
    const before = await page.locator('.badge-ring[data-module="chat"]').evaluate(el => el.getAnimations()[0].startTime);
    await page.evaluate(() => { motion.update(state); motion.update(state); });
    assert.equal(await page.locator('.badge-ring[data-module="chat"]').evaluate(el => el.getAnimations()[0].startTime), before);
    assert.equal(await page.evaluate(() => saved === document.querySelector('.badge-gauges')), true);
    const colors = await page.evaluate(() => [document.querySelector('.badge-fill').style.stroke, document.querySelector('.badge-module').style.color]);
    assert.equal(colors[0], colors[1]);
    await page.evaluate(() => motion.update({ ...state, phase: 'paused' }));
    assert.equal(await page.locator('.badge-ring[data-module="chat"]').evaluate(el => getComputedStyle(el).animationPlayState), 'paused');
    await page.evaluate(() => motion.update({ phase: 'idle', modules: {} }));
    await page.waitForTimeout(1700);
    assert.equal(await page.locator('.link-badge').getAttribute('data-motion-phase'), 'idle');
    assert.equal(await page.locator('.link-edit').isEnabled(), true);
    await page.evaluate(() => motion.destroy());
    assert.equal(await page.locator('.badge-motion').count(), 0);
    assert.deepEqual(errors, []);
    console.log('Badge module: stable DOM, selective rings, shared colors, pause and exit passed.');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exitCode = 1; });
