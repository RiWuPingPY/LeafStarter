// 本机看官网的静态页面（不跑 Worker 接口，/api 一律回「下载没开」）：node scripts/serve.mjs [端口]
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, posix } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../public/', import.meta.url));
const port = Number(process.env.PORT || process.argv[2] || 8789);
const TYPES = { '.html': 'text/html; charset=utf-8', '.css': 'text/css; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.json': 'application/json', '.png': 'image/png', '.svg': 'image/svg+xml', '.ico': 'image/x-icon' };
createServer(async (req, res) => {
    const url = new URL(req.url, 'http://x');
    if (url.pathname.startsWith('/api/')) {
        res.writeHead(200, { 'content-type': 'application/json' });
        return res.end(JSON.stringify({ downloadsOpen: false, latest: null }));
    }
    // 只认 public/ 里面的文件：规范化后不许往上跳
    let p = posix.normalize(decodeURIComponent(url.pathname));
    if (p.split('/').includes('..')) return res.writeHead(400).end();
    if (p.endsWith('/')) p += 'index.html';
    try {
        const body = await readFile(join(root, ...p.split('/').filter(Boolean)));
        res.writeHead(200, { 'content-type': TYPES[extname(p)] || 'application/octet-stream', 'cache-control': 'no-store' });
        res.end(body);
    } catch {
        res.writeHead(404, { 'content-type': 'text/html; charset=utf-8' });
        res.end(await readFile(join(root, '404.html')));
    }
}).listen(port, '127.0.0.1', () => console.log(`官网：http://127.0.0.1:${port}/`));
