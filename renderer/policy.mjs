import dns from 'node:dns/promises';
import ipaddr from 'ipaddr.js';

export function publicAddress(address) {
  try {
    let ip = ipaddr.parse(address);
    if (ip.kind() === 'ipv6' && ip.isIPv4MappedAddress()) ip = ip.toIPv4Address();
    return ip.range() === 'unicast';
  } catch { return false; }
}

export async function destination(raw, { allowPrivate = false, lookup = dns.lookup } = {}) {
  const url = new URL(raw);
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) throw Error('URL not allowed');
  const host = url.hostname.replace(/^\[|\]$/g, '').replace(/\.$/, '');
  const port = Number(url.port || (url.protocol === 'https:' ? 443 : 80));
  if (!allowPrivate && (![80, 443].includes(port) || host === 'localhost' || /\.(localhost|local|internal)$/.test(host))) throw Error('Destination not allowed');
  const addresses = ipaddr.isValid(host) ? [{ address: host }] : await lookup(host, { all: true });
  if (!addresses.length || (!allowPrivate && addresses.some(a => !publicAddress(a.address)))) throw Error('Private or reserved address');
  // Connect to this exact validated IP: never resolve again when opening a socket.
  return { url, host, port, address: addresses[0].address };
}
