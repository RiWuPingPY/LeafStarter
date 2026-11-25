// 生成第三方许可清单：cargo-about 读出程序用到的每个 Rust 依赖和它的许可原文，
// 整理成 src/third-party.json（「关于」里显示）和 THIRD-PARTY-LICENSES.txt（随安装包、便携版一起发）。
//
//   node scripts/licenses.mjs            需要先装 cargo-about：cargo install cargo-about --locked --features cli
import { execFileSync } from 'node:child_process';
import { writeFileSync, readFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(mkdtempSync(join(tmpdir(), 'ls-about-')), 'about.json');
execFileSync('cargo', ['about', 'generate', '--format', 'json', '-o', out], { cwd: join(root, 'src-tauri'), stdio: ['ignore', 'inherit', 'inherit'] });
const j = JSON.parse(readFileSync(out, 'utf8'));

const SELF = new Set(['leafstarter']);
const crates = j.crates
    .filter((c) => !SELF.has(c.package.name))
    .map((c) => [c.package.name, c.package.version, c.license, (c.package.repository || '').replace(/^https?:\/\//, '').replace(/\.git$/, '')])
    .sort((a, b) => a[0].localeCompare(b[0]));

// 同一份原文（只差空白）只留一份，记下谁用了它
const texts = new Map();
for (const l of j.licenses) {
    const key = l.text.replace(/\s+/g, ' ').trim();
    const users = l.used_by.map((u) => `${u.crate.name} ${u.crate.version}`).filter((n) => !SELF.has(n.split(' ')[0]));
    if (!users.length) continue;
    const t = texts.get(key) || { id: l.id, name: l.name, text: l.text.trim(), users: [] };
    t.users.push(...users);
    texts.set(key, t);
}
const list = [...texts.values()].map((t) => ({ ...t, users: [...new Set(t.users)].sort() })).sort((a, b) => b.users.length - a.users.length);

// 前端用到的第三方作品（不经过 cargo）
const web = [{ name: 'Lucide 图标', license: 'ISC（从 Feather 继承的图标为 MIT）', url: 'lucide.dev' }];

const data = { generated: new Date().toISOString().slice(0, 10), web, crates, texts: list };
writeFileSync(join(root, 'src', 'third-party.json'), JSON.stringify(data));

const lines = [
    'LeafStarter 用到的第三方作品和它们的许可',
    `生成于 ${data.generated}，共 ${crates.length} 个 Rust 依赖。LeafStarter 本身以 Apache-2.0 发布，见 LICENSE。`,
    '',
    ...web.map((w) => `${w.name}：${w.license}（${w.url}）`),
    '',
    '=== Rust 依赖 ===',
    ...crates.map(([n, v, l, r]) => `${n} ${v}  ${l}${r ? `  ${r}` : ''}`),
    '',
    '=== 许可原文 ===',
];
for (const t of list) {
    lines.push('', '-'.repeat(72), `${t.name}（${t.id}）`, `用到它的：${t.users.join('、')}`, '-'.repeat(72), '', t.text);
}
writeFileSync(join(root, 'THIRD-PARTY-LICENSES.txt'), lines.join('\n') + '\n');
const size = (p) => (readFileSync(p).length / 1024).toFixed(0) + ' KB';
console.log(`${crates.length} 个依赖，${list.length} 份不同的许可原文；src/third-party.json ${size(join(root, 'src', 'third-party.json'))}，THIRD-PARTY-LICENSES.txt ${size(join(root, 'THIRD-PARTY-LICENSES.txt'))}`);
