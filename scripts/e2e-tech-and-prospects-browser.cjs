// Disposable DO-06/BL-07 UI regression: WEB_ORIGIN M6CHECK_DB OUTPUT_DIRECTORY.
const puppeteer=require(process.env.PUPPETEER_MODULE||"puppeteer-core"),fs=require("node:fs/promises"),path=require("node:path");
(async()=>{
  const [origin,database,outputArg]=process.argv.slice(2);if(!database.startsWith("seoseo-m6check"))throw new Error("Use disposable m6check DB");
  const output=path.resolve(outputArg);await fs.mkdir(output,{recursive:true});
  const browser=await puppeteer.launch({executablePath:process.env.CHROME_PATH||"C:/Program Files/Google/Chrome/Application/chrome.exe",headless:true});
  try{
    const page=await browser.newPage(),errors=[];let discovery=0;
    page.on("pageerror",e=>errors.push(String(e)));page.on("console",m=>{if(m.type()==="error")errors.push(m.text());});
    page.on("request",r=>{if(r.method()==="POST"&&r.url().includes("/backlink-discoveries"))discovery++;});
    await page.setViewport({width:1440,height:1100});
    async function button(text){await page.waitForFunction(t=>[...document.querySelectorAll("button")].some(b=>b.textContent.trim()===t&&!b.disabled),{},text);await page.evaluate(t=>[...document.querySelectorAll("button")].find(b=>b.textContent.trim()===t&&!b.disabled).click(),text);}
    async function contains(text){await page.waitForFunction(t=>document.body.textContent.replace(/\s+/g," ").includes(t),{},text);}
    async function screenshot(name){await page.screenshot({path:path.join(output,name+"-desktop.png"),fullPage:true});await page.setViewport({width:390,height:844});await page.screenshot({path:path.join(output,name+"-mobile.png"),fullPage:true});if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw new Error(name+" mobile overflow");await page.setViewport({width:1440,height:1100});}
    await page.goto(origin+"/sign-up",{waitUntil:"networkidle0"});
    const setup=await page.evaluate(async()=>{
      async function post(route,body){const r=await fetch("/api/v1"+route,{method:"POST",headers:{"Content-Type":"application/json","x-seoseo-csrf":"1"},body:JSON.stringify(body)});if(!r.ok)throw new Error(await r.text());return r.json();}
      await post("/auth/sign-up",{email:`tech-links-ui-${crypto.randomUUID()}@example.com`,name:"Technology and prospect browser",password:"disposable-tech-links-browser-password"});
      const own=await post("/projects",{name:"M6 technology and prospects",domain:"ours.com"});
      const one=await post("/projects",{name:"UI competitor one",domain:"rival-one.com"}),two=await post("/projects",{name:"UI competitor two",domain:"rival-two.com"});
      for(const p of [own,one,two])await post(`/projects/${p.id}/backlink-snapshots`,{expected_domain:p.domain,dataset:"ui-sample",complete_sample:true,links:p.id===own.id?[]:[{source_url:"https://shared-publisher.org/article",target_url:`https://${p.domain}/page`}]});
      return {own,one,two};
    });
    await page.goto(`${origin}/p/${setup.own.id}/competitive`,{waitUntil:"networkidle0"});
    await page.focus('[aria-label="Technology homepage URL"]');await page.keyboard.down("Control");await page.keyboard.press("A");await page.keyboard.up("Control");await page.keyboard.type("http://good.tech-fixture.example/");
    await button("Inspect homepage technology");await contains("WordPress · CMS");await contains("Next.js · Framework");await contains("Shopify · Commerce");await screenshot("homepage-technology");
    await page.goto(`${origin}/p/${setup.own.id}/backlinks`,{waitUntil:"networkidle0"});
    await page.click('[aria-label="Compare backlinks for UI competitor one"]');await page.click('[aria-label="Compare backlinks for UI competitor two"]');
    await page.focus('[aria-label="Minimum link competitors"]');await page.keyboard.down("Control");await page.keyboard.press("A");await page.keyboard.up("Control");await page.keyboard.type("2");
    await button("Find link prospects");await contains("1 source-domain prospects · minimum 2 competitors");await contains("shared-publisher.org");await contains("2 imported observations");
    await page.evaluate(()=>{for(const d of document.querySelectorAll("details"))if(d.querySelector("summary")?.textContent.includes("Sample scope"))d.open=true;});
    await screenshot("link-intersect");
    if(discovery)throw new Error("Read-only link intersect started discovery");if(errors.length)throw new Error(errors.join("\n"));
    console.log("PASS: explicit homepage hints/cache, competitor selection, minimum coverage, source provenance/scope, no discovery, desktop/mobile and no browser errors");console.log(output);
  }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
