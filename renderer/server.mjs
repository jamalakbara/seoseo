import http from 'node:http';
import dns from 'node:dns/promises';
import puppeteer from 'puppeteer-core';
import lighthouse from 'lighthouse';
import { extractBing, normalizeItems } from './serp-parser.mjs';

const UA = 'SeoSeoBot/0.1 (+https://github.com/seoseo/seoseo)';
let busy = false;
let nextSerp = 0;
let blockedUntil = 0;
const send = (res, status, body) => { res.writeHead(status, { 'content-type': 'application/json' }); res.end(JSON.stringify(body)); };

export function blocked(status, text) {
  return [401, 403, 429].includes(status) || /unusual traffic|verify (that )?you are human|access denied/i.test(text);
}

async function run(input) {
  let url;
  const serp = input.mode === 'serp';
  if (serp) {
    if (Date.now() < blockedUntil) throw Error('Search provider blocked; cooldown active');
    if (Date.now() < nextSerp) throw Error('Search rate limit; try later');
    nextSerp = Date.now() + 10000;
    if (typeof input.keyword !== 'string' || input.keyword.length > 120 || !/^[A-Z]{2}$/.test(input.country) || !/^[a-z]{2,3}(-[a-z]{2})?$/.test(input.language)) throw Error('Invalid search market or keyword');
    url = new URL('https://www.bing.com/search');
    url.search = new URLSearchParams({ q: input.keyword, cc: input.country, setlang: input.language }).toString();
  } else {
    url = new URL(input.url);
  }
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) throw Error('Invalid URL');
  // Pin proxy itself; browser has no external network route (Compose internal network).
  const proxy = await dns.lookup('renderer-proxy');
  const browser = await puppeteer.launch({ executablePath: process.env.CHROME_PATH || '/usr/bin/chromium',
    headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage', '--disable-quic',
      '--force-webrtc-ip-handling-policy=disable_non_proxied_udp', '--disable-background-networking',
      `--proxy-server=http://${proxy.address}:8081`, '--proxy-bypass-list=<-loopback>',
      `--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE ${proxy.address}`, `--user-agent=${UA}`] });
  const deadline = setTimeout(() => browser.close().catch(() => {}), 90000);
  try {
    const page = await browser.newPage();
    await page.setUserAgent(UA);
    await page.setBypassServiceWorker(true);
    await page.setRequestInterception(true);
    page.on('request', request => {
      const target = request.url();
      const isNavigation = request.isNavigationRequest() && request.frame() === page.mainFrame();
      const allowed = /^https?:/.test(target) && (!isNavigation || new URL(target).origin === url.origin);
      (allowed ? request.continue() : request.abort()).catch(() => {});
    });
    const response = await page.goto(url.href, { waitUntil: 'networkidle2', timeout: 25000 });
    const text = await page.evaluate(() => document.body?.innerText?.slice(0, 10000) || '');
    const challenge = await page.$('#challenge-running, #b_captcha, #captcha-form, form[action*="/sorry/"]');
    if (challenge || blocked(response?.status(), text)) {
      if (serp) blockedUntil = Date.now() + 60 * 60 * 1000;
      throw Error('Provider blocked or challenged the request; stopped');
    }
    if (!response?.ok()) throw Error(`Page returned ${response?.status() ?? 'no response'}`);
    if (serp) {
      const result = await page.evaluate(extractBing);
      result.items = normalizeItems(result.items);
      if (!result.items.length) throw Error('Search markup changed or no organic results were available');
      return { ...result, features: [], questions: [] };
    }
    const finalUrl = new URL(page.url());
    finalUrl.hash = ''; url.hash = '';
    if (finalUrl.href !== url.href) throw Error('Page navigated away from the requested URL; static content retained');
    const html = await page.content();
    if (Buffer.byteLength(html) > 1024 * 1024) throw Error('Rendered HTML exceeds 1 MiB');
    let lab = null;
    let lighthouseError = null;
    if (input.lighthouse) {
      try {
      const port = Number(new URL(browser.wsEndpoint()).port);
      const result = await lighthouse(url.href, { port, output: 'json', logLevel: 'error',
        onlyCategories: ['performance', 'accessibility', 'best-practices', 'seo'], maxWaitForLoad: 25000,
        formFactor: 'desktop', screenEmulation: { disabled: true }, emulatedUserAgent: UA });
      if (!result || result.lhr.runtimeError) throw Error(result?.lhr.runtimeError?.message || 'Lighthouse failed');
      lab = {
        source: 'lighthouse', version: result.lhr.lighthouseVersion, fetched_at: result.lhr.fetchTime,
        mode: 'desktop-lab', scores: Object.fromEntries(Object.entries(result.lhr.categories).map(([k,v]) => [k,v.score == null ? null : Math.round(v.score * 100)])),
        metrics: Object.fromEntries(['first-contentful-paint', 'largest-contentful-paint', 'total-blocking-time', 'cumulative-layout-shift', 'speed-index'].map(k => [k, {value: result.lhr.audits[k]?.numericValue ?? null, unit: result.lhr.audits[k]?.numericUnit ?? null}])),
        opportunities: Object.values(result.lhr.audits).filter(a => a.score !== null && a.score < 1).slice(0, 20).map(a => ({id:a.id, title:a.title, description:a.description, display_value:a.displayValue ?? null})),
      };
      } catch (e) { lighthouseError = e.message; }
    }
    return { html, final_url: page.url(), lighthouse: lab, lighthouse_error: lighthouseError };
  } finally { clearTimeout(deadline); await browser.close(); }
}

http.createServer(async (req, res) => {
  if (req.url === '/health' && req.method === 'GET') return send(res, 200, {ok:true});
  if (req.url !== '/render' || req.method !== 'POST') return send(res, 404, {error:'Not found'});
  if (busy) return send(res, 429, {error:'Renderer busy'});
  busy = true;
  try {
    let body = '';
    for await (const chunk of req) { body += chunk; if (body.length > 16384) throw Error('Request too large'); }
    const result = await run(JSON.parse(body));
    send(res, 200, result);
  } catch (e) { send(res, 502, {error:e.message}); }
  finally { busy = false; }
}).listen(8080, '0.0.0.0');
