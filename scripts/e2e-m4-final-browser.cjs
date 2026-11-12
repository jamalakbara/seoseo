const { chromium } = require(
  process.env.PLAYWRIGHT_MODULE || "playwright-core",
);
const assert = require("node:assert/strict");
const { randomUUID, createHash } = require("node:crypto");
(async () => {
  const origin = process.argv[2] || "http://127.0.0.1:3300";
  const browser = await chromium.launch({
    executablePath:
      process.env.CHROME_PATH ||
      "C:/Program Files/Google/Chrome/Application/chrome.exe",
    headless: true,
  });
  const context = await browser.newContext({
    viewport: { width: 1280, height: 900 },
  });
  let pid, page;
  async function api(path, method = "GET", data) {
    const r = await context.request.fetch(origin + "/api/v1" + path, {
      method,
      data,
      headers: { "x-seoseo-csrf": "1" },
    });
    assert(r.ok(), `${method} ${path}: ${r.status()} ${await r.text()}`);
    return r.status() === 204 ? null : r.json();
  }
  async function control(mode) {
    const r = await context.request.post("http://127.0.0.1:3101/control", {
      data: { mode },
    });
    assert(r.ok());
  }
  try {
    await api("/auth/sign-up", "POST", {
      email: `m4-final-ui-${randomUUID()}@example.com`,
      password: "disposable-browser-password",
      name: "M4 final browser",
    });
    const p = await api("/projects", "POST", {
      name: "M4 final browser",
      domain: "badseo.test",
    });
    pid = p.id;
    await api(`/projects/${pid}/website-research`, "POST", {
      start_url: "http://badseo.test/",
    });
    page = await context.newPage();
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("dialog", (d) => d.accept());
    await page.goto(`${origin}/p/${pid}/onboarding`);
    await page
      .getByRole("button", { name: "Use website suggestion", exact: true })
      .waitFor({ timeout: 60000 });
    await page
      .getByRole("button", { name: "Generate model draft", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Apply draft to form", exact: true })
      .waitFor();
    assert.equal((await api(`/projects/${pid}/context`)).revision, 0);
    await page
      .getByRole("button", { name: "Apply draft to form", exact: true })
      .click();
    assert.equal(
      await page.getByLabel("Business overview", { exact: true }).inputValue(),
      "Fixture business serving small teams",
    );
    await page
      .getByRole("button", { name: "Confirm and save setup", exact: true })
      .click();
    await page
      .getByText("Website setup saved to project memory", { exact: true })
      .waitFor();
    await page.getByRole("link", { name: "Integrations", exact: true }).click();
    await page
      .getByRole("heading", { name: "Integrations", exact: true })
      .waitFor();
    await page
      .getByRole("button", { name: "Connect Google Analytics", exact: true })
      .click();
    await page.getByLabel("Analytics property", { exact: true }).waitFor();
    await page
      .getByLabel("Analytics property", { exact: true })
      .selectOption("properties/111");
    await page
      .getByRole("button", { name: "Use property", exact: true })
      .click();
    await page.getByLabel("Analytics report", { exact: true }).waitFor();
    for (const kind of [
      "organic_overview",
      "organic_landing_pages",
      "page_performance",
      "traffic_acquisition",
      "key_events",
      "ecommerce_performance",
      "site_search",
      "audience_breakdown",
      "measurement_health",
    ]) {
      await page
        .getByLabel("Analytics report", { exact: true })
        .selectOption(kind);
      await page
        .getByRole("button", { name: "Load Analytics report", exact: true })
        .click();
      await page.getByText(/reported rows/).waitFor();
      assert.equal(await page.locator("tbody tr").count(), 1);
    }
    await api(`/projects/${pid}`, "PATCH", { domain: "example.test" });
    await page.reload();
    await page
      .getByRole("button", { name: "Set up IndexNow", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Verify ownership file", exact: true })
      .click();
    await page.getByText(/Ownership verified/).waitFor();
    await page
      .getByLabel("Changed URLs, one per line", { exact: true })
      .fill("http://example.test/changed");
    await page
      .getByRole("button", { name: "Submit changed URLs", exact: true })
      .click();
    await page.getByText(/engine key validation pending/).waitFor();
    await page
      .getByLabel("Source URL", { exact: true })
      .fill("https://source.example/article");
    await page
      .getByLabel("Target URL on your website", { exact: true })
      .fill("https://example.test/service");
    await page
      .getByRole("button", { name: "Record link observation", exact: true })
      .click();
    await page
      .getByText(
        "live: https://source.example/article → https://example.test/service",
        { exact: true },
      )
      .waitFor();
    const registration = await context.request.post(
      origin + "/oauth/register",
      {
        data: {
          client_name: "Browser MCP client",
          redirect_uris: ["http://127.0.0.1:49199/callback"],
        },
      },
    );
    assert(registration.ok());
    const client = await registration.json();
    const verifier = createHash("sha256")
      .update(randomUUID())
      .digest("base64url");
    const auth = {
      client_id: client.client_id,
      redirect_uri: client.redirect_uris[0],
      response_type: "code",
      code_challenge: createHash("sha256").update(verifier).digest("base64url"),
      code_challenge_method: "S256",
      resource: origin + "/mcp",
      scope: "mcp",
      state: "browser-state",
    };
    await page.route("http://127.0.0.1:49199/callback**", (route) =>
      route.fulfill({ body: "Client callback received" }),
    );
    await page.goto(origin + "/oauth/authorize?" + new URLSearchParams(auth));
    await page.getByText("Browser MCP client", { exact: true }).waitFor();
    await page
      .getByRole("button", { name: "Allow workspace access", exact: true })
      .click();
    await page.waitForURL(/49199\/callback/);
    const code = new URL(page.url()).searchParams.get("code");
    assert(code);
    const tokenResponse = await context.request.post(origin + "/oauth/token", {
      form: {
        grant_type: "authorization_code",
        client_id: client.client_id,
        redirect_uri: auth.redirect_uri,
        code,
        code_verifier: verifier,
        resource: auth.resource,
      },
    });
    assert(tokenResponse.ok(), await tokenResponse.text());
    const token = await tokenResponse.json();
    await page.goto(origin + "/settings/connected-apps");
    await page
      .getByRole("button", { name: "Revoke Browser MCP client", exact: true })
      .click();
    await page.getByText("No connected apps.", { exact: true }).waitFor();
    const rejected = await context.request.post(origin + "/mcp", {
      headers: {
        Authorization: "Bearer " + token.access_token,
        Accept: "application/json, text/event-stream",
      },
      data: { jsonrpc: "2.0", id: 1, method: "tools/list" },
    });
    assert.equal(rejected.status(), 401);
    for (const path of [
      `/p/${pid}/integrations`,
      `/p/${pid}/onboarding`,
      `/oauth/consent?${new URLSearchParams(auth)}`,
      "/settings/connected-apps",
    ]) {
      await page.setViewportSize({ width: 390, height: 844 });
      await page.goto(origin + path);
      await page.waitForLoadState("networkidle");
      assert(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
        `mobile overflow: ${path}`,
      );
    }
    if (process.env.SCREENSHOT_PATH) {
      await page.goto(`${origin}/p/${pid}/integrations`);
      await page.waitForLoadState("networkidle");
      await page.screenshot({
        path: process.env.SCREENSHOT_PATH,
        fullPage: true,
      });
    }
    assert.deepEqual(errors, []);
    console.log(
      "PASS final M4 browser: reviewed model draft, GA4 connect/nine reports, IndexNow, link evidence, OAuth consent/exchange/revoke and mobile views",
    );
  } catch (e) {
    if (page)
      await page.screenshot({
        path: process.env.TEMP + "/seoseo-m4-browser/final-failure.png",
        fullPage: true,
      });
    throw e;
  } finally {
    await control("normal");
    if (pid) {
      await api(`/projects/${pid}/archive`, "POST");
      await api(`/projects/${pid}`, "DELETE");
    }
    await browser.close();
  }
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
