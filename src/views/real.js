// 真程序里和原型不一样的几块界面：数据都来自后端，还没做的功能不放假数据，直接说在做
import { esc, ago, block } from '../ui.js';
import { icon } from '../icons.js';

const ico = (tone, name) => `<span class="ico ${tone}">${icon(name, 16)}</span>`;
const chip = (cls, text) => `<span class="chip ${cls}">${esc(text)}</span>`;
const seg = (act, list, cur, extra = '') => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="${act}" data-v="${v}" ${extra} class="${String(cur) === v ? 'on' : ''}" aria-pressed="${String(cur) === v}">${t}</button>`).join('')}</div>`;
const loadingCard = (what) => `<section class="card"><h3>${ico('sky', 'refresh')}正在读${what}</h3><p>从游戏目录里读，一般一两秒。</p></section>`;
const errorCard = (i) => `<section class="card"><h3>${ico('bad', 'alert')}读不到这个实例的文件</h3><p>${esc(i.loadError)}</p>
    <div class="actions"><button type="button" class="btn" data-act="real-rescan">${icon('refresh')}重新扫描</button></div></section>`;

// ---------- 实例：概览 ----------
export function realOverview(c) {
    const i = c.inst;
    const dir = c.DIRS.find((d) => d.id === i.dir);
    if (i.installing) {
        return `<section class="card"><h3>${ico('sky', 'download')}正在安装${chip('acc', `${i.installing.pct}%`)}</h3>
            <p>${esc(i.installing.stage)}。进度也在右下角的按钮和「任务」页里，装的时候可以做别的事。</p></section>`;
    }
    if (i.loadError) return errorCard(i);
    const loading = !i.loaded;
    const off = i.mods.filter((m) => !m.on).length;
    const facts = `<dl class="facts">
        <dt>游戏版本</dt><dd class="mono">${esc(i.mc)}</dd>
        <dt>加载器</dt><dd>${i.loader ? `${esc(i.loader)} <span class="mono">${esc(i.loaderVer)}</span>` : '原版'}</dd>
        <dt>游戏目录</dt><dd class="mono path">${esc(i.gameDir || dir?.path || '')}</dd>
        <dt>版本隔离</dt><dd>${i.isolatedDir ? '开着：存档和 Mod 只属于这个版本' : '没开：和这个目录里的其他版本共用存档和 Mod'}</dd>
        <dt>需要 Java</dt><dd>${i.javaNeed} 或更高</dd>
    </dl>`;
    const modCard = !i.loader
        ? `<section class="card"><h3>${ico('amber', 'package')}Mod${chip('', '原版')}</h3><p>原版装不了 Mod。想装 Mod，就新建一个带 Fabric、Forge 或 NeoForge 的实例。</p></section>`
        : `<section class="card"><h3>${ico('amber', 'package')}Mod${loading ? '' : chip(off ? 'warn' : 'ok', off ? `${off} 个禁用` : '都启用着')}</h3>
            <p>${loading ? '正在读…' : i.mods.length ? `一共 ${i.mods.length} 个。可以在「Mod」里逐个开关。` : 'mods 文件夹里还没有 Mod。'}</p>
            <div class="actions"><button type="button" class="btn" data-act="tab" data-tab="mods">${icon('list')}管理 Mod</button><button type="button" class="btn" data-act="folder" data-v="mods">${icon('folder')}mods 文件夹</button></div></section>`;
    const worlds = i.worlds.slice(0, 3);
    const worldCard = `<section class="card"><h3>${ico('sky', 'globe')}存档${loading ? '' : chip('', `${i.worlds.length} 个`)}</h3>
        ${loading ? '<p>正在读…</p>' : worlds.length
            ? `<div class="mini-rows">${worlds.map((w) => `<div class="mrow"><span class="tx"><span class="nm">${esc(w.name)}</span><span class="sb">最后保存 ${ago(w.last)}</span></span></div>`).join('')}</div>`
            : '<p>还没有存档。进游戏建一个世界，就会出现在这里。</p>'}
        ${!loading && i.shotCount ? `<p class="note">还有 ${i.shotCount} 张截图在 screenshots 文件夹里。</p>` : ''}
        <div class="actions"><button type="button" class="btn" data-act="folder" data-v="saves">${icon('folder')}存档文件夹</button></div></section>`;
    const howCard = `<section class="card"><h3>${ico('violet', 'play')}怎么玩</h3>
        <p>LeafStarter 自己启动游戏的功能正在做（登录、补全文件、自动选 Java）。${dir?.exe ? `现在点右下角会打开 <b>${esc(dir.name)}</b>，在里面选「${esc(i.name)}」就能玩。` : '现在先用原来的启动器玩这个版本。'}</p></section>`;
    return `<div class="actions">
        <button type="button" class="btn" data-act="folder" data-v="">${icon('folder')}打开文件夹</button>
        <button type="button" class="btn" data-act="real-rescan">${icon('refresh')}重新扫描</button>
    </div>
    <div class="grid2"><section class="card"><h3>${ico('ok', 'layers')}这个版本</h3>${facts}</section>${modCard}${worldCard}${howCard}</div>`;
}

// ---------- 实例：Mod 和存档还在读的时候 ----------
export function realPending(c, what) {
    const i = c.inst;
    if (i.installing) return realOverview(c);
    if (i.loadError) return errorCard(i);
    return i.loaded ? '' : loadingCard(what);
}

// ---------- 实例：存档 ----------
export function realWorlds(c) {
    const i = c.inst;
    const wait = realPending(c, '存档');
    if (wait) return wait;
    const head = `<div class="toolbar">
        <p class="note grow">地图、种子、查找建筑和局外修改在做；直接进入某个世界要等 LeafStarter 能自己启动游戏。</p>
        <button type="button" class="btn" data-act="folder" data-v="saves">${icon('folder')}存档文件夹</button>
    </div>`;
    if (!i.worlds.length) return head + '<section class="card"><h3>还没有存档</h3><p>进游戏建一个世界，就会出现在这里。</p></section>';
    return `${head}<div class="rows">${i.worlds.map((w) => `<div class="row world">
        ${block('grass', 32)}
        <span class="tx"><span class="nm">${esc(w.name)}</span><span class="sb">最后保存 ${ago(w.last)}</span></span>
    </div>`).join('')}</div>`;
}

// ---------- 没有实例 ----------
export function realEmpty(c) {
    const dirs = c.DIRS.filter((d) => d.access === 'direct');
    return `<div class="page"><section class="card"><h3>${dirs.length ? '这些游戏目录里还没有版本' : '没找到游戏目录'}</h3>
        <p>${dirs.length ? `找到了 ${dirs.map((d) => esc(d.name)).join('、')} 的目录，但里面还没有装好的版本。` : 'LeafStarter 会自动找 PCL、HMCL、官方启动器的游戏目录。没找到的话，可以手动选一个 .minecraft 文件夹。'}</p>
        <div class="actions"><button type="button" class="btn pri" data-act="new-inst"${dirs.length ? '' : ' disabled'}>${icon('plus')}新建实例</button><button type="button" class="btn" data-act="real-add-dir">${icon('folder')}选一个 .minecraft 文件夹</button><button type="button" class="btn" data-act="real-rescan">${icon('refresh')}重新扫描</button></div>
    </section></div>`;
}

// ---------- 下载：先只有游戏版本 ----------
const TYPE = { release: '正式版', snapshot: '快照', old_beta: '远古 Beta', old_alpha: '远古 Alpha' };

export function realDownload(c) {
    return `<div class="page" id="page">
        <div class="page-h"><h1>下载</h1></div>
        <p class="note">Mod、整合包、资源包和光影的搜索在做（接 Modrinth 和 CurseForge）。现在可以先装游戏版本，装好就是一个新实例。</p>
        <div class="toolbar">
            <label class="filter grow" for="dlVq">${icon('search', 14)}<input id="dlVq" type="text" placeholder="筛选版本，比如 1.20" value="${esc(c.S.dlvq)}" autocomplete="off"></label>
            ${seg('dlv', [['release', '正式版'], ['snapshot', '快照'], ['old', '远古版本']], c.S.dlv)}
        </div>
        <div class="rows" id="dlList">${realDlRows(c)}</div>
    </div>`;
}

export function realDlRows(c) {
    const m = c.R.manifest;
    if (!m) {
        if (c.R.manifestError) return `<div class="row empty">官方版本清单拿不到：${esc(c.R.manifestError)} <button type="button" class="btn sm" data-act="dl-retry">重试</button></div>`;
        return Array.from({ length: 6 }, () => '<div class="row dl skel" aria-hidden="true"><span class="sk sq"></span><span class="sk ln"></span><span class="sk bt"></span></div>').join('');
    }
    const q = c.S.dlvq.trim().toLowerCase();
    const t = c.S.dlv;
    const all = m.versions.filter((v) => (q ? v.id.toLowerCase().includes(q) : t === 'old' ? v.type.startsWith('old') : v.type === t));
    if (!all.length) return `<div class="row empty">没有「${esc(c.S.dlvq)}」</div>`;
    const list = all.slice(0, 80);
    return list.map((v) => {
        const latest = v.id === m.latest.release || v.id === m.latest.snapshot;
        return `<div class="row dl">
            ${block(v.type === 'release' ? 'grass' : v.type === 'snapshot' ? 'copper' : 'cobble', 30)}
            <span class="tx"><span class="nm mono">${esc(v.id)}${latest ? ' <span class="chip ok">最新</span>' : ''}</span><span class="sb">${TYPE[v.type] || v.type} · ${esc(v.releaseTime.slice(0, 10))}</span></span>
            <button type="button" class="btn sm pri" data-act="dl-mc" data-v="${esc(v.id)}">${icon('plus', 13)}装成新实例</button>
        </div>`;
    }).join('') + (all.length > list.length ? `<div class="row empty">还有 ${all.length - list.length} 个，输入版本号筛选</div>` : '');
}

// ---------- 设置 ----------
const KEYS = [
    ['Enter', '启动选中的实例（现在是用原来的启动器打开）'],
    ['↑ ↓', '换一个实例'],
    ['Ctrl K', '命令面板：搜实例，或者直接输入要做的事'],
    ['/', '筛选左边的实例'],
    ['Esc', '关掉命令面板、侧边面板和弹出的小窗'],
    ['Ctrl 1 2 3 5', '实例 / 下载 / 任务 / 设置（Ctrl 4 的插件页在做）'],
];

export function realSettings(c) {
    const S = c.S;
    const info = c.R.info || {};
    return `<div class="page" id="page">
        <div class="page-h"><h1>设置</h1></div>

        <section class="panel"><h2 class="ph">外观</h2>
            <div class="kv"><div class="k"><b>主题</b></div>
                <div class="v">${seg('set', [['system', '跟随系统'], ['light', '浅色'], ['dark', '深色']], S.theme, 'data-k="theme"')}</div></div>
            <div class="kv"><div class="k"><b>动效</b><small>${window.matchMedia('(prefers-reduced-motion: reduce)').matches ? '你的系统关了「动画效果」，选「跟随系统」启动器就不动了' : '「精简」会关掉所有过渡和特效'}</small></div>
                <div class="v">${seg('set', [['full', '完整'], ['reduced', '精简'], ['system', '跟随系统']], S.motion, 'data-k="motion"')}</div></div>
        </section>

        ${sourcesPanel(c)}

        <section class="panel"><h2 class="ph">游戏目录</h2>
            <p class="note">自动找到的 PCL、HMCL、官方启动器的目录，加上你手动添加的。新建实例时选装在哪个目录里。</p>
            <div class="rows">${c.DIRS.length ? c.DIRS.map((d) => `<div class="row dir">
                <span class="tile" style="--h:${(d.name.charCodeAt(0) * 37) % 360}">${esc(d.name[0] || '?')}</span>
                <span class="tx"><span class="nm">${esc(d.name)}</span><span class="sb mono">${esc(d.path)}</span></span>
                <span class="tags">${chip('', `${d.count} 个版本`)}${d.access === 'direct' ? '' : chip('warn', '不能写入')}</span>
                ${d.access === 'direct' ? `<button type="button" class="btn sm" data-act="dir-open" data-id="${esc(d.id)}">${icon('folder', 13)}打开</button>` : ''}
                ${d.manual ? `<button type="button" class="btn sm ghost danger" data-act="dir-del" data-id="${esc(d.id)}">移除</button>` : ''}
            </div>`).join('') : '<div class="row empty">还没有游戏目录</div>'}</div>
            <div class="actions"><button type="button" class="btn" data-act="real-add-dir">${icon('plus')}添加 .minecraft 文件夹</button><button type="button" class="btn" data-act="real-rescan">${icon('refresh')}重新扫描</button></div>
        </section>

        <section class="panel"><h2 class="ph">Java</h2>
            <p class="note">本机找到的 Java（含 PCL、HMCL、官方启动器下载的运行时）。装 Forge 时会按版本要求自动挑，没有合适的就下载官方运行时。</p>
            <div class="rows">${c.JAVAS.length ? c.JAVAS.map((j) => `<div class="row java"><span class="tile" style="--h:28">${esc(String(j.major || '?'))}</span><span class="tx"><span class="nm">${esc(j.label)}</span><span class="sb mono">${esc(j.path)}</span></span></div>`).join('') : '<div class="row empty">本机没找到 Java</div>'}</div>
            <div class="actions"><button type="button" class="btn" data-act="java-refresh">${icon('refresh')}重新检测</button></div>
        </section>

        <section class="panel"><h2 class="ph">快捷键</h2>
            <div class="keys">${KEYS.map(([k, t]) => `<span class="kk">${k.split(' ').map((x) => `<kbd>${x}</kbd>`).join('')}</span><span>${t}</span>`).join('')}</div>
        </section>
        <section class="panel"><h2 class="ph">关于</h2>
            <div class="kv"><div class="k"><b>LeafStarter ${esc(info.version || '')}</b><small>Apache-2.0 · Copyright 2026 PingYi · 不是 Minecraft 官方产品，与 Mojang 和 Microsoft 无关</small></div>
                <div class="v"><button type="button" class="btn sm" data-act="about">许可和第三方作品</button></div></div>
            <div class="kv"><div class="k"><b>数据目录</b><small class="mono">${esc(info.dataDir || '')}</small></div>
                <div class="v"><button type="button" class="btn sm" data-act="data-open">${icon('folder', 13)}打开</button></div></div>
        </section>
    </div>`;
}

function sourcesPanel(c) {
    const s = c.R.sources;
    if (!s) return '<section class="panel"><h2 class="ph">下载源</h2><p class="note">正在读下载源…</p></section>';
    const st = s.settings;
    const max = Math.max(1, ...s.sources.map((x) => x.liveSpeed || x.testSpeed || 0));
    const speed = (b) => (b ? (b >= 1048576 ? (b / 1048576).toFixed(1) + ' MB/s' : Math.round(b / 1024) + ' KB/s') : '—');
    return `<section class="panel"><h2 class="ph">下载源</h2>
        <div class="kv"><div class="k"><b>怎么选源</b><small>智能：官方源和国内镜像都测速，下载时按每个文件的实际表现随时换；固定：只用你选的那个</small></div>
            <div class="v">${seg('rs-mode', [['auto', '智能'], ['fixed', '固定']], st.mode)}${st.mode === 'fixed' ? `<select id="rsPref" aria-label="固定用哪个源">${s.sources.map((x) => `<option value="${esc(x.id)}"${x.id === st.preferred ? ' selected' : ''}>${esc(x.name)}</option>`).join('')}</select>` : ''}</div></div>
        <div class="kv"><div class="k"><b>同时下载</b><small>个文件</small></div>
            <div class="v"><input type="range" id="rsThreads" min="4" max="64" step="4" value="${st.threads}" aria-label="同时下载的文件数"><b class="mono" id="rsThreadsVal">${st.threads}</b></div></div>
        <div class="rows">${s.sources.map((x, k) => {
            const sp = x.liveSpeed || x.testSpeed;
            return `<div class="row src-row">
                <span class="tile" style="--h:${k === 0 ? 150 : 210}">${k === 0 ? '★' : esc(String(x.rank))}</span>
                <span class="tx"><span class="nm">${esc(x.name)}${k === 0 ? ' <span class="chip ok">首选</span>' : ''}${x.cooling ? ' <span class="chip warn">冷却中</span>' : ''}</span>
                    <span class="sb">${esc(x.kind === 'official' ? 'Mojang / Fabric / Forge / NeoForge 官方' : x.base)}${x.lastError ? ` · 最近错误：${esc(x.lastError)}` : ''}</span>
                    <span class="bar"><i style="--p:${sp ? Math.max(4, (sp / max) * 100) : 0}%"></i></span></span>
                <span class="tags"><span class="chip mono">${x.latencyMs != null ? x.latencyMs + ' ms' : '—'}</span><span class="chip mono">${speed(sp)}</span><span class="chip mono">${x.ok}/${x.ok + x.fail}</span></span>
                ${x.builtin ? '' : `<button type="button" class="btn sm ghost danger" data-act="rs-del" data-id="${esc(x.id)}">删除</button>`}
            </div>`;
        }).join('')}</div>
        <div class="actions"><button type="button" class="btn" data-act="rs-test" id="rsTest">${icon('zap')}重新测速</button></div>
        <div class="kv"><div class="k"><b>加一个镜像</b><small>BMCLAPI 格式的镜像地址，比如自建的镜像站</small></div>
            <div class="v"><input type="text" id="rsName" class="wide" placeholder="名字" autocomplete="off"><input type="text" id="rsBase" class="wide mono" placeholder="https://…" autocomplete="off"><button type="button" class="btn sm" data-act="rs-add">添加</button></div></div>
    </section>`;
}

