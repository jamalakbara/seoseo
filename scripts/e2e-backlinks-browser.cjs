// Headless UI smoke test against a disposable password-auth API with open signup.
// Requires puppeteer-core (PUPPETEER_MODULE may point to a temporary installation).
// node scripts/e2e-backlinks-browser.cjs http://127.0.0.1:3400 OUTPUT_DIRECTORY
const puppeteer = require(process.env.PUPPETEER_MODULE || "puppeteer-core");
const path = require("node:path");
const fs = require("node:fs/promises");

(async () => {
  const origin = process.argv[2];
  const output = process.argv[3];
  await fs.mkdir(output, { recursive: true });
  const browser = await puppeteer.launch({
    executablePath: process.env.CHROME_PATH || "C:/Program Files/Google/Chrome/Application/chrome.exe",
    headless: true,
  });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", e => errors.push(String(e)));
    await page.setViewport({ width: 1440, height: 1000 });
    await page.goto(origin + "/sign-up", { waitUntil: "networkidle0" });
    const project = await page.evaluate(async () => {
      async function api(path, body) {
        const r = await fetch("/api/v1" + path, { method: "POST", headers: { "Content-Type": "application/json", "x-seoseo-csrf": "1" }, body: JSON.stringify(body) });
        if (!r.ok) throw new Error(`Fixture setup failed: ${r.status}`);
        return r.json();
      }
      await api("/auth/sign-up", { email: `backlink-ui-${crypto.randomUUID()}@example.com`, password: "disposable-browser-regression-password", name: "Backlink UI" });
      return api("/projects", { name: "Backlink evidence UI", domain: "example.com" });
    });
    await page.goto(`${origin}/p/${project.id}/backlinks`, { waitUntil: "networkidle0" });
    await page.waitForSelector("#backlink-json");
    const row = name => ({ source_url: `https://${name}.org/page`, target_url: "https://example.com/target", anchor: name, nofollow: null });
    async function importRows(rows, expected) {
      await page.click("#backlink-json");
      await page.keyboard.down("Control");
      await page.keyboard.press("KeyA");
      await page.keyboard.up("Control");
      await page.keyboard.press("Backspace");
      await page.type("#backlink-json", JSON.stringify(rows));
      await page.click('input[type="checkbox"][required]');
      await page.click('button[type="submit"]');
      try {
        await page.waitForFunction(text => document.body.textContent.replace(/\s+/g, " ").includes(text), {}, expected);
      } catch (error) {
        console.error(await page.evaluate(() => document.body.innerText));
        await page.screenshot({ path: path.join(output, "backlinks-failure.png"), fullPage: true });
        throw error;
      }
    }
    await importRows([row("one"), row("two")], "2 in sample · 0 new");
    await importRows([row("one"), row("three")], "2 in sample · 1 new");
    await page.evaluate(() => [...document.querySelectorAll("button")].find(b => b.textContent === "Absent from sample").click());
    await page.waitForFunction(() => [...document.querySelectorAll("td")].some(td => td.textContent === "https://two.org/page"));
    await page.screenshot({ path: path.join(output, "backlinks-desktop.png"), fullPage: true });
    await page.setViewport({ width: 390, height: 844 });
    await page.screenshot({ path: path.join(output, "backlinks-mobile.png"), fullPage: true });
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
    if (overflow) throw new Error("Mobile page overflows the viewport");
    if (errors.length) throw new Error(errors.join("\n"));
    console.log("PASS: UI import baseline/change, sample-state filtering, desktop/mobile rendering, no page errors or page overflow");
    console.log(output);
  } finally {
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
