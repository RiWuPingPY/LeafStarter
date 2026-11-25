// 部署前：把启动器界面复制成官网的在线演示（/demo/，浏览器里自动用演示数据；首页的像素风景也用它的 scene.js），放好图标
import { cpSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
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
// 标签页图标：和启动器顶栏一样的叶子
writeFileSync(
    join(pub, 'favicon.svg'),
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#7C3AED"/><stop offset="1" stop-color="#2563EB"/></linearGradient></defs><path d="M4 4h12a12 12 0 0 1 12 12v12H16A12 12 0 0 1 4 16Z" fill="url(#g)"/></svg>\n`,
);
console.log('demo + icons ready');
