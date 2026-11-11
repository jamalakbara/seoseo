import { test } from 'node:test';
import assert from 'node:assert/strict';
import { extractBing, normalizeItems } from './serp-parser.mjs';

test('normalizes Bing redirects, drops unsafe and duplicate URLs, preserves positions', () => {
  const encoded = 'a1' + Buffer.from('https://example.com/article').toString('base64url');
  const items = normalizeItems([
    {url:`https://www.bing.com/ck/a?u=${encoded}`,title:'Article'},
    {url:'https://example.com/article',title:'Duplicate'},
    {url:'javascript:alert(1)',title:'Unsafe'},
    {url:'https://www.bing.com/search?q=x',title:'Internal'},
    {url:'https://example.org/',title:'Second'},
  ]);
  assert.deepEqual(items.map(i => [i.url,i.position]), [['https://example.com/article',1],['https://example.org/',2]]);
});

test('extracts organic results and related queries from a browser fixture', {skip:!process.env.CHROME_PATH}, async () => {
  const { default: puppeteer } = await import('puppeteer-core');
  const browser = await puppeteer.launch({executablePath:process.env.CHROME_PATH,headless:true,args:['--no-sandbox']});
  try {
    const page = await browser.newPage();
    await page.setContent('<ol id="b_results"><li class="b_ad"><h2><a href="https://ads.example">Ad</a></h2></li><li class="b_algo"><h2><a href="https://example.com/article">Article</a></h2><div class="b_caption"><p>A useful snippet</p></div></li></ol><div class="b_rs"><a>Related phrase</a></div>');
    const result = await page.evaluate(extractBing);
    assert.equal(result.items.length, 1);
    assert.equal(result.items[0].snippet, 'A useful snippet');
    assert.deepEqual(result.related, ['Related phrase']);
  } finally { await browser.close(); }
});
