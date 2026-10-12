// Read-only browser checks; remote catalogs and transfer requests are intercepted.
const assert = require("node:assert/strict");
const { chromium } = require(process.env.OACLIS_PLAYWRIGHT || "playwright");
(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.OACLIS_CHROMIUM || undefined,
    args: ["--no-sandbox"],
  });
  try {
    const page = await browser.newPage({
        viewport: { width: 1440, height: 960 },
      }),
      errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    for (const endpoint of ["analyze", "analysis", "scopes"]) {
      await page.route("**/api/" + endpoint, (route) =>
        route.fulfill({
          json: {
            status: "ready",
            scopes: route.request().postDataJSON()?.scopes || {
              chat: true,
              paths: true,
              apps: true,
            },
            manifest: { paths: [], packages: [], plugins: [], configs: [] },
          },
        }),
      );
    }
    await page.goto("http://localhost:23232");
    await page.waitForSelector(".map-node");
    assert.equal(await page.locator("#panel").isHidden(), true);
    assert.equal(await page.locator("#activity").isHidden(), true);
    assert.equal(await page.locator("h1").count(), 0);
    assert.equal(
      await page
        .locator("#map")
        .evaluate(
          (e) => e.clientWidth === innerWidth && e.clientHeight === innerHeight,
        ),
      true,
    );
    const token = await page
      .locator('meta[name="oaclis-token"]')
      .getAttribute("content");
    const readState = async () =>
      await (
        await page.request.get("http://localhost:23232/api/state", {
          headers: { "X-Oaclis-Token": token },
        })
      ).json();
    const before = await readState();
    assert.equal(
      (await page.request.get("http://localhost:23232/api/state")).status(),
      403,
    );
    assert.equal(
      (
        await page.request.get("http://localhost:23232/", {
          headers: { Host: "untrusted.example:23232" },
        })
      ).status(),
      403,
    );
    await page.screenshot({
      path: "/root/Oaclis/.impeccable/review/desktop.png",
    });
    const writes = [];
    await page.route("**/api/run", (route) => {
      writes.push(route.request().postDataJSON());
      return route.fulfill({ json: { ok: true } });
    });
    const master = page.locator('.map-node[data-id="master"]');
    await master.click();
    await page.waitForSelector(".library-item");
    assert.equal(
      await page.locator(".library-item small").count(),
      await page.locator(".library-item").count(),
    );
    await page.locator(".library-item").first().click();
    await page.waitForSelector("#chat-form");
    assert.equal(
      await page
        .locator("#details h2")
        .evaluate((e) => document.activeElement === e),
      true,
    );
    if (before.devices.length) {
      await page.getByRole("button", { name: "Avvia", exact: true }).click();
      assert.equal(writes[0].source_device_id, "master");
      assert.equal(writes[0].operation, "copy");
      const remote = before.devices[0];
      let remoteTitle = "Chat remota (test)";
      await page.route("**/api/catalog", (route) => {
        if (route.request().postDataJSON().device_id !== remote.id)
          return route.continue();
        return route.fulfill({
          json: {
            root: "/root/.codex",
            chats: [
              {
                ...before.chats[0],
                name: remoteTitle,
                relative_path: "sessions/test.jsonl",
                path: "/root/.codex/sessions/test.jsonl",
              },
            ],
          },
        });
      });
      await page
        .locator(".map-node")
        .filter({ has: page.locator(".node-name", { hasText: remote.name }) })
        .click();
      await page
        .locator(".library-item")
        .filter({ hasText: remoteTitle })
        .waitFor();
      remoteTitle = "Titolo aggiornato (test)";
      await page
        .locator(".library-item")
        .filter({ hasText: remoteTitle })
        .waitFor({ timeout: 12000 });
      const item = page.locator(".library-item").first();
      const transfer = await page.evaluateHandle(() => new DataTransfer());
      await item.dispatchEvent("dragstart", { dataTransfer: transfer });
      await master.dispatchEvent("drop", { dataTransfer: transfer });
      await item.dispatchEvent("dragend", { dataTransfer: transfer });
      await page.waitForSelector("#chat-form");
      assert.equal(writes.length, 1);
      assert.equal(
        await page.locator('select[name="device_id"]').inputValue(),
        "master",
      );
      await page.getByRole("button", { name: "Avvia", exact: true }).click();
      assert.equal(writes[1].source_device_id, remote.id);
      assert.equal(writes[1].device_id, "master");
      assert.equal(writes[1].destination, undefined);
    }
    await page.locator("#theme").click();
    await page.waitForTimeout(200);
    await page.screenshot({ path: "/root/Oaclis/.impeccable/review/dark.png" });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.waitForTimeout(150);
    await page.screenshot({
      path: "/root/Oaclis/.impeccable/review/mobile.png",
    });
    assert.equal(
      await page.evaluate(
        () => document.documentElement.scrollWidth > innerWidth,
      ),
      false,
    );
    await page
      .getByRole("button", { name: "Aggiungi dispositivo", exact: true })
      .click();
    assert.equal(
      await page
        .locator("#details h2")
        .evaluate((e) => document.activeElement === e),
      true,
    );
    await page
      .getByRole("button", { name: "Chiudi pannello", exact: true })
      .click();
    assert.equal(await page.locator("#panel").isHidden(), true);
    await page.evaluate(() => {
      data.devices = Array.from({ length: 5 }, (_, i) => ({
        id: "fixture-" + i,
        name: "Dispositivo " + (i + 1),
        ssh_alias: "fixture-" + i,
      }));
      positions = {};
      renderMap();
    });
    const bounds = await page.locator(".map-node").evaluateAll((nodes) =>
      nodes.map((n) => ({
        x: n.offsetLeft,
        y: n.offsetTop,
        w: n.offsetWidth,
        h: n.offsetHeight,
      })),
    );
    for (let i = 0; i < bounds.length; i++)
      for (let j = i + 1; j < bounds.length; j++)
        assert.ok(
          bounds[i].x + bounds[i].w <= bounds[j].x ||
            bounds[j].x + bounds[j].w <= bounds[i].x ||
            bounds[i].y + bounds[i].h <= bounds[j].y ||
            bounds[j].y + bounds[j].h <= bounds[i].y,
        );
    await page.screenshot({
      path: "/root/Oaclis/.impeccable/review/mobile-six.png",
    });
    await page.locator(".map-node").last().scrollIntoViewIfNeeded();
    await page.evaluate(() => {
      data.devices = [];
      positions = {};
      renderMap();
    });
    const after = await readState();
    assert.deepEqual(after.devices, before.devices);
    assert.deepEqual(after.rules, before.rules);
    assert.deepEqual(after.job, before.job);
    let expired = false;
    await page.route("**/api/status", (route) => {
      if (!expired) {
        expired = true;
        return route.fulfill({
          status: 403,
          body: "Sessione locale richiesta",
        });
      }
      return route.continue();
    });
    const reloaded = page.waitForEvent("load", { timeout: 5000 });
    await page.evaluate(() => api("status").catch(() => {})).catch(() => {});
    await reloaded;
    await page.waitForSelector(".map-node");
    assert.equal(expired, true);
    assert.deepEqual(errors, []);
    console.log(
      "UI: fullscreen map, contextual panels, local/remote catalogs, rename refresh, both transfer proposals, mobile, token recovery; no real writes: PASS",
    );
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
