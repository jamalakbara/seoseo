// Disposable headless UI test. Configure the API/worker with ai-model-fixture.py.
// PUPPETEER_MODULE can point at a temporary puppeteer-core installation.
const puppeteer=require(process.env.PUPPETEER_MODULE||"puppeteer-core");
const path=require("node:path");const fs=require("node:fs/promises");
(async()=>{
  const origin=process.argv[2],output=process.argv[3];await fs.mkdir(output,{recursive:true});
  const browser=await puppeteer.launch({executablePath:process.env.CHROME_PATH||"C:/Program Files/Google/Chrome/Application/chrome.exe",headless:true});
  try{
    const page=await browser.newPage();const errors=[];page.on("pageerror",e=>errors.push(String(e)));await page.setViewport({width:1440,height:1000});
    await page.goto(origin+"/sign-up",{waitUntil:"networkidle0"});
    const project=await page.evaluate(async()=>{
      async function api(path,body){const r=await fetch("/api/v1"+path,{method:"POST",headers:{"Content-Type":"application/json","x-seoseo-csrf":"1"},body:JSON.stringify(body)});if(!r.ok)throw new Error(`setup failed ${r.status}`);return r.json();}
      await api("/auth/sign-up",{email:`ai-ui-${crypto.randomUUID()}@example.com`,password:"disposable-ai-browser-password",name:"AI UI"});
      return api("/projects",{name:"AI answer comparisons UI",domain:"example.com"});
    });
    await page.goto(`${origin}/p/${project.id}/ai`,{waitUntil:"networkidle0"});await page.waitForSelector("#ai-name");
    await page.type("#ai-name","Fixture comparison");await page.type("#ai-brands","Acme\nRival");await page.type("#ai-prompts","Which tools are useful?");
    const boxes=await page.$$('fieldset input[type="checkbox"]');if(boxes.length!==3)throw new Error("Expected three approved models");for(const box of boxes)await box.click();
    await page.click('button[type="submit"]');
    await page.waitForFunction(()=>[...document.querySelectorAll("button")].some(b=>b.textContent==="Run model comparison"));
    await page.evaluate(()=>[...document.querySelectorAll("button")].find(b=>b.textContent==="Run model comparison").click());
    await page.waitForFunction(()=>document.body.textContent.replace(/\s+/g," ").includes("3 successful / 3 expected answers"),{timeout:60000});
    if(!await page.evaluate(()=>document.body.textContent.includes("33.3%")&&document.body.textContent.includes("66.7%")))throw new Error("Mention rates missing");
    await page.evaluate(()=>{for(const details of document.querySelectorAll("details"))details.open=true;});
    await page.screenshot({path:path.join(output,"ai-desktop.png"),fullPage:true});await page.setViewport({width:390,height:844});
    await page.screenshot({path:path.join(output,"ai-mobile.png"),fullPage:true});
    if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw new Error("Mobile page overflow");
    if(errors.length)throw new Error(errors.join("\n"));
    console.log("PASS: UI tracker creation, 3-model queued run/polling, literal rates, answer viewer, desktop/mobile layout and no browser errors");console.log(output);
  }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
