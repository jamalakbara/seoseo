// Disposable graph UI regression: WEB_ORIGIN M6CHECK_DB_CONTAINER OUTPUT_DIRECTORY.
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
    let crawlRequests = 0;
    page.on("pageerror", e => errors.push(String(e)));
    page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
    page.on("request", r => { if (r.method() === "POST" && /\/audits(?:\?|$)/.test(r.url())) crawlRequests++; });
    await page.setViewport({ width: 1440, height: 1100 });
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
      await post("/auth/sign-up", { email: `graph-ui-${crypto.randomUUID()}@example.com`, name: "Link graph browser", password: "disposable-link-graph-browser-password" });
      return post("/projects", { name: "M6 internal links browser", domain: "example.com" });
    });
    const aid = randomUUID();
    const nodes = Object.fromEntries(["", "seo-crawling", "seo-auditing", "contact"].map(name => [name, randomUUID()]));
    let sql = `INSERT INTO audits(id,project_id,start_url,status,phase,pages_crawled,pages_total,finished_at) VALUES ('${aid}','${project.id}','https://example.com/','completed','done',4,4,now());`;
    for (const [name, id] of Object.entries(nodes)) {
      const title = name === "" ? "Home" : name === "contact" ? "Contact us" : "Technical SEO auditing";
      sql += `INSERT INTO audit_pages(id,audit_id,url,url_hash,status_code,fetch_class,depth,title) VALUES ('${id}','${aid}','https://example.com/${name}','${id}',200,'html',1,'${title}');`;
    }
    sql += `INSERT INTO audit_links(audit_id,from_page_id,to_url_hash,to_url,nofollow) VALUES ('${aid}','${nodes[""]}','${nodes["seo-crawling"]}','https://example.com/seo-crawling',false),('${aid}','${nodes["contact"]}','${nodes["seo-auditing"]}','https://example.com/seo-auditing',true);`;
    execFileSync("docker", ["exec", "-i", database, "psql", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", "seoseo"], { input: sql, stdio: ["pipe", "pipe", "pipe"] });
    await page.goto(`${origin}/p/${project.id}/audit/${aid}`, { waitUntil: "networkidle0" });
    await button("Internal links");
    await contains("4 of 4 stored pages · 2 of 2 edges between selected pages · 2 link suggestions");
    await page.waitForSelector('svg[aria-label="Internal link neighborhood"]');
    await page.evaluate(() => document.querySelector('g[aria-label="Select https://example.com/seo-crawling"]').dispatchEvent(new MouseEvent("click", { bubbles: true })));
    await page.waitForFunction(() => document.querySelector('[aria-label="Selected page details"]').textContent.includes("1 incoming source pages · 1 followed sources"));
    // The table selects a different page, including its nofollow-only incoming edge.
    await page.evaluate(() => [...document.querySelectorAll("td button")].find(b => b.textContent === "https://example.com/seo-auditing").click());
    await page.waitForSelector('line[stroke-dasharray="5 4"]');
    await page.waitForFunction(() => document.querySelector('[aria-label="Selected page details"]').textContent.includes("1 incoming source pages · 0 followed sources"));
    await page.type('[aria-label="Filter graph pages"]', "seo-crawling");
    await page.waitForFunction(() => document.querySelectorAll("tbody tr").length === 1);
    await page.focus('[aria-label="Filter graph pages"]');
    await page.keyboard.down("Control"); await page.keyboard.press("A"); await page.keyboard.up("Control"); await page.keyboard.press("Backspace");
    await page.focus("#graph-limit");
    await page.keyboard.down("Control"); await page.keyboard.press("A"); await page.keyboard.up("Control"); await page.keyboard.type("10");
    await button("Load graph");
    await contains("2 link suggestions");
    await fs.rm(path.join(output, "internal-link-graph.json"), { force: true });
    await button("Export graph JSON");
    let exported;
    for (let i = 0; i < 50; i++) {
      try { exported = JSON.parse(await fs.readFile(path.join(output, "internal-link-graph.json"), "utf8")); break; } catch { await new Promise(r => setTimeout(r, 100)); }
    }
    if (exported?.nodes.length !== 4 || exported.suggestions.length !== 2) throw new Error("Graph export failed");
    await page.screenshot({ path: path.join(output, "internal-links-desktop.png"), fullPage: true });
    await page.setViewport({ width: 390, height: 844 });
    await page.screenshot({ path: path.join(output, "internal-links-mobile.png"), fullPage: true });
    if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)) throw new Error("Mobile overflow");
    if (crawlRequests) throw new Error("Graph unexpectedly started a crawl");
    if (errors.length) throw new Error(errors.join("\n"));
    console.log("PASS: audit tab, interactive SVG/table selection, full-crawl counts, nofollow style, page filter, limit change, JSON export, suggestions, desktop/mobile layout and no crawl/browser errors");
    console.log(output);
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exitCode = 1; });
