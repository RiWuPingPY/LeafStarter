// 部署前：把启动器界面复制成官网的在线演示（/demo/，浏览器里自动用演示数据；首页的像素风景也用它的 scene.js），放好图标
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const site = join(dirname(fileURLToPath(import.meta.url)), '..');
const app = join(site, '..');
const pub = join(site, 'public');

rmSync(join(pub, 'demo'), { recursive: true, force: true });
mkdirSync(join(pub, 'demo'), { recursive: true });
// 许可清单 400 多 KB，演示里用不到
cpSync(join(app, 'src'), join(pub, 'demo'), { recursive: true, filter: (p) => !p.endsWith('third-party.json') });
cpSync(join(app, 'src-tauri', 'icons', 'icon.png'), join(pub, 'icon-512.png'));
// 标签页图标：和启动器一样的「方块芽」
cpSync(join(app, 'src', 'logo.svg'), join(pub, 'favicon.svg'));
// 插件开发文档：把 sdk/README.md 转成 /docs/plugins.html（只认用到的几种 Markdown 写法）
const esc = (t) => t.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const inline = (t) => esc(t).replace(/`([^`]+)`/g, '<code>$1</code>').replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>');
function md(src) {
    const out = [];
    const lines = src.replace(/\r/g, '').split('\n');
    for (let i = 0; i < lines.length; i++) {
        const l = lines[i];
        if (l.startsWith('```')) {
            const buf = [];
            while (++i < lines.length && !lines[i].startsWith('```')) buf.push(lines[i]);
            out.push(`<pre><code>${esc(buf.join('\n'))}</code></pre>`);
        } else if (/^#{1,3} /.test(l)) {
            const n = l.match(/^#+/)[0].length;
            out.push(`<h${n}>${inline(l.slice(n + 1))}</h${n}>`);
        } else if (l.startsWith('|')) {
            const rows = [];
            while (i < lines.length && lines[i].startsWith('|')) rows.push(lines[i++]);
            i--;
            const cells = (r) => r.split('|').slice(1, -1).map((c) => c.trim());
            out.push(`<table><tr>${cells(rows[0]).map((c) => `<th>${inline(c)}</th>`).join('')}</tr>${rows.slice(2).map((r) => `<tr>${cells(r).map((c) => `<td>${inline(c)}</td>`).join('')}</tr>`).join('')}</table>`);
        } else if (/^\d+\. /.test(l)) {
            const items = [];
            while (i < lines.length && /^\d+\. /.test(lines[i])) items.push(lines[i++].replace(/^\d+\. /, ''));
            i--;
            out.push(`<ol>${items.map((x) => `<li>${inline(x)}</li>`).join('')}</ol>`);
        } else if (l.trim()) out.push(`<p>${inline(l)}</p>`);
    }
    return out.join('\n');
}
mkdirSync(join(pub, 'docs'), { recursive: true });
writeFileSync(join(pub, 'docs', 'plugins.html'), `<!DOCTYPE html><html lang="zh-CN"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>插件开发 · LeafStarter</title><link rel="icon" href="/favicon.svg" type="image/svg+xml"><link rel="stylesheet" href="/site.css">
<style>.doc{max-width:820px;margin:0 auto;padding:40px 16px 80px}.doc h1{font:800 34px/1.2 var(--serif)}.doc h2{margin-top:36px}.doc pre{background:var(--panel);border:1px solid var(--line);border-radius:12px;padding:14px;overflow:auto}.doc pre code{background:none;padding:0;color:inherit}.doc table{border-collapse:collapse;width:100%;margin:12px 0}.doc th,.doc td{border:1px solid var(--line);padding:8px 10px;text-align:left;vertical-align:top}.doc .back{display:inline-block;margin-bottom:18px}</style></head>
<body><main class="doc"><a class="back" href="/">← LeafStarter 首页</a>${md(readFileSync(join(app, 'sdk', 'README.md'), 'utf8'))}
<p>模板：<a href="https://github.com/RiWuPingPY/LeafStarter/tree/main/sdk/plugin-template">sdk/plugin-template</a> · 接口类型：<a href="https://github.com/RiWuPingPY/LeafStarter/blob/main/sdk/leafstarter.d.ts">leafstarter.d.ts</a></p></main></body></html>
`);
console.log('demo + icons + docs ready');
