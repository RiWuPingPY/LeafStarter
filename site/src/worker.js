// LeafStarter 官网后端（Cloudflare Worker，leafstarter.pingyi.me）：
//   GET /api/status                 下载开没开、最新版本（官网用来点亮下载按钮）
//   GET /api/update/<频道>           自动更新信息（R2 的 update/<频道>.json；还没有就回 0.0.0，启动器当作「已经是最新」）
//   GET /update/<版本>/<文件>        自动更新下的 exe：只给更新信息里写到的那个文件，下载开没开都给（老用户要能升级）
//   GET /download/<种类>[/<版本>]     setup / portable / portableFull；DOWNLOADS_OPEN 不是 "true" 时不给
//   GET /api/stats                  各种类的下载次数（近似值）
//   /cf/…                           CurseForge 代理（见 cf.js；Key 在 Worker 密钥 CF_API_KEY 里）
// 其余路径交给静态资源。R2 绑定 RELEASES 可以先不配：没配就当还没有发布过版本。

import { curseforge } from './cf.js';

const STATS_KEY = 'stats/downloads.json';
const KINDS = new Set(['setup', 'portable', 'portableFull']);
const CHANNELS = new Set(['stable', 'beta']);

const json = (data, init = {}) =>
    new Response(JSON.stringify(data), {
        ...init,
        headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'public, max-age=60', ...init.headers },
    });

async function readJson(env, key, fallback) {
    if (!env.RELEASES) return fallback;
    const obj = await env.RELEASES.get(key);
    if (!obj) return fallback;
    try {
        return await obj.json();
    } catch {
        return fallback;
    }
}

const open = (env) => String(env.DOWNLOADS_OPEN || '').toLowerCase() === 'true';

async function bump(env, kind) {
    const stats = await readJson(env, STATS_KEY, {});
    stats[kind] = (stats[kind] || 0) + 1;
    stats.updatedAt = new Date().toISOString();
    await env.RELEASES.put(STATS_KEY, JSON.stringify(stats), { httpMetadata: { contentType: 'application/json' } });
}

/** 中文文件名走 filename*，不认它的老客户端用英文文件名 */
function disposition(name, key) {
    const ascii = key.split('/').pop().replace(/[^\x20-\x7e]|"/g, '_');
    return `attachment; filename="${ascii}"; filename*=UTF-8''${encodeURIComponent(name)}`;
}

/** 从 R2 流式给文件，支持断点续传 */
async function serve(request, env, ctx, key, name, type, countAs) {
    if (!env.RELEASES) return new Response('还没有发布过版本', { status: 404 });
    const range = request.headers.get('range');
    const obj = await env.RELEASES.get(key, { range: request.headers, onlyIf: request.headers });
    if (!obj) return new Response('文件不存在', { status: 404 });
    const headers = new Headers();
    obj.writeHttpMetadata(headers);
    headers.set('etag', obj.httpEtag);
    headers.set('accept-ranges', 'bytes');
    headers.set('content-disposition', disposition(name, key));
    headers.set('cache-control', 'public, max-age=3600');
    if (!headers.get('content-type')) headers.set('content-type', type);
    if (!('body' in obj) || !obj.body) return new Response(null, { status: 304, headers });
    let status = 200;
    if (range && obj.range) {
        const { offset = 0, length = obj.size - offset } = obj.range;
        headers.set('content-range', `bytes ${offset}-${offset + length - 1}/${obj.size}`);
        headers.set('content-length', String(length));
        status = 206;
    } else {
        headers.set('content-length', String(obj.size));
    }
    // 续传的后半段不算一次新下载
    if (countAs && request.method === 'GET' && (!range || /^bytes=0-/.test(range))) ctx.waitUntil(bump(env, countAs).catch(() => {}));
    return new Response(request.method === 'HEAD' ? null : obj.body, { status, headers });
}

function closedPage() {
    return new Response(
        `<!DOCTYPE html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>下载还没开放 · LeafStarter</title><link rel="stylesheet" href="/site.css"></head>
<body><main class="wrap" style="min-height:100vh;display:grid;place-items:center;text-align:center"><div><span class="kicker">LeafStarter</span><h1 style="font:800 36px/1.3 var(--serif);margin:8px 0 12px">下载<span class="grad">还没开放</span></h1><p class="hint" style="margin-bottom:22px">公测开始时会在官网和 GitHub Releases 里放出来。</p><a class="btn pri" href="/">回到首页</a></div></main></body></html>`,
        { status: 403, headers: { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' } },
    );
}

export default {
    async fetch(request, env, ctx) {
        const url = new URL(request.url);
        const path = url.pathname.replace(/\/+$/, '') || '/';
        if (path.startsWith('/cf/')) return curseforge(request, env, path.slice(3));
        if (request.method !== 'GET' && request.method !== 'HEAD') {
            return new Response('Method Not Allowed', { status: 405, headers: { allow: 'GET, HEAD' } });
        }

        if (path === '/api/status') {
            const latest = open(env) ? await readJson(env, 'releases/latest.json', null) : null;
            return json({ downloadsOpen: open(env) && !!latest, latest }, { headers: { 'cache-control': 'public, max-age=60' } });
        }

        let m = path.match(/^\/api\/update\/([a-z]+)$/);
        if (m) {
            if (!CHANNELS.has(m[1])) return json({ error: '没有这个频道' }, { status: 404 });
            const data = await readJson(env, `update/${m[1]}.json`, null);
            // 还没发过版本：回一个最小的版本号，启动器当作「已经是最新」
            return json(data || { version: '0.0.0', files: {} }, { headers: { 'cache-control': 'public, max-age=300' } });
        }

        m = path.match(/^\/update\/([\w.\-]+)\/([\w.\-]+\.exe)$/);
        if (m) {
            // 只给某个频道的更新信息里真的写到的文件
            const [, ver, file] = m;
            const want = `/update/${ver}/${file}`;
            const refs = await Promise.all([...CHANNELS].map((c) => readJson(env, `update/${c}.json`, null)));
            const ok = refs.some((r) => r && Object.values(r.files || {}).some((f) => new URL(f.url).pathname === want));
            if (!ok) return new Response('没有这个更新包', { status: 404 });
            return serve(request, env, ctx, `releases/${ver}/${file}`, file, 'application/vnd.microsoft.portable-executable', 'update');
        }

        // 启动器按需下的第三方工具（EasyTier 等），原样转存，启动器按 SHA-256 校验
        m = path.match(/^\/tools\/([\w.\-]+\.zip)$/);
        if (m) return serve(request, env, ctx, `tools/${m[1]}`, m[1], 'application/zip', null);

        m = path.match(/^\/download\/([A-Za-z]+)(?:\/([\w.\-]+))?$/);
        if (m) {
            if (!KINDS.has(m[1])) return new Response('没有这种下载', { status: 404 });
            if (!open(env)) return closedPage();
            const rel = await readJson(env, m[2] ? `releases/${m[2]}/release.json` : 'releases/latest.json', null);
            const f = rel?.files?.[m[1]];
            if (!f) return new Response('这个版本没有这种下载', { status: 404 });
            const type = f.name.endsWith('.zip') ? 'application/zip' : 'application/vnd.microsoft.portable-executable';
            return serve(request, env, ctx, f.key, f.name, type, m[1]);
        }

        if (path === '/api/stats') {
            const s = await readJson(env, STATS_KEY, {});
            return json({ setup: s.setup || 0, portable: s.portable || 0, portableFull: s.portableFull || 0, update: s.update || 0, updatedAt: s.updatedAt || null }, { headers: { 'cache-control': 'public, max-age=30' } });
        }

        if (path.startsWith('/api/')) return json({ error: 'not found' }, { status: 404, headers: { 'cache-control': 'no-store' } });
        return env.ASSETS.fetch(request);
    },
};
