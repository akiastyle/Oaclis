const assert = require("node:assert/strict");
const { chromium } = require(process.env.OACLIS_PLAYWRIGHT);
(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.OACLIS_CHROMIUM,
    args: ["--no-sandbox"],
  });
  try {
    const page = await browser.newPage({
      viewport: { width: 1440, height: 960 },
    });
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await page.route("**/api/status", async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      for (const id of Object.keys(body.connections))
        body.connections[id] = "online";
      await route.fulfill({ json: body });
    });
    let saved;
    await page.route("**/api/appearance", (route) => {
      saved = route.request().postDataJSON();
      return route.fulfill({ json: { ok: true } });
    });
    await page.addInitScript(() => {
      window.cspViolations = [];
      document.addEventListener("securitypolicyviolation", (e) =>
        window.cspViolations.push(e.violatedDirective),
      );
    });
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
    await page.locator('.map-node[data-id="master"]').click();
    await page.waitForSelector(".library-item");
    const native = await page
      .locator(".library-item strong")
      .first()
      .textContent();
    assert.equal(
      await page.locator(".library-item .chat-badge").first().textContent(),
      [...native][0].toLocaleUpperCase(),
    );
    await page.locator(".library-row .icon-button").first().click();
    await page.locator("[name=label]").fill("Lavoro personale");
    await page.locator("[name=color]").evaluate((el) => {
      el.value = "#995533";
      el.dispatchEvent(new Event("input", { bubbles: true }));
    });
    const image = await page.evaluate(() => {
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = 16;
      const ctx = canvas.getContext("2d");
      ctx.fillStyle = "#995533";
      ctx.fillRect(0, 0, 16, 16);
      return canvas.toDataURL("image/png").split(",")[1];
    });
    await page
      .locator("#icon-upload")
      .setInputFiles({
        name: "icon.png",
        mimeType: "image/png",
        buffer: Buffer.from(image, "base64"),
      });
    await page.waitForSelector("#icon-preview img");
    await page.getByRole("button", { name: "Salva", exact: true }).click();
    assert.ok(saved.value.icon.startsWith("data:image/png;base64,"));
    await page.locator(".library-item .chat-badge img").first().waitFor();
    assert.notEqual(
      await page
        .locator(".library-row")
        .first()
        .evaluate((el) => getComputedStyle(el).backgroundColor),
      "rgba(0, 0, 0, 0)",
    );
    assert.equal(
      await page
        .locator(".library-row > .icon-button")
        .first()
        .evaluate((el) => getComputedStyle(el).position),
      "absolute",
    );
    assert.equal(saved.value.color, "#995533");
    const originalIcon = saved.value.icon;
    await page.locator(".library-row .icon-button").first().click();
    const replacement = await page.evaluate(() => {
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = 16;
      const ctx = canvas.getContext("2d");
      ctx.fillStyle = "#336699";
      ctx.fillRect(0, 0, 16, 16);
      return canvas.toDataURL("image/png").split(",")[1];
    });
    await page
      .locator("#icon-upload")
      .setInputFiles({
        name: "replacement.png",
        mimeType: "image/png",
        buffer: Buffer.from(replacement, "base64"),
      });
    await page.waitForFunction(
      (original) =>
        document.querySelector("#icon-preview img")?.src !== original,
      originalIcon,
    );
    await page.getByRole("button", { name: "Salva", exact: true }).click();
    assert.notEqual(saved.value.icon, originalIcon);
    await page.locator(".library-item").first().click();
    await page.locator("[name=operation]").selectOption("sync");
    await page.waitForSelector("#chat-link");
    assert.equal(
      await page
        .locator(".link-badge")
        .evaluate(
          (el) =>
            el.getBoundingClientRect().x > 100 &&
            el.getBoundingClientRect().y > 100,
        ),
      true,
    );
    assert.equal(
      await page
        .locator(".connection-lines > path")
        .getAttribute("stroke-dasharray"),
      "none",
    );
    assert.equal(
      await page
        .locator("#chat-link")
        .evaluate((el) => getComputedStyle(el).color),
      "rgb(153, 85, 51)",
    );
    await page.locator("[name=operation]").selectOption("send");
    assert.equal(
      await page
        .locator(".connection-lines > path")
        .getAttribute("stroke-dasharray"),
      "7 6",
    );
    assert.equal(
      await page
        .locator('.map-node[data-id="master"] .connection-port')
        .count(),
      0,
    );
    await page.screenshot({ path: ".impeccable/review/appearance.png" });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.screenshot({ path: ".impeccable/review/appearance-mobile.png" });
    assert.deepEqual(errors, []);
    assert.deepEqual(await page.evaluate(() => window.cspViolations), []);
    console.log(
      "Chat initial, label/color, directional lines, master exclusion: OK (writes mocked)",
    );
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
