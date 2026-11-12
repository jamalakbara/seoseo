// Disposable M5 UI regression. Requires native model + research fixtures.
// node scripts/e2e-m5-final-browser.cjs WEB_ORIGIN DB_CONTAINER RESEARCH_FIXTURE OUTPUT
const puppeteer = require(process.env.PUPPETEER_MODULE || "puppeteer-core");
const fs = require("node:fs/promises");
const path = require("node:path");
const { randomUUID } = require("node:crypto");
const { execFileSync } = require("node:child_process");

(async () => {
  const [origin, database, fixture, output] = process.argv.slice(2);
  await fs.mkdir(output, { recursive: true });
  const browser = await puppeteer.launch({ executablePath: process.env.CHROME_PATH || "C:/Program Files/Google/Chrome/Application/chrome.exe", headless: true });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", e => errors.push(String(e)));
    await page.setViewport({ width: 1440, height: 1000 });
    async function button(text) {
      await page.waitForFunction(t => [...document.querySelectorAll("button")].some(b => b.textContent.trim() === t && !b.disabled), {}, text);
      await page.evaluate(t => [...document.querySelectorAll("button")].find(b => b.textContent.trim() === t && !b.disabled).click(), text);
    }
    async function contains(text, timeout = 60000) {
      await page.waitForFunction(t => document.body.textContent.replace(/\s+/g, " ").includes(t), { timeout }, text);
    }
    async function screenshots(name) {
      await page.screenshot({ path: path.join(output, name + "-desktop.png"), fullPage: true });
      await page.setViewport({ width: 390, height: 844 });
      await page.screenshot({ path: path.join(output, name + "-mobile.png"), fullPage: true });
      if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)) throw new Error(name + " mobile overflow");
      await page.setViewport({ width: 1440, height: 1000 });
    }
    await page.goto(origin + "/sign-up", { waitUntil: "networkidle0" });
    const project = await page.evaluate(async () => {
      async function post(route, body) {
        const r = await fetch("/api/v1" + route, { method: "POST", headers: { "Content-Type": "application/json", "x-seoseo-csrf": "1" }, body: JSON.stringify(body) });
        if (!r.ok) throw new Error(`setup ${r.status}`);
        return r.json();
      }
      await post("/auth/sign-up", { email: `m5-ui-${crypto.randomUUID()}@example.com`, name: "M5 browser", password: "disposable-m5-browser-password" });
      return post("/projects", { name: "M5 browser checks", domain: "example.com" });
    });
    await page.goto(`${origin}/p/${project.id}/ai`, { waitUntil: "networkidle0" });
    await page.type("#ai-topic", "SEO software");
    await button("Generate question drafts");
    await button("Use these questions");
    if (!(await page.$eval("#ai-prompts", el => el.value)).includes("technical SEO")) throw new Error("Drafts not applied");
    await page.type("#ai-name", "Native comparison");
    await page.type("#ai-brands", "Acme\nRival");
    await page.focus("#ai-prompts");
    await page.keyboard.down("Control"); await page.keyboard.press("A"); await page.keyboard.up("Control");
    await page.keyboard.type("Which tools are useful?");
    await page.evaluate(() => {
      for (const label of document.querySelectorAll("fieldset label")) {
        if (["fixture-a", "anthropic/fixture-claude", "gemini/fixture-gemini"].includes(label.textContent.trim())) label.querySelector("input").click();
      }
    });
    await button("Create tracker");
    await button("Run model comparison");
    await contains("3 successful / 3 expected answers");
    await contains("sources.example: 1");
    await page.select('select[aria-label="AI schedule"]', "daily");
    await button("Save schedule");
    await contains("Current: daily");
    await page.select('select[aria-label="AI schedule"]', "manual");
    await button("Save schedule");
    await contains("Current: manual");
    await page.evaluate(() => { for (const d of document.querySelectorAll("details")) d.open = true; });
    await contains("Engine: anthropic"); await contains("Grounding: provider_citations");
    await contains("Engine: gemini");
    if (!await page.$('a[href="https://sources.example/anthropic"]')) throw new Error("Provider citation link missing");
    await screenshots("m5-ai");

    await fetch(fixture + "/control", { method: "POST", body: JSON.stringify({ mode: "baseline" }) });
    await page.goto(`${origin}/p/${project.id}/backlinks`, { waitUntil: "networkidle0" });
    await page.type("#discovery-urls", "http://source-one.example/page\nhttp://source-two.example/page");
    await button("Start backlink discovery");
    await contains("completed"); await contains("serp_crawl"); await contains("verified");
    await contains("Risk: 40");
    await screenshots("m5-backlinks");

    const sid = randomUUID();
    const sql = `INSERT INTO serp_snapshots(id,provider,exact,keyword,country,language,fetched_at) VALUES ('${sid}','fixture',true,'ui m5 keyword','US','en',now()); INSERT INTO serp_items(snapshot_id,position,url,domain,title) VALUES ('${sid}',1,'https://competitor.com/page','competitor.com','UI fixture');`;
    execFileSync("docker", ["exec", "-i", database, "psql", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", "seoseo"], { input: sql, stdio: ["pipe", "pipe", "pipe"] });
    await page.goto(`${origin}/p/${project.id}/competitive`, { waitUntil: "networkidle0" });
    await page.type("#volume-source", "Synthetic UI counts, test month");
    await page.type("#volume-rows", JSON.stringify([{ keyword: "ui m5 keyword", monthly_search_volume: 1000 }]));
    await button("Import search volumes"); await contains("Search volumes imported.");
    await page.focus("#research-domain");
    await page.keyboard.down("Control"); await page.keyboard.press("A"); await page.keyboard.up("Control");
    await page.keyboard.type("competitor.com");
    await button("Inspect domain");
    await contains("280.0 estimated clicks"); await contains("Eligible volume coverage 1/1");
    await screenshots("m5-competitive");
    if (errors.length) throw new Error(errors.join("\n"));
    console.log("PASS: question draft review/apply, native answers/citation links/trends, schedule save/disable, discovery polling/verification/risk, volume import/estimate; desktop/mobile and no browser errors");
    console.log(output);
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exitCode = 1; });
