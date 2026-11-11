// Runs inside Chromium; intentionally only uses DOM APIs.
export function extractBing() {
  const items = [...document.querySelectorAll('#b_results > li.b_algo')].slice(0, 20).flatMap(row => {
    const a = row.querySelector('h2 a');
    return a ? [{url: a.href, title: a.textContent.trim(), snippet: row.querySelector('.b_caption p')?.textContent?.trim() || null}] : [];
  });
  const related = [...document.querySelectorAll('.b_rs a')].map(a => a.textContent.trim()).filter(Boolean).slice(0, 20);
  return { items, related };
}

export function normalizeItems(items) {
  const seen = new Set();
  return items.flatMap(item => {
    try {
      const target = new URL(item.url);
      if ((target.hostname === 'bing.com' || target.hostname.endsWith('.bing.com')) && target.pathname === '/ck/a') {
        const encoded = target.searchParams.get('u');
        if (encoded?.startsWith('a1')) item = {...item, url: Buffer.from(encoded.slice(2), 'base64url').toString()};
      }
      const decoded = new URL(item.url);
      if (!['https:', 'http:'].includes(decoded.protocol) || decoded.hostname === 'bing.com' || decoded.hostname.endsWith('.bing.com') || seen.has(decoded.href)) return [];
      seen.add(decoded.href);
      return [item];
    } catch { return []; }
  }).map((item, i) => ({...item, position: i + 1}));
}
