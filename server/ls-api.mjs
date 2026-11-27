// LeafStarter 源站 API：CurseForge 代理（Key 只放在这台服务器的 /etc/leafstarter/cf.env 里）
//
// 链路：启动器 → nginx（api.pingyi.me:8443/ls/，第一层限速、gzip、防护模式）→ 这里（127.0.0.1:8790）→ CurseForge
// 这里做第二层：每个 IP 每分钟次数、全局并发和每分钟上限。超了回 503 { fallback: true }，
// 启动器就改走官网 Worker（leafstarter.pingyi.me/cf，Key 在 Cloudflare 那边），玩家只是慢一点，不会搜不到。
//
// CurseForge 第三方 API 条款：Key 不给客户端；不缓存、不存储 API 返回的数据（只裁掉启动器用不到的字段，省带宽）；
// 文件由客户端直接从 CurseForge 的 CDN 下，这里不转发文件；只放行启动器用到的接口和 Minecraft（gameId 432）。
import http from 'node:http';
import { existsSync } from 'node:fs';

const PORT = Number(process.env.PORT || 8790);
const HOST = process.env.HOST || '127.0.0.1';
const KEY = process.env.CF_API_KEY || '';
// 测试时可以把上游指到官网 Worker 的 /cf（Key 在那边，这里就不带 Key）；正式就是 CurseForge
const UPSTREAM = (process.env.CF_UPSTREAM || 'https://api.curseforge.com').replace(/\/+$/, '');
const DIRECT = UPSTREAM === 'https://api.curseforge.com';
const SHIELD = process.env.SHIELD_FILE || '/run/ls-guard/shield';

const PER_IP_MIN = Number(process.env.PER_IP_MIN || 120); // 每个 IP 每分钟（和 Worker 一样）
const GLOBAL_MIN = Number(process.env.GLOBAL_MIN || 1500); // 全部加起来每分钟，护住 Key 的配额和这台机器的带宽
const MAX_INFLIGHT = Number(process.env.MAX_INFLIGHT || 24); // 同时在等 CurseForge 的请求
const MC = '432';

const ALLOW = [
    ['GET', /^\/v1\/mods\/search$/],
    ['GET', /^\/v1\/mods\/\d+$/],
    ['GET', /^\/v1\/mods\/\d+\/description$/],
    ['GET', /^\/v1\/mods\/\d+\/files$/],
    ['GET', /^\/v1\/mods\/\d+\/files\/\d+$/],
    ['GET', /^\/v1\/mods\/\d+\/files\/\d+\/download-url$/],
    ['POST', /^\/v1\/mods$/],
    ['POST', /^\/v1\/mods\/files$/],
    ['POST', /^\/v1\/fingerprints\/432$/],
    ['GET', /^\/v1\/games\/432\/versions$/],
    ['GET', /^\/v1\/categories$/],
];

// ---------- 只留启动器用得到的字段（src-tauri/src/content/curseforge.rs 读的就是这些；启动器要多读什么，这里一起加） ----------
const pick = (o, keys) => (o ? Object.fromEntries(keys.filter((k) => o[k] !== undefined).map((k) => [k, o[k]])) : o);
const modOut = (m) => m && {
    id: m.id, classId: m.classId, slug: m.slug, name: m.name, summary: m.summary,
    authors: (m.authors || []).slice(0, 3).map((a) => ({ name: a.name })),
    downloadCount: m.downloadCount,
    logo: m.logo ? { thumbnailUrl: m.logo.thumbnailUrl } : null,
    categories: (m.categories || []).map((c) => ({ name: c.name })),
    latestFilesIndexes: [...new Set((m.latestFilesIndexes || []).map((x) => x.gameVersion))].map((gameVersion) => ({ gameVersion })),
    dateModified: m.dateModified,
    links: pick(m.links, ['websiteUrl', 'sourceUrl', 'issuesUrl', 'wikiUrl']),
};
const fileOut = (f) => f && {
    id: f.id, modId: f.modId, displayName: f.displayName, fileName: f.fileName, releaseType: f.releaseType,
    fileDate: f.fileDate, fileLength: f.fileLength, downloadUrl: f.downloadUrl, gameVersions: f.gameVersions,
    hashes: (f.hashes || []).filter((h) => h.algo === 1),
    dependencies: (f.dependencies || []).map((d) => ({ modId: d.modId, relationType: d.relationType })),
    fileFingerprint: f.fileFingerprint,
};
function trim(method, path, v) {
    if (!v || typeof v !== 'object') return v;
    const list = (fn) => ({ ...v, data: (v.data || []).map(fn) });
    if (path === '/v1/mods/search') return list(modOut);
    if (method === 'POST' && path === '/v1/mods') return list(modOut);
    if (/^\/v1\/mods\/\d+$/.test(path)) return { data: modOut(v.data) };
    if (/^\/v1\/mods\/\d+\/files$/.test(path)) return list(fileOut);
    if (/^\/v1\/mods\/\d+\/files\/\d+$/.test(path)) return { data: fileOut(v.data) };
    if (method === 'POST' && path === '/v1/mods/files') return list(fileOut);
    if (path === '/v1/fingerprints/432') return { data: { exactMatches: (v.data?.exactMatches || []).map((m) => ({ id: m.id, file: { id: m.file?.id, fileFingerprint: m.file?.fileFingerprint } })) } };
    return v;
}

// ---------- 限速 ----------
const perIp = new Map(); // ip → { n, until }
let globalWin = { n: 0, until: 0 };
let inflight = 0;
const stats = { ok: 0, ipLimited: 0, globalLimited: 0, upstreamErr: 0 };
function allowIp(ip, now) {
    let e = perIp.get(ip);
    if (!e || e.until <= now) perIp.set(ip, (e = { n: 0, until: now + 60_000 }));
    return ++e.n <= PER_IP_MIN;
}
function allowGlobal(now) {
    if (globalWin.until <= now) globalWin = { n: 0, until: now + 60_000 };
    return ++globalWin.n <= GLOBAL_MIN && inflight < MAX_INFLIGHT;
}
setInterval(() => {
    const now = Date.now();
    for (const [k, e] of perIp) if (e.until <= now) perIp.delete(k);
}, 60_000).unref();
setInterval(() => {
    if (stats.ok + stats.ipLimited + stats.globalLimited + stats.upstreamErr) console.log(`最近 10 分钟：转发 ${stats.ok}，单 IP 超限 ${stats.ipLimited}，总量超限 ${stats.globalLimited}，上游出错 ${stats.upstreamErr}`);
    Object.keys(stats).forEach((k) => (stats[k] = 0));
}, 600_000).unref();

const send = (res, status, body) => {
    const s = JSON.stringify(body);
    res.writeHead(status, { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store', 'content-length': Buffer.byteLength(s) });
    res.end(s);
};
const readBody = (req, max) => new Promise((ok, bad) => {
    const parts = [];
    let n = 0;
    req.on('data', (c) => {
        n += c.length;
        // 超了：剩下的读完丢掉（不存），好正常回 413，而不是直接掐断连接
        if (n > max) bad(new Error('too large'));
        else parts.push(c);
    });
    req.on('end', () => ok(Buffer.concat(parts).toString('utf8')));
    req.on('error', bad);
});

async function handle(req, res) {
    const url = new URL(req.url, 'http://x');
    const path = url.pathname.replace(/^\/ls/, '');
    const now = Date.now();
    // 启动器按代理地址拼 /ls/cf/health（和 Worker 的 /cf/health 一样），两种都认
    if (path === '/health' || path === '/cf/health') return send(res, 200, { ready: DIRECT ? !!KEY : true, shield: existsSync(SHIELD) });
    if (!path.startsWith('/cf/')) return send(res, 404, { error: 'not found' });
    const cf = path.slice(3);
    if (!ALLOW.some(([m, re]) => m === req.method && re.test(cf))) return send(res, 404, { error: 'not allowed' });
    if (DIRECT && !KEY) return send(res, 503, { error: 'CurseForge Key 还没放到源站', fallback: true });
    const ip = req.headers['x-real-ip'] || req.socket.remoteAddress || '?';
    if (!allowIp(ip, now)) {
        stats.ipLimited++;
        return send(res, 429, { error: '请求太频繁，等一分钟', fallback: true });
    }
    if (!allowGlobal(now)) {
        stats.globalLimited++;
        return send(res, 503, { error: '源站忙', fallback: true });
    }
    inflight++;
    try {
        const q = new URLSearchParams(url.search);
        if (cf === '/v1/mods/search') q.set('gameId', MC);
        const body = req.method === 'POST' ? await readBody(req, 200_000) : undefined;
        const headers = { accept: 'application/json', 'content-type': 'application/json', 'user-agent': 'LeafStarter-origin' };
        if (DIRECT) headers['x-api-key'] = KEY;
        else headers.origin = 'https://leafstarter.pingyi.me';
        const r = await fetch(`${UPSTREAM}${cf}${q.toString() ? `?${q}` : ''}`, { method: req.method, body, headers, signal: AbortSignal.timeout(20_000) });
        const text = await r.text();
        if (!r.ok) {
            stats.upstreamErr++;
            return send(res, r.status >= 500 ? 502 : r.status, { error: `CurseForge 返回 ${r.status}`, fallback: r.status >= 500 || r.status === 429 });
        }
        let out;
        try {
            out = trim(req.method, cf, JSON.parse(text));
        } catch {
            out = text; // description 之类不是 JSON 的，原样给
        }
        stats.ok++;
        if (typeof out === 'string') {
            res.writeHead(200, { 'content-type': r.headers.get('content-type') || 'text/plain; charset=utf-8', 'cache-control': 'no-store' });
            return res.end(out);
        }
        return send(res, 200, out);
    } catch (e) {
        stats.upstreamErr++;
        return send(res, e.message === 'too large' ? 413 : 502, { error: e.message === 'too large' ? 'too large' : '连不上 CurseForge', fallback: e.message !== 'too large' });
    } finally {
        inflight--;
    }
}

const server = http.createServer((req, res) => {
    handle(req, res).catch((e) => {
        console.error(e);
        if (!res.headersSent) send(res, 500, { error: 'internal', fallback: true });
    });
});
server.keepAliveTimeout = 30_000;
server.headersTimeout = 10_000;
server.requestTimeout = 30_000;
server.listen(PORT, HOST, () => console.log(`ls-api 在 ${HOST}:${PORT}，上游 ${UPSTREAM}${DIRECT ? (KEY ? '' : '（还没放 Key）') : '（测试：经 Worker）'}`));
