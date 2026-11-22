// 下载、任务、设置三页，以及「新建实例」侧边面板
import { esc, ago, head } from '../ui.js';
import { icon } from '../icons.js';
import { loaderVersion } from '../data.js';

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
    ['Ctrl 1–4', '实例 / 下载 / 任务 / 设置（浏览器里这几个键被标签页占了，用 Alt 1–4）'],
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
                <div class="v">${S.account === a.id ? '<span class="chip ok">默认账户</span>' : `<button type="button" class="btn sm" data-act="acct-switch" data-id="${a.id}">设为默认</button>`}</div></div>`).join('')}
            <div class="kv"><div class="k"><small>微软账户用设备代码登录：在手机或浏览器上输入一串代码就行。</small></div>
                <div class="v"><button type="button" class="btn sm" data-act="acct-add">${icon('plus', 13)}微软账户</button><button type="button" class="btn sm" data-act="acct-add">${icon('plus', 13)}离线账户</button></div></div>
        </section>

        <section class="panel"><h2 class="ph">外观</h2>
            <div class="kv"><div class="k"><b>主题</b></div>
                <div class="v">${seg('set', [['system', '跟随系统'], ['light', '浅色'], ['dark', '深色']], S.theme, 'data-k="theme"')}</div></div>
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

        <section class="panel"><h2 class="ph">快捷键</h2>
            <div class="keys">${KEYS.map(([k, t]) => `<span class="kk">${k.split(' ').map((x) => `<kbd>${x}</kbd>`).join('')}</span><span>${t}</span>`).join('')}</div>
        </section>
        <p class="note">LeafStarter 交互原型 · 界面是真的，数据都是示例。</p>
    </div>`;
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
