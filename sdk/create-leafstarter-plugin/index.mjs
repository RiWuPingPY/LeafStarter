#!/usr/bin/env node
// npm create leafstarter-plugin <文件夹名>：照模板建一个脚本插件，改好 id 和名字
import { cpSync, existsSync, readFileSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const name = process.argv[2];
if (!name) {
    console.log('用法：npm create leafstarter-plugin <文件夹名>');
    process.exit(1);
}
const dir = resolve(name);
if (existsSync(dir)) {
    console.log(`${dir} 已经有了，换个名字`);
    process.exit(1);
}
const here = dirname(fileURLToPath(import.meta.url));
// 发布到 npm 时模板在包里的 template/；在仓库里直接跑时用 ../plugin-template
const tpl = [join(here, 'template'), join(here, '..', 'plugin-template')].find((p) => existsSync(p));
cpSync(tpl, dir, { recursive: true });
const id = basename(dir).toLowerCase().replace(/[^a-z0-9-]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 40) || 'my-plugin';
const mf = join(dir, 'plugin.json');
const m = JSON.parse(readFileSync(mf, 'utf8'));
m.id = id.length >= 2 ? id : `${id}-plugin`;
m.name = basename(dir);
writeFileSync(mf, JSON.stringify(m, null, 2) + '\n');
console.log(`建好了：${dir}
下一步：
  1. 改 plugin.json 里的 permissions 和 contributes，写 index.html
  2. LeafStarter 里打开「插件 → 开发者」，选这个文件夹，改了点「重新加载」
  3. 打包：leafstarter --cli plugin-pack "${dir}" "${m.id}.lsplugin"`);
