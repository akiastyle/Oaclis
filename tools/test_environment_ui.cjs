// Bounded UI check; analysis, installation and transfers are intercepted.
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
    page.on("pageerror", (error) => errors.push(error.message));
    let state = "pending",
      selection = { chat: true, paths: true, apps: true },
      request;
    const response = () => ({
      status: state,
      scopes: selection,
      manifest: {
        paths: ["/work/project"],
        packages: ["nginx"],
        plugins: [],
        configs: [],
      },
      error: "Analisi non disponibile",
    });
    await page.route("**/api/analyze", (route) =>
      route.fulfill({ json: response() }),
    );
    await page.route("**/api/analysis", (route) =>
      route.fulfill({ json: response() }),
    );
    await page.route("**/api/scopes", (route) => {
      selection = route.request().postDataJSON().scopes;
      return route.fulfill({ json: response() });
    });
    await page.route("**/api/run", (route) => {
      request = route.request().postDataJSON();
      return route.fulfill({ json: { ok: true } });
    });
    await page.goto("http://localhost:23232");
    await page.waitForSelector(".map-node");
    await page.locator('.map-node[data-id="master"]').click();
    await page.waitForSelector(".library-item");
    await page.locator(".library-item").first().click();
    await page.waitForSelector(".scope-toggle");
    assert.equal(
      await page.locator('.scope-toggle[aria-pressed="true"]').count(),
      3,
    );
    assert.equal(await page.locator("#start-chat").isDisabled(), true);
    assert.equal(await page.locator(".analysis-spinner").count(), 1);
    assert.equal(await page.locator('[name="destination"]').count(), 0);
    state = "ready";
    await page.waitForFunction(
      () => !document.querySelector("#start-chat").disabled,
    );
    await page.locator('[data-scope="apps"]').click();
    await page.waitForFunction(
      () => document.querySelector("#chat-form").dataset.saving === "false",
    );
    assert.equal(selection.apps, false);
    await page.screenshot({ path: ".impeccable/review/environment-ready.png" });
    await page.locator("#start-chat").click();
    assert.deepEqual(request.scopes, { chat: true, paths: true, apps: false });
    await page.locator('.map-node[data-id="master"]').click();
    await page.locator(".library-item").first().click();
    await page.waitForFunction(
      () => !document.querySelector("#start-chat").disabled,
    );
    assert.equal(
      await page.locator('[data-scope="apps"]').getAttribute("aria-pressed"),
      "false",
    );
    await page.locator("#back").click();
    state = "failed";
    await page.locator(".library-item").first().click();
    await page.getByRole("button", { name: "Ripeti analisi" }).waitFor();
    assert.equal(await page.locator("#start-chat").isDisabled(), true);
    state = "ready";
    await page.getByRole("button", { name: "Ripeti analisi" }).click();
    await page.waitForFunction(
      () => !document.querySelector("#start-chat").disabled,
    );
    await page.setViewportSize({ width: 390, height: 844 });
    await page.screenshot({
      path: ".impeccable/review/environment-mobile.png",
    });
    assert.deepEqual(errors, []);
    console.log(
      "UI: icon scopes, real loading states, saved choices, retry and payload OK; no real installations/transfers",
    );
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
