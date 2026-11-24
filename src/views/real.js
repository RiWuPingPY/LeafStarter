// 真程序里和原型不一样的几块界面：数据都来自后端，还没做的功能不放假数据，直接说在做
import { esc, ago, block, head, clock, hours } from '../ui.js';
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
    const howCard = runCard(c, i);
    const busy = !!c.RUN.get(i.id);
    return `${realCrashBanner(c)}<div class="actions">
        <button type="button" class="btn" data-act="folder" data-v="">${icon('folder')}打开文件夹</button>
        <button type="button" class="btn" data-act="inst-rename"${busy ? ' disabled' : ''}>${icon('keyboard')}改名</button>
        <button type="button" class="btn" data-act="inst-copy"${busy ? ' disabled' : ''}>${icon('copy')}复制一份</button>
        <button type="button" class="btn" data-act="inst-shortcut">${icon('monitor')}桌面快捷方式</button>
        <button type="button" class="btn ghost danger" data-act="inst-del"${busy ? ' disabled' : ''}>${icon('trash')}删除</button>
    </div>
    <div class="grid2"><section class="card"><h3>${ico('ok', 'layers')}这个版本</h3>${facts}</section>${modCard}${worldCard}${howCard}</div>`;
}

// 运行状态：运行中显示时长，没在跑就说上次怎样
function runCard(c, i) {
    const r = c.RUN.get(i.id);
    let body;
    if (r?.phase === 'running') body = `${chip('ok', '运行中')}</h3><p>已经玩了 <b class="mono" id="runClock">${clock(Math.floor((Date.now() - r.started) / 1000))}</b>。关掉游戏窗口，或者点右下角的「结束」。</p>`;
    else if (r?.phase === 'crashed') body = `${chip('bad', '崩溃了')}</h3><p>原因在上面。</p>`;
    else if (r) body = `${chip('acc', '正在启动')}</h3><p>${esc(r.detail || '')}。进度在右下角的按钮里。</p>`;
    else if (!i.lastRun) body = `${chip('', '还没玩过')}</h3><p>按右下角的启动，或者直接按 <kbd>Enter</kbd>。第一次启动会先补上缺的文件，没有合适的 Java 也会自动下载。</p>`;
    else if (i.lastRun.ok) body = `${chip('', '正常退出')}</h3><p>${ago(i.lastRun.at)}${i.lastRun.min ? `，玩了 ${hours(i.lastRun.min)}` : ''}。</p>`;
    else body = `${chip('bad', '上次崩溃')}</h3><p>${ago(i.lastRun.at)}。日志里有原因。</p>`;
    const hasLog = r?.log?.length || i.lastLog?.length;
    return `<section class="card"><h3>${ico('violet', 'play')}运行${body}
        ${hasLog ? `<div class="actions"><button type="button" class="btn" data-act="tab" data-tab="logs">${icon('file')}看日志</button></div>` : ''}</section>`;
}

// 崩溃了：退出码、崩溃报告里的原因；按规则分析和一键修复在 M3
export function realCrashBanner(c) {
    const i = c.inst;
    const r = c.RUN.get(i.id);
    if (r?.phase !== 'crashed') return '';
    const k = r.crash || {};
    const d = k.diag;
    if (d) {
        return `<div class="banner" role="alert">${icon('alert', 20)}<div class="tx"><b>游戏崩溃了：${esc(d.title)}<span class="rule-tag">规则认出来的 · 不用 AI</span></b><p class="pre">${esc(d.detail)}</p></div>
        <div class="actions">${d.fix ? `<button type="button" class="btn pri" id="crashFix" data-act="crash-fix-real">${icon('wrench')}${esc(d.fixLabel)}</button>` : ''}${k.report ? `<button type="button" class="btn" data-act="crash-open">${icon('file')}崩溃报告</button>` : ''}<button type="button" class="btn" data-act="tab" data-tab="logs">${icon('list')}看日志</button><button type="button" class="btn ghost" data-act="crash-dismiss">先不管</button></div></div>`;
    }
    const why = k.desc ? `崩溃报告里写的是：${k.desc}${k.exc ? `（${k.exc}）` : ''}` : k.report ? '崩溃报告已经生成，可以打开看看。' : '没有生成崩溃报告，原因看日志的最后几行。';
    return `<div class="banner" role="alert">${icon('alert', 20)}<div class="tx"><b>游戏崩溃了${k.code != null ? `（退出码 ${k.code}）` : ''}</b><p>${esc(why)} 规则没认出原因；以后可以交给 AI 读日志。</p></div>
        <div class="actions">${k.report ? `<button type="button" class="btn pri" data-act="crash-open">${icon('file')}打开崩溃报告</button>` : ''}<button type="button" class="btn" data-act="tab" data-tab="logs">${icon('list')}看日志</button><button type="button" class="btn ghost" data-act="crash-dismiss">先不管</button></div></div>`;
}

// ---------- 账户 ----------
export function realAcctPop(c) {
    const list = c.R.accounts;
    return (list.length ? list.map((a) => `<button type="button" class="pop-item" data-act="acct-switch" data-id="${esc(a.id)}">${head('steve', 22)}<span class="tx"><span class="nm">${esc(a.name)}</span><span class="sb">${a.kind === 'msa' ? '微软正版' : '离线'}</span></span>${a.id === c.S.account ? icon('check', 14) : ''}</button>`).join('') + '<div class="pop-sep"></div>' : '<p class="pop-note">还没有账户。微软正版账户能进正版服务器；离线账户只能单人和进离线服务器。</p>')
        + `<button type="button" class="pop-item" data-act="acct-add-msa">${icon('plus')}<span class="tx"><span class="nm">添加微软账户</span><span class="sb">在手机或浏览器上输入一串代码</span></span></button>`
        + `<button type="button" class="pop-item" data-act="acct-add-off">${icon('plus')}<span class="tx"><span class="nm">添加离线账户</span><span class="sb">只起一个游戏名</span></span></button>`;
}

export function realMsaDrawer(c) {
    const d = c.S.drawer;
    let body;
    if (d.state === 'loading') body = '<p class="note">正在向微软要登录代码…</p>';
    else if (d.state === 'err') body = `<p class="callout warn">${icon('alert', 14)}<span>${esc(d.msg)}</span></p>`;
    else if (d.state === 'ok') body = `<p class="callout">${icon('check', 14)}<span>登录好了：<b>${esc(d.name)}</b>。以后启动默认用这个账户。</span></p>`;
    else body = `<p>在手机或电脑的浏览器上打开 <b class="mono">${esc(d.uri)}</b>，输入下面这串代码，再登录你买了 Minecraft 的微软账户。</p>
        <div class="msa-code mono" aria-label="登录代码">${esc(d.code)}</div>
        <p class="note">登录完不用回来点什么，这里会自动完成。代码 ${Math.max(1, Math.round((d.expires - Date.now()) / 60000))} 分钟内有效。令牌加密存在这台电脑上，不会给任何人。</p>`;
    const primary = d.state === 'wait'
        ? `${icon('copy', 22)}<span class="txt"><span class="lbl">复制代码并打开网页</span><span class="sub">${esc(d.code)}</span></span>`
        : d.state === 'err'
            ? `${icon('refresh', 22)}<span class="txt"><span class="lbl">重试</span><span class="sub">重新要一串代码</span></span>`
            : `${icon('check', 22)}<span class="txt"><span class="lbl">好了</span><span class="sub">关掉这个面板</span></span>`;
    return `<section class="drawer" role="dialog" aria-label="添加微软账户">
        <header class="dr-head"><h2>添加微软账户</h2><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
        <div class="dr-body">${body}</div>
        <footer class="dr-foot"><span class="hint">${d.state === 'wait' ? '正在等你在网页上登录' : ''}</span>
            <button type="button" class="go" id="drCreate" data-act="dr-create"${d.state === 'loading' ? ' disabled' : ''}>${primary}</button></footer>
    </section>`;
}

export function realOffDrawer(c) {
    const d = c.S.drawer;
    return `<section class="drawer" role="dialog" aria-label="添加离线账户">
        <header class="dr-head"><h2>添加离线账户</h2><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
        <div class="dr-body">
            ${d.blocked ? `<p class="callout warn">${icon('alert', 14)}<span>${esc(d.blocked)}</span></p>` : `<label class="field" for="offName"><span>游戏名</span><input id="offName" type="text" value="${esc(d.name || '')}" maxlength="16" autocomplete="off" placeholder="比如 Steve"></label>
            <small class="note">3 到 16 个字符，只能用英文字母、数字和下划线。离线账户能玩单人、局域网和离线模式的服务器，进不了正版服务器。</small>`}
            ${d.err ? `<p class="callout warn">${icon('alert', 14)}<span>${esc(d.err)}</span></p>` : ''}
        </div>
        <footer class="dr-foot"><span class="hint"></span>
            <button type="button" class="go" id="drCreate" data-act="dr-create"${d.blocked ? ' disabled' : ''}>${icon('plus', 22)}<span class="txt"><span class="lbl">添加</span><span class="sub">离线账户</span></span><kbd>Enter</kbd></button></footer>
    </section>`;
}

function accountsPanel(c) {
    const list = c.R.accounts;
    return `<section class="panel"><h2 class="ph">账户</h2>
        <div class="rows">${list.length ? list.map((a) => `<div class="row acct-row">
            ${head('steve', 30)}
            <span class="tx"><span class="nm">${esc(a.name)}</span><span class="sb">${a.kind === 'msa' ? '微软正版' : '离线'} · <span class="mono">${esc(a.uuid)}</span></span></span>
            ${a.id === c.S.account ? chip('ok', '默认账户') : `<button type="button" class="btn sm" data-act="acct-switch" data-id="${esc(a.id)}">设为默认</button>`}
            <button type="button" class="btn sm ghost danger" data-act="acct-del" data-id="${esc(a.id)}">移除</button>
        </div>`).join('') : '<div class="row empty">还没有账户</div>'}</div>
        <div class="actions"><button type="button" class="btn" data-act="acct-add-msa">${icon('plus')}微软账户</button><button type="button" class="btn" data-act="acct-add-off">${icon('plus')}离线账户</button></div>
    </section>`;
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
    ['Enter', '启动选中的实例'],
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

        ${launchPanel(c)}

        ${accountsPanel(c)}

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

// 启动：直接启动、游戏窗口出来以后、跳转列表（5.2）
function launchPanel(c) {
    const p = c.R.prefs;
    if (!p) return '';
    const last = p.recent[0];
    const recent = p.recent.slice(0, 5).map((r) => `「${esc(r.name)}」`).join('');
    return `<section class="panel"><h2 class="ph">启动</h2>
        <div class="kv"><div class="k"><b>直接启动</b><small>${last ? `双击 LeafStarter 图标就启动最近玩的「${esc(last.name)}」，主界面不出现；按住 Shift 再双击才打开界面` : '双击 LeafStarter 图标就启动最近玩的实例，主界面不出现。先启动过一次游戏才知道启动哪个'}</small></div>
            <div class="v"><input type="checkbox" class="switch" id="lpDirect"${p.directLaunch ? ' checked' : ''} aria-label="直接启动"></div></div>
        <div class="kv"><div class="k"><b>游戏窗口出来以后</b><small>启动器自己怎么办。选「隐藏」的话，游戏退出后启动器会回来</small></div>
            <div class="v">${seg('lp-after', [['keep', '不变'], ['min', '最小化'], ['hide', '隐藏']], p.afterLaunch)}</div></div>
        <div class="kv"><div class="k"><b>任务栏跳转列表</b><small>在任务栏的 LeafStarter 图标上点右键，能直接启动最近玩的 5 个实例${recent ? `：${recent}` : ''}。每个实例的概览里还能建桌面快捷方式</small></div><div class="v"></div></div>
    </section>`;
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

// ---------- 实例设置：存在版本文件夹里，启动时读 ----------
const MEM = [2048, 3072, 4096, 6144, 8192, 12288, 16384];
const WIN = [['', '默认（854 × 480）'], ['1280x720', '1280 × 720'], ['1600x900', '1600 × 900'], ['1920x1080', '1920 × 1080'], ['full', '全屏']];

export function realConfig(c) {
    const i = c.inst;
    const wait = realPending(c, '实例设置');
    if (wait) return wait;
    const g = i.cfgReal || {};
    const info = c.R.info || {};
    const total = info.totalMemoryMb || 0;
    const win = g.fullscreen ? 'full' : g.width ? `${g.width}x${g.height}` : '';
    const opt = (v, t, cur) => `<option value="${esc(v)}"${String(cur) === String(v) ? ' selected' : ''}>${esc(t)}</option>`;
    const row = (label, hint, ctl) => `<div class="kv"><div class="k"><b>${label}</b>${hint ? `<small>${hint}</small>` : ''}</div><div class="v">${ctl}</div></div>`;
    return `<p class="note">只影响这个实例，存在它自己的文件夹里（versions/${esc(i.name)}/LeafStarter/instance.json），换启动器、拷到别的电脑也带着走。</p>
    <section class="panel">
        ${row('内存', `本机 ${Math.round(total / 1024)} GB。自动是本机的四分之一，最少 2 GB、最多 6 GB`, `<select data-rcfg="memory">${opt('', `自动（${Math.round((info.autoMemoryMb || 4096) / 1024 * 10) / 10} GB）`, g.memoryMb || '')}${MEM.filter((m) => !total || m <= total * 0.8).map((m) => opt(m, `${m / 1024} GB`, g.memoryMb || '')).join('')}</select>`)}
        ${row('Java', `自动：这个版本要 Java ${i.javaNeed}，按版本 JSON 的要求挑，没有就下载官方运行时`, `<select data-rcfg="java">${opt('', '自动', g.javaPath || '')}${c.JAVAS.map((j) => opt(j.path, j.label, g.javaPath || '')).join('')}${g.javaPath && !c.JAVAS.some((j) => j.path === g.javaPath) ? opt(g.javaPath, g.javaPath, g.javaPath) : ''}</select>`)}
        ${row('窗口大小', '', `<select data-rcfg="window">${WIN.map(([v, t]) => opt(v, t, win)).join('')}</select>`)}
        ${row('JVM 参数', '一般不用填。只能在这里自己填，整合包和同步都带不进来', `<input type="text" class="wide mono" data-rcfg="jvm" value="${esc(g.jvmArgs || '')}" placeholder="比如 -XX:+UseZGC" autocomplete="off">`)}
    </section>`;
}
