// Disposable-stack dashboard/onboarding UI regression. Requires playwright-core + Chrome.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || "playwright-core");
const assert = require("node:assert/strict");
const { randomUUID } = require("node:crypto");
(async () => {
  const origin = process.argv[2] || "http://127.0.0.1:3200";
  const browser = await chromium.launch({executablePath: process.env.CHROME_PATH || "C:/Program Files/Google/Chrome/Application/chrome.exe", headless:true});
  const context = await browser.newContext({viewport:{width:1280,height:900}});
  let pid, page;
  async function api(path,method="GET",data) {
    const response=await context.request.fetch(origin+"/api/v1"+path,{method,data,headers:{"x-seoseo-csrf":"1"}});
    assert(response.ok(),`${method} ${path}: ${response.status()} ${await response.text()}`);
    return response.status()===204?null:response.json();
  }
  try {
    await api("/auth/sign-up","POST",{email:`dashboard-ui-${randomUUID()}@example.com`,password:"disposable-dashboard-browser-password",name:"Dashboard browser"});
    page=await context.newPage(); const errors=[];
    page.on("pageerror",e=>errors.push(e.message));
    await page.goto(origin+"/projects");
    await page.getByRole("button",{name:"New project",exact:true}).click();
    await page.getByLabel("Project name").fill("Dashboard browser project");
    await page.getByLabel("Website").fill("http://badseo.test/");
    await page.getByRole("button",{name:"Create project",exact:true}).click();
    await page.waitForURL(/\/p\/[^/]+\/onboarding/);
    pid=page.url().split("/p/")[1].split("/")[0];
    await page.getByRole("heading",{name:"Website setup",exact:true}).waitFor();
    await page.getByRole("button",{name:"Use website suggestion",exact:true}).waitFor({timeout:60000});
    await page.getByRole("button",{name:"Use website suggestion",exact:true}).click();
    assert((await page.getByLabel("Business overview",{exact:true}).inputValue()).length>0);
    await page.getByLabel("Current goal",{exact:true}).fill("Increase qualified search leads");
    await page.getByRole("button",{name:"Confirm and save setup",exact:true}).click();
    await page.getByText("Website setup saved to project memory",{exact:true}).waitFor();
    await page.getByLabel("Current goal",{exact:true}).fill("Increase qualified leads this quarter");
    await page.getByRole("button",{name:"Confirm and save setup",exact:true}).click();
    await page.waitForFunction(async id=>{const r=await fetch(`/api/v1/projects/${id}/context`);return (await r.json()).revision===2;},pid);
    await page.getByRole("link",{name:"Go to dashboard",exact:true}).click();
    await page.getByText("Set up this project",{exact:true}).waitFor();
    await page.getByText("Site health",{exact:true}).waitFor();
    await page.getByRole("link",{name:"Search keywords",exact:true}).click();
    await page.getByRole("heading",{name:"Search keywords",exact:true}).waitFor();
    await page.getByRole("link",{name:"Set up Search Console",exact:true}).waitFor();
    await page.getByRole("link",{name:"Memory",exact:true}).click();
    await page.locator("textarea").filter({hasText:"Increase qualified leads this quarter"}).waitFor();
    await page.getByText("Project memory",{exact:true}).waitFor();
    for (const route of ["", "/onboarding", "/dashboard/keywords", "/memory"]) {
      await page.setViewportSize({width:390,height:844});
      await page.goto(`${origin}/p/${pid}${route}`);
      await page.waitForLoadState("networkidle");
      assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),`mobile overflow ${route}`);
    }
    const report=await api(`/projects/${pid}/reports`,"POST",{title:"Public dashboard regression",html:"<!doctype html><html><body><h1>Shared result</h1></body></html>"});
    const share=await api(`/projects/${pid}/reports/${report.id}/share`,"POST");
    const anonymous=await browser.newContext();
    const publicPage=await anonymous.newPage();
    await publicPage.goto(`${origin}/s/${share.token}`);
    await publicPage.getByRole("heading",{name:"Public dashboard regression",exact:true}).waitFor();
    await publicPage.frameLocator("iframe").getByRole("heading",{name:"Shared result"}).waitFor();
    await anonymous.close();
    if(process.env.SCREENSHOT_PATH) await page.screenshot({path:process.env.SCREENSHOT_PATH,fullPage:true});
    assert.deepEqual(errors,[]);
    console.log("PASS browser: project creation auto-audit, evidence review, repeated memory saves, dashboard/navigation, mobile layouts, anonymous report sharing");
  } catch(e) {
    if(page) { await page.screenshot({path:process.env.TEMP+"/seoseo-m4-browser/dashboard-failure.png",fullPage:true}); }
    throw e;
  } finally {
    if(pid){await api(`/projects/${pid}/archive`,"POST");await api(`/projects/${pid}`,"DELETE");}
    await browser.close();
  }
})().catch(e=>{console.error(e);process.exit(1);});
