// 方块补给官网后端：
//   GET /api/latest            最新版本信息（R2 里的 releases/latest.json）
//   GET /api/stats             各平台下载次数（近似值）
//   GET /download/<平台>        从 R2 流式下载最新安装包，支持断点续传（Range）
//   GET /download/<平台>/<版本>  下载指定版本
// 其余路径交给静态资源。

const LATEST_KEY = 'releases/latest.json';
const STATS_KEY = 'stats/downloads.json';
const PLATFORMS = new Set(['windows', 'android']);

const json = (data, init = {}) =>
    new Response(JSON.stringify(data), {
        ...init,
        headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'public, max-age=60', ...init.headers },
    });

async function readJson(env, key, fallback) {
    const obj = await env.RELEASES.get(key);
    if (!obj) return fallback;
    try {
        return await obj.json();
    } catch {
        return fallback;
    }
}

async function latest(env) {
    return readJson(env, LATEST_KEY, null);
}

/** 下载计数：读改写一个 JSON，并发时可能少记几次，只做展示用 */
async function bump(env, platform) {
    const stats = await readJson(env, STATS_KEY, {});
    stats[platform] = (stats[platform] || 0) + 1;
    stats.updatedAt = new Date().toISOString();
    await env.RELEASES.put(STATS_KEY, JSON.stringify(stats), { httpMetadata: { contentType: 'application/json' } });
}

/** 中文文件名走 filename*，不认它的老客户端用 R2 里的英文文件名 */
function contentDisposition(name, key) {
    const ascii = key.split('/').pop().replace(/[^\x20-\x7e]|"/g, '_');
    return `attachment; filename="${ascii}"; filename*=UTF-8''${encodeURIComponent(name)}`;
}

async function download(request, env, ctx, platform, version) {
    let file;
    if (version) {
        const meta = await readJson(env, `releases/${version}/release.json`, null);
        file = meta?.files?.[platform];
    } else {
        file = (await latest(env))?.files?.[platform];
    }
    if (!file) return new Response('这个平台还没有可下载的版本', { status: 404 });

    const range = request.headers.get('range');
    const obj = await env.RELEASES.get(file.key, { range: request.headers, onlyIf: request.headers });
    if (!obj) return new Response('文件不存在', { status: 404 });

    const headers = new Headers();
    obj.writeHttpMetadata(headers);
    headers.set('etag', obj.httpEtag);
    headers.set('accept-ranges', 'bytes');
    headers.set('content-disposition', contentDisposition(file.name, file.key));
    headers.set('cache-control', 'public, max-age=300');
    if (!headers.get('content-type')) {
        headers.set('content-type', platform === 'android' ? 'application/vnd.android.package-archive' : 'application/octet-stream');
    }

    // onlyIf 条件不满足时 R2 返回没有 body 的对象
    if (!('body' in obj) || !obj.body) {
        return new Response(null, { status: 304, headers });
    }
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
    const isFresh = request.method === 'GET' && (!range || /^bytes=0-/.test(range));
    if (isFresh) ctx.waitUntil(bump(env, platform).catch(() => {}));
    return new Response(request.method === 'HEAD' ? null : obj.body, { status, headers });
}

export default {
    async fetch(request, env, ctx) {
        const url = new URL(request.url);
        const path = url.pathname.replace(/\/+$/, '') || '/';

        if (request.method !== 'GET' && request.method !== 'HEAD') {
            return new Response('Method Not Allowed', { status: 405, headers: { allow: 'GET, HEAD' } });
        }

        if (path === '/api/latest') {
            const data = await latest(env);
            return data ? json(data) : json({ error: '还没有发布版本' }, { status: 404, headers: { 'cache-control': 'no-store' } });
        }

        if (path === '/api/stats') {
            const stats = await readJson(env, STATS_KEY, {});
            return json({ windows: stats.windows || 0, android: stats.android || 0, updatedAt: stats.updatedAt || null }, { headers: { 'cache-control': 'public, max-age=30' } });
        }

        const m = path.match(/^\/download\/([a-z]+)(?:\/([\w.\-]+))?$/);
        if (m) {
            if (!PLATFORMS.has(m[1])) return new Response('未知平台', { status: 404 });
            return download(request, env, ctx, m[1], m[2]);
        }

        if (path.startsWith('/api/')) return json({ error: 'not found' }, { status: 404, headers: { 'cache-control': 'no-store' } });
        return env.ASSETS.fetch(request);
    },
};
