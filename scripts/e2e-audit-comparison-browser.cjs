// Disposable SA-13 UI regression: WEB_ORIGIN M6CHECK_DB OUTPUT_DIRECTORY.
const puppeteer = require(process.env.PUPPETEER_MODULE || "puppeteer-core");
const fs = require("node:fs/promises");
const path = require("node:path");
const { randomUUID } = require("node:crypto");
const { execFileSync } = require("node:child_process");
(async () => {
  const [origin,database,outputArg]=process.argv.slice(2);
  if (!database.startsWith("seoseo-m6check")) throw new Error("Use disposable m6check DB");
  const output=path.resolve(outputArg); await fs.mkdir(output,{recursive:true});
  const browser=await puppeteer.launch({executablePath:process.env.CHROME_PATH||"C:/Program Files/Google/Chrome/Application/chrome.exe",headless:true});
  try {
    const page=await browser.newPage(), errors=[];
    page.on("pageerror",e=>errors.push(String(e)));
    page.on("console",m=>{if(m.type()==="error")errors.push(m.text());});
    await page.setViewport({width:1440,height:1100});
    async function button(text) {
      await page.waitForFunction(t=>[...document.querySelectorAll("button")].some(b=>b.textContent.trim()===t&&!b.disabled),{},text);
      await page.evaluate(t=>[...document.querySelectorAll("button")].find(b=>b.textContent.trim()===t&&!b.disabled).click(),text);
    }
    async function contains(text) {await page.waitForFunction(t=>document.body.textContent.replace(/\s+/g," ").includes(t),{},text);}
    async function screenshot(name) {
      await page.screenshot({path:path.join(output,name+"-desktop.png"),fullPage:true});
      await page.setViewport({width:390,height:844});
      await page.screenshot({path:path.join(output,name+"-mobile.png"),fullPage:true});
      if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw new Error(name+" mobile overflow");
      await page.setViewport({width:1440,height:1100});
    }
    await page.goto(origin+"/sign-up",{waitUntil:"networkidle0"});
    const project=await page.evaluate(async()=>{
      async function post(route,body){const r=await fetch("/api/v1"+route,{method:"POST",headers:{"Content-Type":"application/json","x-seoseo-csrf":"1"},body:JSON.stringify(body)});if(!r.ok)throw new Error(await r.text());return r.json();}
      await post("/auth/sign-up",{email:`sa13-ui-${crypto.randomUUID()}@example.com`,name:"Audit comparison browser",password:"disposable-SA13-browser-password"});
      return post("/projects",{name:"M6 audit comparisons",domain:"audit-fixture.example"});
    });
    const before=randomUUID(), after=randomUUID(), bpage=randomUUID(), apage=randomUUID(), missing=randomUUID();
    const cfg=JSON.stringify({max_pages:3,concurrency:1,delay_ms:0,respect_robots:true,render_js:false,lighthouse:false,exclude_paths:[]});
    const sql=`INSERT INTO audits(id,project_id,start_url,status,phase,config,health_score,created_at,finished_at) VALUES ('${before}','${project.id}','http://audit-fixture.example/','completed','done','${cfg}',70,now()-interval '1 day',now()-interval '1 day'),('${after}','${project.id}','http://audit-fixture.example/','completed','done','${cfg}',80,now(),now());
      INSERT INTO audit_pages(id,audit_id,url,url_hash,status_code,fetch_class,depth,title) VALUES ('${bpage}','${before}','http://audit-fixture.example/common','common',200,'html',1,'Old title'),('${apage}','${after}','http://audit-fixture.example/common','common',200,'html',1,'New title'),('${missing}','${before}','http://audit-fixture.example/missing','missing',200,'html',1,'Missing observation');
      INSERT INTO audit_issues(id,audit_id,page_id,issue_type,severity) VALUES ('${randomUUID()}','${before}','${bpage}','thin-content','warning'),('${randomUUID()}','${before}','${missing}','missing-title','warning');`;
    execFileSync("docker",["exec","-i",database,"psql","-v","ON_ERROR_STOP=1","-U","seoseo","-d","seoseo"],{input:sql,stdio:["pipe","pipe","pipe"]});
    await page.goto(`${origin}/p/${project.id}/audit/${after}`,{waitUntil:"networkidle0"});
    await button("Compare");
    await page.select('[aria-label="Comparison baseline"]',before);
    await button("Compare audits");
    await contains("1 changed · 1 only in baseline · 0 only in current");
    await contains("Unconfirmed absence: missing-title");
    await contains("No longer observed: thin-content");
    await screenshot("audit-comparison");
    await page.select('[aria-label="Comparison filter"]',"only_in_baseline");
    await contains("1 matching URLs");
    if(await page.$$eval("tbody tr",rows=>rows.length)!==1)throw new Error("Comparison filter failed");
    await page.goto(`${origin}/p/${project.id}/audit`,{waitUntil:"networkidle0"});
    await page.select('[aria-label="Audit schedule frequency"]',"weekly");
    await page.select('[aria-label="Schedule baseline audit"]',before);
    await contains("Page budget: 3 · Robots: respected");
    await button("Save audit schedule");
    await contains("Current schedule: weekly");
    await screenshot("audit-schedule");
    await page.select('[aria-label="Audit schedule frequency"]',"manual");
    await button("Save audit schedule");
    await contains("Current schedule: manual");
    if(errors.length)throw new Error(errors.join("\n"));
    console.log("PASS: comparison filters, unchanged vs unknown evidence, baseline selection, schedule review/save/disable, desktop/mobile and no browser errors");console.log(output);
  } finally {await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
