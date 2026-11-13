// Disposable clustering UI regression: WEB_ORIGIN M6CHECK_DB_CONTAINER OUTPUT_DIRECTORY.
const puppeteer = require(process.env.PUPPETEER_MODULE || "puppeteer-core");
const fs = require("node:fs/promises");
const path = require("node:path");
const { randomUUID } = require("node:crypto");
const { execFileSync } = require("node:child_process");

(async () => {
  const [origin, database, outputArg] = process.argv.slice(2);
  if (!database.startsWith("seoseo-m6check")) throw new Error("Use the disposable m6check database");
  const output = path.resolve(outputArg);
  await fs.mkdir(output, { recursive: true });
  const browser = await puppeteer.launch({ executablePath: process.env.CHROME_PATH || "C:/Program Files/Google/Chrome/Application/chrome.exe", headless: true });
  try {
    const page = await browser.newPage();
    const errors = [];
    let serpRequests = 0;
    page.on("pageerror", e => errors.push(String(e)));
    page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
    page.on("request", r => { if (r.url().includes("/serp-analysis")) serpRequests++; });
    await page.setViewport({ width: 1440, height: 1000 });
    const cdp = await page.createCDPSession();
    await cdp.send("Page.setDownloadBehavior", { behavior: "allow", downloadPath: output });
    async function button(text) {
      await page.waitForFunction(t => [...document.querySelectorAll("button")].some(b => b.textContent.trim() === t && !b.disabled), {}, text);
      await page.evaluate(t => [...document.querySelectorAll("button")].find(b => b.textContent.trim() === t && !b.disabled).click(), text);
    }
    async function contains(text) {
      await page.waitForFunction(t => document.body.textContent.replace(/\s+/g, " ").includes(t), {}, text);
    }
    await page.goto(origin + "/sign-up", { waitUntil: "networkidle0" });
    const project = await page.evaluate(async () => {
      async function post(route, body) {
        const r = await fetch("/api/v1" + route, { method: "POST", headers: { "Content-Type": "application/json", "x-seoseo-csrf": "1" }, body: JSON.stringify(body) });
        if (!r.ok) throw new Error(`setup ${r.status}: ${await r.text()}`);
        return r.json();
      }
      await post("/auth/sign-up", { email: `m6-ui-${crypto.randomUUID()}@example.com`, name: "Clustering browser", password: "disposable-m6-browser-password" });
      const project = await post("/projects", { name: "M6 clustering browser", domain: "example.com" });
      await post(`/projects/${project.id}/saved-keywords`, { keywords: ["alpha topic", "beta topic", "missing topic", "=topic"] });
      return project;
    });
    let sql = "";
    for (const keyword of ["alpha topic", "beta topic", "=topic"]) {
      const sid = randomUUID();
      sql += `INSERT INTO serp_snapshots(id,provider,exact,keyword,country,language) VALUES ('${sid}','fixture',true,'${keyword}','US','en');`;
      for (let n = 1; n <= 3; n++) sql += `INSERT INTO serp_items(snapshot_id,position,url,domain,title) VALUES ('${sid}',${n},'https://fixture.example/${n}','fixture.example','UI fixture');`;
    }
    execFileSync("docker", ["exec", "-i", database, "psql", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", "seoseo"], { input: sql, stdio: ["pipe", "pipe", "pipe"] });
    await page.goto(`${origin}/p/${project.id}/keywords`, { waitUntil: "networkidle0" });
    await button("Group keywords");
    await contains("1 group · 3 eligible keywords · 4 of 4 project keywords evaluated · threshold 3");
    await page.evaluate(() => { for (const d of document.querySelectorAll("details")) d.open = true; });
    await contains("Every pair shares at least 3 URLs");
    await contains("No snapshot from the last seven days");
    await fs.rm(path.join(output, "keyword-clusters.csv"), { force: true });
    await button("Export groups CSV");
    let csv;
    for (let i = 0; i < 50; i++) {
      try { csv = await fs.readFile(path.join(output, "keyword-clusters.csv"), "utf8"); break; } catch { await new Promise(r => setTimeout(r, 100)); }
    }
    if (!csv?.includes("'=topic") || !csv.includes("no_recent_snapshot")) throw new Error("CSV export or formula escaping failed");
    await page.screenshot({ path: path.join(output, "clusters-desktop.png"), fullPage: true });
    await page.setViewport({ width: 390, height: 844 });
    await page.screenshot({ path: path.join(output, "clusters-mobile.png"), fullPage: true });
    if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)) throw new Error("Mobile overflow");
    await page.focus("#cluster-minimum");
    await page.keyboard.down("Control"); await page.keyboard.press("A"); await page.keyboard.up("Control");
    await page.keyboard.type("4");
    await button("Group keywords");
    await contains("threshold 4");
    await contains("No groups meet this threshold");
    if (serpRequests !== 0) throw new Error("Grouping unexpectedly requested a SERP");
    if (errors.length) throw new Error(errors.join("\n"));
    console.log("PASS: grouping, threshold changes, coverage/missing evidence, CSV export/formula protection, no SERP calls, desktop/mobile layout, no browser errors");
    console.log(output);
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exitCode = 1; });
