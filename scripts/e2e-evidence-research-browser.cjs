// Disposable RT-07/AI-05 UI regression: WEB_ORIGIN M6CHECK_DB OUTPUT_DIRECTORY.
const puppeteer=require(process.env.PUPPETEER_MODULE||"puppeteer-core");
const fs=require("node:fs/promises"),path=require("node:path");
const {randomUUID}=require("node:crypto"),{execFileSync}=require("node:child_process");
(async()=>{
  const [origin,database,outputArg]=process.argv.slice(2);if(!database.startsWith("seoseo-m6check"))throw new Error("Use disposable m6check DB");
  const output=path.resolve(outputArg);await fs.mkdir(output,{recursive:true});
  const browser=await puppeteer.launch({executablePath:process.env.CHROME_PATH||"C:/Program Files/Google/Chrome/Application/chrome.exe",headless:true});
  try{
    const page=await browser.newPage(),errors=[];let paidCalls=0;
    page.on("pageerror",e=>errors.push(String(e)));page.on("console",m=>{if(m.type()==="error")errors.push(m.text());});
    page.on("request",r=>{if(r.method()==="POST"&&(r.url().includes("serp-analysis")||/ai-trackers\/[^/]+\/runs/.test(r.url())))paidCalls++;});
    await page.setViewport({width:1440,height:1100});
    async function button(text){await page.waitForFunction(t=>[...document.querySelectorAll("button")].some(b=>b.textContent.trim()===t&&!b.disabled),{},text);await page.evaluate(t=>[...document.querySelectorAll("button")].find(b=>b.textContent.trim()===t&&!b.disabled).click(),text);}
    async function contains(text){await page.waitForFunction(t=>document.body.textContent.replace(/\s+/g," ").includes(t),{},text);}
    async function screenshot(name){await page.screenshot({path:path.join(output,name+"-desktop.png"),fullPage:true});await page.setViewport({width:390,height:844});await page.screenshot({path:path.join(output,name+"-mobile.png"),fullPage:true});if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw new Error(name+" mobile overflow");await page.setViewport({width:1440,height:1100});}
    await page.goto(origin+"/sign-up",{waitUntil:"networkidle0"});
    const project=await page.evaluate(async()=>{async function post(route,body){const r=await fetch("/api/v1"+route,{method:"POST",headers:{"Content-Type":"application/json","x-seoseo-csrf":"1"},body:JSON.stringify(body)});if(!r.ok)throw new Error(await r.text());return r.json();}await post("/auth/sign-up",{email:`ev-ui-${crypto.randomUUID()}@example.com`,name:"Evidence research browser",password:"disposable-evidence-browser-password"});const p=await post("/projects",{name:"M6 evidence research",domain:"example.com"});await post(`/projects/${p.id}/saved-keywords`,{keywords:["ui competing urls"]});return p;});
    const sid=randomUUID(),tid=randomUUID(),rid=randomUUID();
    const cfg=JSON.stringify({brands:["Acme"],prompts:["Which technical SEO tools?","Unread prompt?"],models:["fixture-a","fixture-b"],country:"US",language:"en"});
    const sql=`INSERT INTO serp_snapshots(id,provider,exact,keyword,country,language) VALUES ('${sid}','fixture',false,'ui competing urls','US','en');INSERT INTO serp_items(snapshot_id,position,url,domain,title) VALUES ('${sid}',1,'https://example.com/guide','example.com','Guide'),('${sid}',4,'https://example.com/blog','example.com','Blog');
      INSERT INTO ai_visibility_trackers(id,project_id,domain,name,config) VALUES ('${tid}','${project.id}','example.com','UI evidence','${cfg}');INSERT INTO ai_visibility_runs(id,tracker_id,config,gateway_id,status) VALUES ('${rid}','${tid}','${cfg}','fixture-gateway','failed');
      INSERT INTO ai_visibility_answers(run_id,prompt_index,model,answer,mentions,sources) VALUES ('${rid}',0,'fixture-a','Acme is mentioned in this synthetic saved answer.','["Acme"]','[]');INSERT INTO ai_visibility_answers(run_id,prompt_index,model,error) VALUES ('${rid}',0,'fixture-b','Synthetic provider failure');`;
    execFileSync("docker",["exec","-i",database,"psql","-v","ON_ERROR_STOP=1","-U","seoseo","-d","seoseo"],{input:sql,stdio:["pipe","pipe","pipe"]});
    await page.goto(`${origin}/p/${project.id}/rank`,{waitUntil:"networkidle0"});await button("Find competing URLs");await contains("1 flagged keywords · 1 checked of 1 sampled");await contains("#4 · https://example.com/blog");await screenshot("cannibalization");
    await page.goto(`${origin}/p/${project.id}/ai`,{waitUntil:"networkidle0"});await page.type('[aria-label="AI index brand"]',"Acme");await button("Search saved AI evidence");await contains("1 successful / 4 expected answers · 1 failed · 2 unchecked");await contains("100.0% of successful answers");await screenshot("ai-prompt-index");
    await page.select('[aria-label="AI index view"]',"answers");await button("Search saved AI evidence");await contains("1 matching answers");await page.evaluate(()=>{for(const d of document.querySelectorAll("details"))d.open=true;});await contains("Acme is mentioned in this synthetic saved answer.");await screenshot("ai-brand-lookup");
    if(paidCalls)throw new Error("Read-only views triggered provider/model requests");if(errors.length)throw new Error(errors.join("\n"));
    console.log("PASS: multi-URL positions/provider labels, own prompt/brand search, successful/failure/unchecked rates, excerpts, no new calls, desktop/mobile and no browser errors");console.log(output);
  }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
