// Disposable M7 browser regression; requires the offline model fixture and optional local Ollama.
// PUPPETEER_MODULE=... node scripts/e2e-assistant-browser.cjs ORIGIN DB_CONTAINER OUTPUT [LOCAL_MODEL]
const puppeteer = require(process.env.PUPPETEER_MODULE || "puppeteer-core");
const fs = require("node:fs/promises");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { randomUUID } = require("node:crypto");

(async () => {
  const [origin, database, output, localModel] = process.argv.slice(2);
  if (!database.startsWith("seoseo-m7check")) throw new Error("Disposable M7 database required");
  await fs.mkdir(output, { recursive: true });
  const browser = await puppeteer.launch({ executablePath: process.env.CHROME_PATH || "C:/Program Files/Google/Chrome/Application/chrome.exe", headless: true });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on("response", r => { if (r.url().endsWith("/events")) console.log("SSE response", { encoding: r.headers()["content-encoding"], cache: r.headers()["cache-control"] }); });
    page.on("pageerror", e => errors.push(String(e)));
    await page.setViewport({ width: 1440, height: 1000 });
    async function button(text) {
      await page.waitForFunction(t => [...document.querySelectorAll("button")].some(b => b.textContent.trim() === t && !b.disabled), {}, text);
      await page.evaluate(t => [...document.querySelectorAll("button")].find(b => b.textContent.trim() === t && !b.disabled).click(), text);
    }
    async function contains(text, timeout = 90000) {
      await page.waitForFunction(t => document.body.textContent.includes(t), { timeout }, text);
    }
    async function screenshots(name) {
      await page.evaluate(() => window.scrollTo(0, 0));
      await page.screenshot({ path: path.join(output, name + "-desktop.png"), fullPage: true });
      await page.setViewport({ width: 390, height: 844 });
      await page.screenshot({ path: path.join(output, name + "-mobile.png"), fullPage: true });
      if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)) throw new Error(name + " mobile overflow");
      await page.setViewport({ width: 1440, height: 1000 });
    }
    async function text(id, value) {
      await page.focus(id); await page.keyboard.down("Control"); await page.keyboard.press("A"); await page.keyboard.up("Control"); await page.keyboard.type(value);
    }
    await page.goto(origin + "/sign-up", { waitUntil: "networkidle0" });
    const project = await page.evaluate(async () => {
      async function req(route, method, body) {
        const r = await fetch("/api/v1" + route, { method, headers: { "Content-Type": "application/json", "x-seoseo-csrf": "1" }, body: JSON.stringify(body) });
        if (!r.ok) throw new Error(`setup ${r.status}`); return r.json();
      }
      await req("/auth/sign-up", "POST", { email: `m7-browser-${crypto.randomUUID()}@example.com`, name: "M7 browser", password: "disposable-m7-browser-password" });
      const p = await req("/projects", "POST", { name: "M7 browser checks", domain: "ours.example" });
      await req(`/projects/${p.id}/context`, "PUT", { revision: 0, sections: [{ key: "business_overview", title: "Business overview", content: "We provide technical SEO audits for small businesses." }] });
      return p;
    });
    await page.goto(`${origin}/p/${project.id}/assistant`, { waitUntil: "networkidle0" });
    await contains("Chats are private");
    await button("New chat");
    await page.waitForSelector("#assistant-message");
    await page.type("#assistant-message", "SLOW What should I fix first?");
    await button("Send message");
    await contains("Fixture answer");
    await page.type("#assistant-message", "STREAM REMEMBER prioritize technical SEO audits");
    await button("Send message");
    await page.waitForSelector("[data-streaming-draft]", { timeout: 60000 });
    await screenshots("assistant-streaming-draft");
    await contains("Proposed memory note");
    const beforeReview = await page.evaluate(async (pid) => (await (await fetch(`/api/v1/projects/${pid}/context`)).json()).revision, project.id);
    if (beforeReview !== 1) throw new Error("Memory changed without approval");
    await button("Review memory note");
    await page.waitForSelector('textarea[id^="memory-note-"]');
    await text('textarea[id^="memory-note-"]', "Reviewed in the browser: focus on small business technical SEO audits.");
    await button("Save reviewed note");
    await contains("Saved to project memory");
    const afterReview = await page.evaluate(async (pid) => (await (await fetch(`/api/v1/projects/${pid}/context`)).json()), project.id);
    if (afterReview.revision !== 2 || afterReview.research_log.length !== 1 || !afterReview.research_log[0].summary.startsWith("Reviewed in the browser")) throw new Error("Reviewed note was not appended once");
    await screenshots("assistant-reviewed-memory");
    await page.reload({ waitUntil: "networkidle0" });
    // Refresh preserves the chat list; select the private conversation to restore history.
    await page.waitForFunction(() => document.querySelector("#assistant-chat")?.options.length > 1);
    const first = await page.$eval("#assistant-chat", el => el.options[1].value);
    await page.select("#assistant-chat", first);
    await contains("Fixture answer");
    await page.evaluate(() => { for (const d of document.querySelectorAll("details")) d.open = true; });
    await contains("project_memory");
    await screenshots("assistant-evidence");
    await page.type("#content-keyword", "missing keyword");
    await page.type("#content-draft", "A useful draft about technical SEO audits.");
    await button("Check term coverage");
    await contains("No score");
    const sid = randomUUID();
    execFileSync("docker", ["exec", "-i", database, "psql", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", "seoseo"], { input: `INSERT INTO serp_snapshots(id,provider,exact,keyword,country,language) VALUES('${sid}','browser_fixture',false,'seo audits','US','en'); INSERT INTO serp_items(snapshot_id,position,url,domain,title,snippet) VALUES('${sid}',1,'https://one.example/guide','one.example','SEO audits checklist','Technical advice'),('${sid}',2,'https://two.example/guide','two.example','SEO audits checklist','Technical advice');` });
    await text("#content-keyword", "seo audits");
    await text("#content-draft", "SEO audits checklist technical advice");
    await button("Check term coverage");
    await contains("Term coverage: 100%");
    await screenshots("content-coverage");
    await button("Generate content brief");
    await contains("Fixture answer from project_memory and cached_serp");
    await page.type("#assistant-message", "MALFORMED");
    await button("Send message");
    await contains("Model did not return the assistant action format");
    await screenshots("assistant-failure");
    if (localModel) {
      await page.select("#assistant-model", localModel);
      const previousChat = await page.$eval("#assistant-chat", e => e.value);
      await button("New chat");
      await page.waitForFunction(old => document.querySelector("#assistant-chat")?.value !== old && document.querySelector("#assistant-message") && !document.querySelector("#assistant-message").disabled, {}, previousChat);
      await page.type("#assistant-message", "Use project_memory to say what services this business provides. Do not call any more tools. Return a final answer in one sentence.");
      await button("Send message");
      await page.waitForFunction(() => document.querySelector("article")?.textContent.includes("completed") || document.querySelector("article")?.textContent.includes("failed"), { timeout: 120000 });
      const result = await page.$eval("article", e => e.textContent);
      if (!result.includes("completed")) throw new Error("Local model failed: " + result);
      await fs.writeFile(path.join(output, "local-model-result.txt"), result);
      await screenshots("assistant-local-ollama");
      console.log("PASS: real local model", localModel, result);
    }
    if (errors.length) throw new Error(errors.join("\n"));
    console.log("PASS: private assistant chat, explicit send, refresh/history, evidence, nullable/100% content coverage, queued brief, provider-format error, desktop/mobile and no browser errors", { projectId: project.id, output });
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exit(1); });
