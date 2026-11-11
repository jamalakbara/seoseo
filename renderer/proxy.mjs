// Internal forward proxy; no public port. Every CONNECT and HTTP request pins validated DNS.
import http from 'node:http';
import net from 'node:net';
import { destination } from './policy.mjs';
const options = { allowPrivate: process.env.RENDERER_ALLOW_PRIVATE === 'true' };
const server = http.createServer(async (req, res) => {
  try {
    const d = await destination(req.url, options);
    if (d.url.protocol !== 'http:') throw Error('HTTPS requires CONNECT');
    const headers = { ...req.headers, host: d.url.host };
    delete headers['proxy-authorization']; delete headers['proxy-connection'];
    const upstream = http.request({ hostname: d.address, port: d.port, method: req.method,
      path: d.url.pathname + d.url.search, headers, timeout: 20000 }, r => {
      res.writeHead(r.statusCode, r.headers); r.pipe(res);
    });
    upstream.on('timeout', () => upstream.destroy());
    upstream.on('error', () => { if (!res.headersSent) res.writeHead(502); res.end(); });
    req.on('aborted', () => upstream.destroy());
    req.pipe(upstream);
  } catch { res.writeHead(403); res.end('Destination blocked'); }
});
server.on('connect', async (req, client, head) => {
  client.on('error', () => {});
  try {
    const d = await destination(`https://${req.url}`, options);
    const upstream = net.connect({ host: d.address, port: d.port });
    upstream.setTimeout(60000, () => upstream.destroy());
    upstream.once('connect', () => {
      client.write('HTTP/1.1 200 Connection Established\r\n\r\n');
      if (head.length) upstream.write(head);
      client.pipe(upstream); upstream.pipe(client);
    });
    upstream.on('error', () => client.destroy());
    client.on('close', () => upstream.destroy());
  } catch { client.end('HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n'); }
});
server.headersTimeout = 10000;
server.requestTimeout = 30000;
server.listen(8081, '0.0.0.0');
