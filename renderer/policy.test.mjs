import { test } from 'node:test';
import assert from 'node:assert/strict';
import { destination, publicAddress } from './policy.mjs';
test('blocks private, mapped, metadata and reserved addresses', () => {
  for (const ip of ['127.0.0.1', '10.0.0.1', '169.254.169.254', '100.64.0.1', '::1', '::ffff:127.0.0.1', '64:ff9b::a00:1', '2001:db8::1']) assert.equal(publicAddress(ip), false, ip);
  assert.equal(publicAddress('8.8.8.8'), true);
});
test('rejects mixed public/private DNS and pins the validated public IP', async () => {
  await assert.rejects(destination('https://example.com', {lookup: async () => [{address:'8.8.8.8'}, {address:'127.0.0.1'}]}));
  const d = await destination('https://example.com/a', {lookup: async () => [{address:'8.8.8.8'}]});
  assert.equal(d.address, '8.8.8.8');
  for (const url of ['file:///etc/passwd', 'http://user:pass@example.com', 'http://127.1', 'http://2130706433', 'http://example.com:8080']) await assert.rejects(destination(url));
});

// Exercise the actual HTTP and CONNECT proxy entry points, not just the helper.
test('forward proxy denies loopback HTTP and CONNECT requests', async () => {
  const { spawn } = await import('node:child_process');
  const http = await import('node:http');
  const child = spawn(process.execPath, ['proxy.mjs'], {env: {...process.env, RENDERER_ALLOW_PRIVATE: 'false'}});
  try {
    let ready = false;
    for (let i = 0; i < 30 && !ready; i++) {
      try { await new Promise((resolve, reject) => { const req = http.get('http://127.0.0.1:8081', r => {r.resume(); resolve();}); req.on('error', reject); }); ready = true; }
      catch { await new Promise(r => setTimeout(r, 50)); }
    }
    assert.ok(ready);
    const code = await new Promise((resolve, reject) => {
      http.get('http://127.0.0.1:8081', {path:'http://127.0.0.1:8080/private'}, r => {r.resume(); resolve(r.statusCode);}).on('error', reject);
    });
    assert.equal(code, 403);
    const connect = await new Promise((resolve, reject) => {
      const req = http.request({host:'127.0.0.1', port:8081, method:'CONNECT', path:'169.254.169.254:443'});
      req.on('connect', (r, socket) => {socket.destroy(); resolve(r.statusCode);}); req.on('error', reject); req.end();
    });
    assert.equal(connect, 403);
  } finally { child.kill(); }
});
