// Desktop/mobile checks on the disposable M8 stack, plus a sandboxed MCP Apps host.
const puppeteer = require(
  process.env.PUPPETEER_MODULE || "../renderer/node_modules/puppeteer-core",
);
const fs = require("node:fs/promises");
const path = require("node:path");
(async () => {
  const origin = process.argv[2] || "http://localhost:3600";
  if (!/^http:\/\/(localhost|127\.0\.0\.1):3600$/.test(origin))
    throw Error("Disposable M8 web origin required");
  const output = path.resolve(
    process.argv[3] || "F:/projects/seoseo-m8-evidence",
  );
  await fs.mkdir(output, { recursive: true });
  const browser = await puppeteer.launch({
    executablePath:
      process.env.CHROME_PATH ||
      "C:/Program Files/Google/Chrome/Application/chrome.exe",
    headless: true,
  });
  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 1440, height: 1000 });
    const errors = [];
    page.on("pageerror", (e) => errors.push(String(e)));
    async function contains(text) {
      await page.waitForFunction(
        (t) => document.body.textContent.includes(t),
        { timeout: 30000 },
        text,
      );
    }
    async function button(text) {
      const h = await page.evaluateHandle(
        (t) =>
          [...document.querySelectorAll("button")].find(
            (b) => b.textContent.trim() === t && !b.disabled,
          ),
        text,
      );
      if (!h.asElement()) throw Error("Missing button " + text);
      await h.asElement().click();
      await h.dispose();
    }
    async function shots(name) {
      await page.evaluate(() => scrollTo(0, 0));
      await page.screenshot({
        path: path.join(output, name + "-desktop.png"),
        fullPage: true,
      });
      await page.setViewport({ width: 390, height: 844 });
      await page.screenshot({
        path: path.join(output, name + "-mobile.png"),
        fullPage: true,
      });
      if (
        await page.evaluate(
          () => document.documentElement.scrollWidth > innerWidth,
        )
      )
        throw Error(name + " mobile overflow");
      await page.setViewport({ width: 1440, height: 1000 });
    }
    await page.goto(origin + "/tools", { waitUntil: "networkidle0" });
    await contains("Free SEO tools");
    await button("Analyze");
    await contains('"local_templates"');
    await page.select("select:not([aria-label])", "serp-preview");
    await button("Analyze");
    await contains("Fresh coffee, delivered");
    await shots("public-tools");
    await page.select("select[aria-label]", "id");
    await contains("Alat SEO gratis");
    await page.reload({ waitUntil: "networkidle0" });
    await contains("Alat SEO gratis");
    if ((await page.evaluate(() => document.documentElement.lang)) !== "id")
      throw Error("HTML locale not set");
    await page.select("select[aria-label]", "en");
    await page.goto(origin + "/docs", { waitUntil: "networkidle0" });
    await contains("Documentation");
    await shots("docs-index");
    await page.goto(origin + "/docs/07-roadmap", { waitUntil: "networkidle0" });
    await contains("Roadmap");
    await page.waitForSelector("table");
    await shots("docs-roadmap");
    // Set up a fresh disposable identity using the actual web proxy and cookie flow.
    const project = await page.evaluate(async () => {
      async function call(url, body) {
        const r = await fetch("/api/v1" + url, {
          method: "POST",
          headers: { "content-type": "application/json", "x-seoseo-csrf": "1" },
          body: JSON.stringify(body),
        });
        if (!r.ok) throw Error("setup " + r.status);
        return r.json();
      }
      await call("/auth/sign-up", {
        email: `m8-browser-${crypto.randomUUID()}@example.invalid`,
        name: "M8 browser",
        password: "disposable-m8-browser-password",
      });
      return call("/projects", {
        name: "M8 browser acceptance",
        domain: "store.example.com",
      });
    });
    await page.goto(`${origin}/p/${project.id}/extras`, {
      waitUntil: "networkidle0",
    });
    await contains("No active signed credential");
    await contains("Unknown");
    const raw =
      '203.0.113.2 - - [10/Oct/2000:13:55:36 -0700] "GET /secret?token=private HTTP/1.1" 200 20 "-" "Bingbot"';
    await page.type("textarea", raw);
    await button("Analyze log");
    await contains("Log summary saved");
    await page.evaluate(() =>
      [...document.querySelectorAll("details")]
        .find((d) => d.textContent.includes("uploaded_access_log"))
        .querySelector("summary")
        .click(),
    );
    await contains("declared_agents");
    await shots("project-extras");
    const lastTextarea = (await page.$$("textarea")).at(-1);
    await lastTextarea.type("coffee");
    await button("Read historical metrics");
    await contains('"available": false');
    await page.select("select[aria-label]", "id");
    await contains("Alat tambahan");
    await page.reload({ waitUntil: "networkidle0" });
    await contains("Pelacakan peringkat");
    await shots("project-indonesian");
    await page.select("select[aria-label]", "en");
    // Model a host bridge: only parent-origin messages reach the sandboxed resource.
    const widgetHtml = await fs.readFile(
      path.join(__dirname, "../backend/crates/mcp/src/evidence-widget.html"),
      "utf8",
    );
    await page.goto("about:blank");
    await page.setContent(
      '<!doctype html><html><body><iframe title="MCP evidence" sandbox="allow-scripts" style="width:100%;height:700px;border:0"></iframe></body></html>',
    );
    await page.evaluate((html) => {
      const frame = document.querySelector("iframe");
      window.addEventListener("message", (e) => {
        if (e.source !== frame.contentWindow) return;
        if (e.data.method === "ui/initialize") {
          frame.contentWindow.postMessage(
            {
              jsonrpc: "2.0",
              id: e.data.id,
              result: {
                protocolVersion: "2026-01-26",
                hostInfo: { name: "test-host", version: "1" },
                hostCapabilities: {},
              },
            },
            "*",
          );
        }
        if (e.data.method === "ui/notifications/initialized")
          frame.contentWindow.postMessage(
            {
              jsonrpc: "2.0",
              method: "ui/notifications/tool-result",
              params: {
                structuredContent: {
                  provider: "fixture",
                  exact: true,
                  country: "US",
                  language: "en",
                  items: [
                    {
                      position: 1,
                      title: '<img src=x onerror="alert(1)">',
                      url: "https://example.com/",
                      sr: null,
                    },
                    {
                      position: 2,
                      title: "Another result",
                      url: "https://other.example/",
                      sr: 12,
                    },
                  ],
                },
              },
            },
            "*",
          );
      });
      frame.srcdoc = html;
    }, widgetHtml);
    const iframe = await page.waitForSelector("iframe");
    const frame = await iframe.contentFrame();
    await frame.waitForFunction(
      () => document.querySelectorAll("tbody tr").length === 2,
    );
    if ((await frame.$$("img")).length)
      throw Error("Untrusted widget markup executed");
    await frame.type("#filter", "Another");
    await frame.waitForFunction(
      () => document.querySelectorAll("tbody tr").length === 1,
    );
    await page.screenshot({
      path: path.join(output, "mcp-widget.png"),
      fullPage: true,
    });
    if (errors.length) throw Error(errors.join("\n"));
    console.log(
      "PASS: anonymous tools, SERP preview, docs/tables, log summary, CPC unknowns, persisted en/id navigation, desktop/mobile, MCP handshake/filter/XSS isolation",
      { output, project: project.id },
    );
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
