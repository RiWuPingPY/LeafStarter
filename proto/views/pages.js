// 下载、任务、设置三页，以及「新建实例」侧边面板
import { esc, ago, head } from '../ui.js';
import { icon } from '../icons.js';
import { loaderVersion } from '../data.js';
import { lvChip } from './agentView.js';
import { PRESETS, KIND, USES, PARAMS, paramsFor, ready, isImg, MCP_CLIENTS } from '../aisrc.js';

const seg = (act, list, cur, extra = '') => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="${act}" data-v="${v}" ${extra} class="${String(cur) === v ? 'on' : ''}" aria-pressed="${String(cur) === v}">${t}</button>`).join('')}</div>`;

// ---------- 下载 ----------
const TYPES = [['mod', 'Mod'], ['pack', '整合包'], ['rp', '资源包'], ['shader', '光影']];

export function downloadView(c) {
    const d = c.S.dl;
    return `<div class="page" id="page">
        <div class="page-h"><h1>下载</h1><span class="chip warn">示例结果</span></div>
        <div class="toolbar">
            <label class="filter grow" for="dlQ">${icon('search', 14)}<input id="dlQ" type="text" placeholder="搜索 Modrinth 和 CurseForge" value="${esc(d.q)}" autocomplete="off"></label>
            ${seg('dl-type', TYPES, d.type)}
        </div>
        <div class="toolbar" id="dlTargetBar">${targetBar(c)}</div>
        <div class="rows" id="dlList">${dlList(c)}</div>
    </div>`;
}

export function dlTarget(c) {
    return c.byId(c.S.dl.target) || c.inst;
}

export function targetBar(c) {
    if (c.S.dl.type === 'pack') return '<span class="note">整合包会装成一个新实例，装的时候可以继续做别的事。</span>';
    const t = dlTarget(c);
    if (!t) return '';
    return `<label class="note" for="dlTarget">装到</label>
        <select id="dlTarget">${c.INSTANCES.map((i) => `<option value="${i.id}"${i.id === t.id ? ' selected' : ''}>${esc(i.name)}（${esc(i.mc)} · ${i.loader ? esc(i.loader) : '原版'}）</option>`).join('')}</select>
        <span class="note">${c.S.dl.type === 'mod' ? '结果只列出能装进这个实例的' : ''}</span>`;
}

function results(c) {
    const d = c.S.dl;
    const q = d.q.trim().toLowerCase();
    const t = dlTarget(c);
    return c.CATALOG[d.type].filter((it) => (!q || `${it.name} ${it.sum} ${it.by}`.toLowerCase().includes(q)) && (d.type !== 'mod' || !t?.loader || it.loaders.includes(t.loader)));
}

export function dlList(c) {
    const d = c.S.dl;
    const t = dlTarget(c);
    const list = results(c);
    if (d.type === 'mod' && t && !t.loader) return `<div class="row empty">「${esc(t.name)}」是原版，装不了 Mod。换一个实例，或者先新建一个 Fabric 实例。</div>`;
    if (!list.length) return `<div class="row empty">没找到「${esc(d.q)}」。换个词试试。</div>`;
    // 加载时的占位行和结果行一样高、一样多，结果出来时页面不会跳
    if (d.loading) return list.map(() => '<div class="row dl skel" aria-hidden="true"><span class="sk sq"></span><span class="sk ln"></span><span class="sk bt"></span></div>').join('');
    return list.map((it) => {
        const key = d.type === 'pack' ? `pack:${it.id}` : `${t?.id}:${it.id}`;
        const has = d.installed[key] === 'done' || (d.type === 'mod' && t?.mods.some((m) => m.name === it.name));
        const busy = d.installed[key] === 'busy';
        const label = d.type === 'pack' ? '装成新实例' : '安装';
        const btn = has
            ? `<button type="button" class="btn sm" disabled>${icon('check', 13)}${d.type === 'pack' ? '已装成实例' : '已装好'}</button>`
            : busy
                ? '<button type="button" class="btn sm" disabled>正在装…</button>'
                : `<button type="button" class="btn sm pri" data-act="dl-install" data-id="${it.id}">${icon('download', 13)}${label}</button>`;
        const extra = d.type === 'pack' ? `${esc(it.mc)} · ${esc(it.loader)}` : d.type === 'mod' ? esc(it.loaders.join(' / ')) : '';
        return `<div class="row dl">
            <span class="tile lg" style="--h:${it.h}">${esc(it.name[0])}</span>
            <span class="tx"><span class="nm">${esc(it.name)} <span class="by">${esc(it.by)}</span></span><span class="sb">${esc(it.sum)}</span></span>
            <span class="tags"><span class="chip">${it.src}</span>${extra ? `<span class="chip mono">${extra}</span>` : ''}</span>
            ${btn}
        </div>`;
    }).join('');
}

// ---------- 任务 ----------
export function tasksView(c) {
    return `<div class="page" id="page">
        <div class="page-h"><h1>任务</h1><span class="sp"></span><button type="button" class="btn" data-act="tasks-clear">清除已完成</button></div>
        <p class="note">下载、安装、同步都在后台做，不挡你做别的事。</p>
        <div class="rows" id="taskList">${taskRows(c)}</div>
    </div>`;
}

export function taskRows(c) {
    if (!c.TASKS.length) return '<div class="row empty">没有任务</div>';
    return c.TASKS.map((t) => `<div class="row task">
        <span class="ticon ${t.state}">${icon(t.state === 'done' ? 'check' : t.state === 'fail' ? 'alert' : 'download', 15)}</span>
        <span class="tx"><span class="nm">${esc(t.title)}</span><span class="sb">${esc(t.stage || t.sub)}</span>${t.state === 'run' ? `<span class="bar"><i style="--p:${t.pct}%"></i></span>` : ''}</span>
        <span class="sb mono">${t.state === 'run' ? t.pct + '%' : ago(t.at)}</span>
    </div>`).join('');
}

// ---------- 设置 ----------
const KEYS = [
    ['Enter', '启动选中的实例'],
    ['↑ ↓', '换一个实例'],
    ['Ctrl K', '命令面板：搜实例、Mod，或者直接输入要做的事'],
    ['/', '筛选左边的实例，输完按 Enter 直接启动'],
    ['Esc', '关掉命令面板、侧边面板和弹出的小窗'],
    ['Ctrl Z', '撤销刚才的删除'],
    ['Ctrl 1–5', '实例 / 下载 / 任务 / 插件 / 设置（浏览器里这几个键被标签页占了，用 Alt 1–5）'],
    ['按住 Ctrl 打开', '安全模式：不加载任何第三方插件'],
];

export function settingsView(c) {
    const S = c.S;
    const g = (key, opts) => `<select id="g-${key}" data-g="${key}">${opts.map(([v, t]) => `<option value="${v}"${c.GLOBAL[key] === v ? ' selected' : ''}>${t}</option>`).join('')}</select>`;
    return `<div class="page" id="page">
        <div class="page-h"><h1>设置</h1></div>

        <section class="panel"><h2 class="ph">启动</h2>
            <div class="kv"><div class="k"><b>直接启动</b><small>双击 LeafStarter 图标就启动上次玩的实例，主界面不出现；按住 Shift 再双击才打开界面。</small></div>
                <div class="v"><button type="button" class="btn sm" data-act="preview-direct">${icon('play', 13)}试一下</button><input type="checkbox" class="switch" id="setDirect" data-set="direct"${S.direct ? ' checked' : ''} aria-label="直接启动"></div></div>
            <div class="kv"><div class="k"><b>游戏窗口出来以后</b><small>启动器自己怎么办</small></div>
                <div class="v">${seg('set', [['hide', '隐藏'], ['min', '最小化'], ['keep', '不变']], S.after, 'data-k="after"')}</div></div>
            <div class="kv"><div class="k"><b>Java</b><small>按每个版本的要求自动挑，找不到就下载官方运行时</small></div>
                <div class="v">${g('java', [['auto', '自动选择'], ...c.JAVAS.map((j) => [j.id, j.label])])}</div></div>
            <div class="kv"><div class="k"><b>内存</b><small>本机 16 GB，自动给 4 GB</small></div>
                <div class="v">${g('mem', [['auto', '自动（4 GB）'], ['2', '2 GB'], ['4', '4 GB'], ['6', '6 GB'], ['8', '8 GB']])}</div></div>
        </section>

        <section class="panel"><h2 class="ph">账户</h2>
            ${c.ACCOUNTS.map((a) => `<div class="kv"><div class="k acctk">${head(a.face, 28)}<span><b>${esc(a.name)}</b><small>${esc(a.label)}</small></span></div>
                <div class="v">${S.account === a.id ? `${c.plugOff('skin') ? '' : `<button type="button" class="btn sm" data-act="skin">${icon('user', 13)}换皮肤</button>`}<span class="chip ok">默认账户</span>` : `<button type="button" class="btn sm" data-act="acct-switch" data-id="${a.id}">设为默认</button>`}</div></div>`).join('')}
            <div class="kv"><div class="k"><small>微软账户用设备代码登录：在手机或浏览器上输入一串代码就行。</small></div>
                <div class="v"><button type="button" class="btn sm" data-act="acct-add">${icon('plus', 13)}微软账户</button><button type="button" class="btn sm" data-act="acct-add">${icon('plus', 13)}离线账户</button></div></div>
        </section>

        <section class="panel"><h2 class="ph">外观</h2>
            <div class="kv"><div class="k"><b>主题</b></div>
                <div class="v">${seg('set', [['system', '跟随系统'], ['light', '浅色'], ['dark', '深色']], S.theme, 'data-k="theme"')}</div></div>
            <div class="kv"><div class="k"><b>动效</b><small>${window.matchMedia('(prefers-reduced-motion: reduce)').matches ? '你的系统关了「动画效果」，选「跟随系统」启动器就不动了' : '「精简」会关掉所有过渡和特效'}</small></div>
                <div class="v">${seg('set', [['full', '完整'], ['reduced', '精简'], ['system', '跟随系统']], S.motion, 'data-k="motion"')}</div></div>
        </section>

        <section class="panel"><h2 class="ph">下载</h2>
            <div class="kv"><div class="k"><b>下载源</b><small>智能：按实测速度给每个源打分，哪个快用哪个，卡住了自动换</small></div>
                <div class="v">${seg('set', [['auto', '智能'], ['fixed', '固定']], S.src, 'data-k="src"')}</div></div>
            <div class="kv"><div class="k"><b>同时下载</b><small>个文件</small></div>
                <div class="v"><input type="range" id="setThreads" min="4" max="64" step="4" value="24" aria-label="同时下载的文件数"><b class="mono" id="threadsVal">24</b></div></div>
        </section>

        <section class="panel"><h2 class="ph">手机电脑同步</h2>
            <div class="kv"><div class="k"><b>LeafHub 账号</b><small>同步用，和游戏账户无关</small></div><div class="v"><span class="chip ok">${icon('check', 13)}PingYi 已登录</span></div></div>
            <div class="kv"><div class="k"><b>云空间</b><small>同一个 Mod 文件只存一份；每个存档留最近 10 个历史版本（示例数字）</small></div>
                <div class="v"><span class="bar w"><i style="--p:15%"></i></span><span class="mono sb">312 MB / 2 GB</span></div></div>
        </section>

        ${aiPanel(c)}
        ${mcpPanel(c)}
        ${plugPanel(c)}
        ${mpPanel(c)}
        ${worldPanel(c)}

        <section class="panel"><h2 class="ph">快捷键</h2>
            <div class="keys">${KEYS.map(([k, t]) => `<span class="kk">${k.split(' ').map((x) => `<kbd>${x}</kbd>`).join('')}</span><span>${t}</span>`).join('')}</div>
        </section>
        <section class="panel"><h2 class="ph">关于</h2>
            <div class="kv"><div class="k"><b>LeafStarter 0.1.0（交互原型）</b><small>Apache-2.0 · Copyright 2026 PingYi · 不是 Minecraft 官方产品，与 Mojang 和 Microsoft 无关</small></div>
                <div class="v"><button type="button" class="btn sm" data-act="about">许可和第三方作品</button></div></div>
        </section>
        <p class="note">LeafStarter 交互原型 · 界面是真的，数据都是示例。</p>
    </div>`;
}

// ---------- 设置：AI 源、MCP、功能包、联机 ----------
function aiPanel(c) {
    const S = c.S;
    const L = S.ai.list;
    const perm = [
        ['read', '列实例、读日志、查 Mod、看服务器状态', '直接做'],
        ['rev', '禁用 / 更新 Mod、改内存和配置', '先建快照；逐条问，可选「这次会话都允许」'],
        ['ext', '启动游戏、在服务器上执行命令、调云接口', '每次都问，给你看完整命令'],
        ['irr', '删世界、删服务器上的文件', '每次都问并写明后果，先备份'],
    ];
    const useOf = (id) => USES.filter(([u]) => S.ai.use[u] === id).map(([, t]) => t);
    const rows = L.map((s) => {
        const p = PRESETS[s.p];
        const t = s.test;
        const st = !ready(s) ? '<span class="chip warn">没填 Key</span>' : t?.state === 'run' ? '<span class="chip acc">测试中…</span>' : t?.state === 'ok' ? `<span class="chip ok">${icon('check', 12)}${t.ms} ms</span>` : t?.state === 'fail' ? `<span class="chip bad">${esc(t.msg)}</span>` : '';
        const open = S.ai.edit === s.id;
        return `<div class="src${open ? ' open' : ''}" data-src="${s.id}">
            <button type="button" class="src-h" data-act="ais-edit" data-id="${s.id}" aria-expanded="${open}">
                <span class="ico ${p.kind === 'image' ? 'amber' : p.local ? 'sky' : 'violet'}">${icon(p.kind === 'image' ? 'image' : p.local ? 'monitor' : 'cloud', 16)}</span>
                <span class="tx"><span class="nm" data-srcname="${s.id}">${esc(s.name)}</span><span class="sb">${KIND[p.kind]} · <span class="mono">${esc(s.model || '没填模型')}</span></span></span>
                <span class="tags">${useOf(s.id).map((u) => `<span class="chip">${u}</span>`).join('')}${st}</span>
                <span class="chev">${icon('chevron', 15)}</span>
            </button>
            ${open ? srcEditor(c, s) : ''}
        </div>`;
    }).join('');
    const uses = USES.map(([u, t, h]) => {
        const opts = L.filter((s) => (u === 'img') === isImg(s));
        const cur = S.ai.use[u] || '';
        const warn = u === 'chat' && cur && PRESETS[L.find((s) => s.id === cur)?.p]?.unknown ? '<small class="warnt">认不出能不能调工具，测试一下</small>' : '';
        return `<div class="use"><span class="tx"><b>${t}</b><small>${h}</small>${warn}</span>
            <select data-use="${u}" aria-label="${t}用哪个源">${u === 'img' && !opts.length ? '<option value="">没有生图源（皮肤只用「描述 → 程序画」）</option>' : `<option value="">${u === 'chat' ? '不用 AI' : '不用'}</option>${opts.map((s) => `<option value="${s.id}"${cur === s.id ? ' selected' : ''}>${esc(s.name)}${ready(s) ? '' : '（没填 Key）'}</option>`).join('')}`}</select></div>`;
    }).join('');
    return `<section class="panel" id="aiPanel"><h2 class="ph">AI</h2>
        <div class="kv"><div class="k"><b>AI 源</b><small>云端各家、本机的 Ollama / llama.cpp / LM Studio，再加生图接口。Key 只存在本机的加密保管库里，AI 和 MCP 都拿不到明文；费用你和服务商直接结。不配也行：常见崩溃靠规则、Mod 冲突靠二分、开服靠固定剧本</small></div>
            <div class="v"><button type="button" class="btn sm" data-act="ais-add">${icon('plus', 13)}加一个</button></div></div>
        ${L.length ? `<div class="srcs">${rows}</div>` : '<div class="kv"><div class="k"><small>还没有 AI 源。</small></div><div class="v"><button type="button" class="btn sm" data-act="ai-demo">用示例 Key 试试（原型）</button></div></div>'}
        <div class="kv stack"><div class="k"><b>按用途分配</b><small>排查问题用强模型，翻译用便宜的，生图用图像模型；默认全用同一个</small></div>
            <div class="uses">${uses}</div></div>
        <div class="kv"><div class="k"><b>每次会话最多用</b><small>到了就停下来问你；按源分别统计</small></div>
            <div class="v">${seg('set', [['2w', '2 万 token'], ['10w', '10 万'], ['none', '不限']], S.aiCap, 'data-k="aiCap"')}</div></div>
        <div class="kv"><div class="k"><b>发出去之前可以先看</b><small>不管开不开，用户名路径、令牌、IP、密码都会先打码</small></div>
            <div class="v"><input type="checkbox" class="switch" data-set="aiPreview"${S.aiPreview ? ' checked' : ''} aria-label="发出前可以预览"></div></div>
        <div class="perm">${perm.map(([k, what, how]) => `<div class="pr">${lvChip(k)}<span>${what}</span><b>${how}</b></div>`).join('')}
            <div class="pr"><span class="lv no">${icon('x', 12)}不开放</span><span>读密钥明文、付款、改密码</span><b>工具里根本没有</b></div></div>
    </section>`;
}

const EFF = { low: '低', medium: '中', high: '高' };
function srcEditor(c, s) {
    const p = PRESETS[s.p];
    const P = paramsFor(s);
    const f = (k, label, val, ph = '', cls = 'wide') => `<label class="sf"><span>${label}</span><input type="text" class="${cls}" data-ais="${s.id}" data-f="${k}" value="${esc(val)}" placeholder="${esc(ph)}" autocomplete="off"></label>`;
    const prm = P.list.map((k) => {
        const [label, type, a, b, step, def] = PARAMS[k];
        const v = s.prm[k];
        if (type === 'range') return `<label class="sf"><span>${label} <b class="mono" data-rv="${s.id}:${k}">${v ?? def}</b></span><input type="range" min="${a}" max="${b}" step="${step}" value="${v ?? def}" data-ais="${s.id}" data-f="prm:${k}"></label>`;
        if (type === 'seg') return `<div class="sf"><span>${label}</span>${seg('ais-effort', a.map((x) => [x, EFF[x]]), v ?? b, `data-id="${s.id}"`)}</div>`;
        if (type === 'num') return f('prm:' + k, label + (b ? `（${b}）` : ''), v ?? a, '', 'wide mono');
        return f('prm:' + k, label, v ?? a, b);
    }).join('');
    const t = s.test;
    const caps = t?.state === 'ok'
        ? `<span class="caps"><span class="chip ok">${icon('check', 12)}能连上 · ${t.ms} ms</span>${t.kind === 'image' ? '' : t.tools == null ? '<span class="chip warn">没测出能不能调工具</span>' : t.tools ? '<span class="chip ok">能调工具</span>' : '<span class="chip">不能调工具</span>'}${t.kind === 'image' ? '' : t.vision ? '<span class="chip ok">能看图</span>' : t.vision === false ? '<span class="chip">不能看图</span>' : ''}</span>`
        : '';
    return `<div class="src-b">
        <div class="sfs">
            ${f('name', '名字', s.name)}
            ${f('base', '地址', s.base, p.kind === 'image' ? 'http://127.0.0.1:8188' : 'https://…/v1', 'wide mono')}
            ${p.local ? '' : `<label class="sf"><span>API Key <span class="chip${s.key ? ' ok' : ''}" data-keyst="${s.id}">${s.key ? '已存进保管库' : '没填'}</span></span><input type="password" class="wide" data-ais="${s.id}" data-f="key" placeholder="${s.key ? '已保存（原型里填什么都行）' : '粘贴 Key（原型里填什么都行）'}" autocomplete="off"></label>`}
            ${f('model', '模型', s.model, p.kind === 'image' ? '工作流或模型名' : '按服务商给的模型名填', 'wide mono')}
        </div>
        ${p.models ? `<div class="mchips">${p.models.map((m) => `<button type="button" class="chip${s.model === m ? ' acc' : ''}" data-act="ais-model" data-id="${s.id}" data-v="${m}">${m}</button>`).join('')}</div>` : ''}
        <div class="actions"><button type="button" class="btn sm" data-act="ais-test" data-id="${s.id}"${t?.state === 'run' ? ' disabled' : ''}>${icon('zap', 13)}${t?.state === 'run' ? '测试中…' : '测试连接'}</button>${caps}${t?.state === 'fail' ? `<span class="chip bad">${esc(t.msg)}</span>` : ''}<span class="sp"></span><button type="button" class="btn sm ghost danger" data-act="ais-del" data-id="${s.id}">${icon('trash', 13)}删掉这个源</button></div>
        <details class="adv"${P.warn ? ' open' : ''}><summary>${icon('sliders', 13)}高级参数 · ${P.list.length} 项</summary>
            ${P.note ? `<p class="callout${P.warn ? ' warn' : ''}">${icon(P.warn ? 'alert' : 'check', 14)}<span>${P.note}</span></p>` : ''}
            <div class="sfs">${prm}</div>
        </details>
    </div>`;
}

function mcpPanel(c) {
    const S = c.S;
    const snip = '{\n  "mcpServers": {\n    "leafstarter": { "command": "leafstarter", "args": ["--mcp"] }\n  }\n}';
    const calls = [
        ['Claude 桌面版', 'list_instances', 'read', '3 分钟前', ''],
        ['Claude 桌面版', 'set_memory「生存一服」6 GB', 'rev', '2 分钟前', ''],
        ['Cursor', 'launch「生存一服」', 'ext', '1 分钟前', '你在 LeafStarter 里点了允许'],
    ];
    return `<section class="panel"><h2 class="ph">MCP</h2>
        <div class="kv"><div class="k"><b>让别的 AI 客户端操作 LeafStarter</b><small>Claude Code、Codex、Claude 桌面版、Cursor、Cherry Studio 等，支持 MCP 的都行。比如对它说「把 1.20.1 那个实例的内存调到 6G，然后启动」</small></div>
            <div class="v"><input type="checkbox" class="switch" data-set="mcp"${S.mcp ? ' checked' : ''} aria-label="开放 MCP"></div></div>
        ${S.mcp ? `
        <div class="kv"><div class="k"><b>开放哪几级</b><small>「影响外部」和「不可撤回」始终在 LeafStarter 自己的窗口里弹确认，不让外部客户端代答</small></div>
            <div class="v"><label class="tick"><input type="checkbox" checked disabled aria-label="只读（一直开放）">${lvChip('read')}</label><label class="tick"><input type="checkbox" data-set="mcpRev"${S.mcpRev ? ' checked' : ''} aria-label="可撤回">${lvChip('rev')}</label><span class="tick">${lvChip('ext')}<small>要你确认</small></span></div></div>
        <div class="kv stack"><div class="k"><b>一键接入</b><small>找到的客户端，由 LeafStarter 把配置写进去；写之前给你看改哪几行</small></div>
            <div class="clients">${MCP_CLIENTS.map(([id, name, path, found]) => `<div class="client"><span class="ico ${found ? 'violet' : ''}">${icon('plug', 15)}</span><span class="tx"><span class="nm">${name}</span><span class="sb mono">${found ? esc(path) : '这台电脑上没找到'}</span></span>${!found ? '<button type="button" class="btn sm" disabled>写入配置</button>' : S.mcpw[id] ? `<span class="chip ok">${icon('check', 12)}已接入</span><button type="button" class="btn sm ghost" data-act="mcp-unwrite" data-v="${id}">移除</button>` : `<button type="button" class="btn sm" data-act="mcp-write" data-v="${id}">写入配置</button>`}</div>`).join('')}</div></div>
        <div class="kv stack"><div class="k"><b>其他支持 MCP 的客户端</b><small>Cherry Studio、各种 Code 类工具……把这段加进它的 MCP 配置。它通过 stdio 连到正在运行的 LeafStarter；也可以开本机 HTTP（只听 127.0.0.1，带随机令牌）</small></div>
            <div class="snipbox"><pre class="snip mono" id="mcpSnip">${esc(snip)}</pre><button type="button" class="btn sm" data-act="mcp-copy">${icon('copy', 13)}复制</button></div></div>
        <div class="kv stack"><div class="k"><b>最近的调用</b><small>示例记录</small></div>
            <div class="calls">${calls.map(([who, what, lv, at, note]) => `<div class="call">${icon('plug', 14)}<span class="who">${who}</span><span class="mono what">${esc(what)}</span>${lvChip(lv)}<span class="sb">${at}${note ? ` · ${note}` : ''}</span></div>`).join('')}</div></div>` : ''}
    </section>`;
}

function mpPanel(c) {
    const S = c.S;
    const vault = S.ai.list.filter((s) => s.key).length + (S.sakura === true ? 1 : 0) + c.INSTANCES.filter((i) => i.srv).length;
    return `<section class="panel"><h2 class="ph">联机</h2>
        <div class="kv"><div class="k"><b>P2P 中继节点</b><small>打不通直连时才走中继。LeafStarter 自己不运营节点</small></div>
            <div class="v">${seg('set', [['public', 'EasyTier 社区节点'], ['own', '我自己的节点']], S.relay, 'data-k="relay"')}</div></div>
        <div class="kv"><div class="k"><b>樱花账号</b><small>朋友没装 LeafStarter 时，用你自己的樱花账号建穿透隧道</small></div>
            <div class="v">${S.sakura === true ? `<span class="chip ok">${icon('check', 13)}已连接</span><button type="button" class="btn sm ghost" data-act="mp-sakura-off">断开</button>` : S.sakura === 'busy' ? '<button type="button" class="btn sm" disabled>正在连…</button>' : '<button type="button" class="btn sm" data-act="mp-sakura">连接</button>'}</div></div>
        <div class="kv"><div class="k"><b>凭据保管库</b><small>SSH 密钥、云 AccessKey、面板 Key、樱花和模型的 Key，加密存在本机。AI 和 MCP 只能「用」，拿不到明文</small></div>
            <div class="v"><span class="chip">${icon('key', 13)}${vault} 项</span></div></div>
    </section>`;
}

// 功能包和插件都搬到了「插件」页；这里只留简洁界面和一个入口
function plugPanel(c) {
    const S = c.S;
    return `<section class="panel" id="extPanel"><h2 class="ph">插件</h2>
        <div class="kv"><div class="k"><b>插件和功能包</b><small>官方功能包（联机、世界工具、离线翻译）和第三方插件都在「插件」页；那里还有插件市场和定制版制作器</small></div>
            <div class="v"><span class="chip">${c.plugCount()} 个第三方插件在用</span><button type="button" class="btn sm" data-page="plugins">${icon('plug', 13)}去插件页</button></div></div>
        <div class="kv"><div class="k"><b>简洁界面</b><small>把不常用的入口藏起来：AI 助手按钮、截图标签，还有没装的功能包的入口。和装没装功能包无关</small></div>
            <div class="v"><input type="checkbox" class="switch" data-set="simple"${S.simple ? ' checked' : ''} aria-label="简洁界面"></div></div>
    </section>`;
}

function worldPanel(c) {
    return `<section class="panel"><h2 class="ph">存档工具</h2>
        <div class="kv"><div class="k"><b>局外指令台</b><small>不开游戏也能对世界执行命令：后台起一个看不见的本地服务器，执行前先建快照。默认关着</small></div>
            <div class="v"><input type="checkbox" class="switch" data-set="console"${c.S.console ? ' checked' : ''} aria-label="局外指令台"></div></div>
        <div class="kv"><div class="k"><b>改世界之前自动备份</b><small>局外修改、指令台、套用设置都会先备份，这一项不能关</small></div>
            <div class="v"><span class="chip ok">${icon('check', 12)}一直开着</span></div></div>
    </section>`;
}

// ---------- 新建实例（侧边面板，主按钮和启动按钮在同一个位置） ----------
export function drawerView(c) {
    const d = c.S.drawer;
    if (!d) return '';
    const lv = loaderVersion(d.loader, d.mc);
    const bad = d.loader && lv === null;
    const loaderNote = !d.loader ? '不装加载器，就是原版' : bad ? `${d.loader} 不支持 ${d.mc}，换个版本或加载器` : `${d.loader} ${lv}（推荐版本）`;
    return `<section class="drawer" role="dialog" aria-label="新建实例">
        <header class="dr-head"><h2>新建实例</h2><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
        <div class="dr-body">
            <label class="field" for="drName"><span>名字</span><input id="drName" type="text" value="${esc(d.name)}" autocomplete="off"></label>
            <div class="field"><span>游戏版本</span>
                <label class="filter" for="drVq">${icon('search', 14)}<input id="drVq" type="text" placeholder="筛选版本，比如 1.20" value="${esc(d.vq)}" autocomplete="off"></label>
                <div class="vlist" id="drVlist" role="listbox" aria-label="游戏版本">${vlist(c)}</div>
                <small class="note">示例列表，正式版读官方版本清单。</small>
            </div>
            <div class="field"><span>Mod 加载器</span>
                ${seg('dr-loader', [['', '原版'], ['Fabric', 'Fabric'], ['Forge', 'Forge'], ['NeoForge', 'NeoForge'], ['Quilt', 'Quilt']], d.loader || '')}
                <small class="note${bad ? ' bad' : ''}">${esc(loaderNote)}</small>
            </div>
            <label class="check" for="drPerf"><input type="checkbox" id="drPerf"${d.perf && d.loader ? ' checked' : ''}${d.loader ? '' : ' disabled'}>
                <span><b>流畅优先</b><small>${d.loader ? '自动加上 Sodium、Lithium 这类优化 Mod，随时能撤掉' : '原版装不了优化 Mod'}</small></span></label>
            <label class="check" for="drSync"><input type="checkbox" id="drSync"${d.sync ? ' checked' : ''}>
                <span><b>手机电脑同步</b><small>装好后同步到你的手机启动器</small></span></label>
            <label class="field" for="drDir"><span>装在哪个游戏目录</span>
                <select id="drDir">${c.DIRS.map((x) => `<option value="${x.id}"${d.dir === x.id ? ' selected' : ''}>${esc(x.name)} · ${esc(x.path)}</option>`).join('')}</select></label>
        </div>
        <footer class="dr-foot">
            <span class="hint">装的时候可以继续做别的事</span>
            <button type="button" class="go" id="drCreate" data-act="dr-create"${bad || !d.name.trim() ? ' disabled' : ''}>${icon('download', 22)}<span class="txt"><span class="lbl">创建并安装</span><span class="sub">${esc(d.mc)} · ${d.loader ? esc(d.loader) : '原版'}</span></span><kbd>Enter</kbd></button>
        </footer>
    </section>`;
}

export function vlist(c) {
    const d = c.S.drawer;
    const q = d.vq.trim();
    const list = c.VERSIONS.filter(([v]) => !q || v.includes(q));
    if (!list.length) return `<div class="vitem empty">没有「${esc(q)}」</div>`;
    return list.map(([v, date]) => `<button type="button" class="vitem${v === d.mc ? ' on' : ''}" data-act="dr-mc" data-v="${v}" role="option" aria-selected="${v === d.mc}"><span class="mono">${v}</span><span class="sb">${date}</span></button>`).join('');
}
