// 实例页：头部、标签、各标签的内容。这里只出 HTML，事件统一在 app.js 里接
import { esc, ago, hours, clock, block } from '../ui.js';
import { icon } from '../icons.js';

export const TABS = [['overview', '概览'], ['mods', 'Mod'], ['worlds', '存档'], ['shots', '截图'], ['servers', '服务器'], ['logs', '日志'], ['config', '设置']];

const chip = (cls, ic, text) => `<span class="chip ${cls}">${ic ? icon(ic, 13) : ''}${esc(text)}</span>`;
const seg = (act, list, cur) => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="${act}" data-v="${v}" class="${cur === v ? 'on' : ''}" aria-pressed="${cur === v}">${t}</button>`).join('')}</div>`;

export function instanceView(c) {
    const i = c.inst;
    if (!i) {
        return `<div class="page"><section class="card"><h3>还没有实例</h3><p>点左上角的 ＋，或按 <kbd>Ctrl K</kbd> 输入「新建」。</p></section></div>`;
    }
    return `<header class="ihead">
        ${block(i.block, 44)}
        <div class="ititle"><h1>${esc(i.name)}</h1><div class="meta">${meta(i, c)}</div></div>
        <div class="chips" id="hstat">${statusChips(i, c)}</div>
    </header>
    <nav class="tabs" id="tabs" role="tablist" aria-label="实例内容">${tabsHTML(c)}</nav>
    <div class="body" id="body">${tabBody(c)}</div>`;
}

export function tabsHTML(c) {
    const i = c.inst;
    const count = { mods: i.loader ? i.mods.length : '', worlds: i.worlds.length, shots: i.shots.length, servers: i.servers.length };
    return TABS.map(([k, t]) => {
        const on = c.S.tab === k;
        const n = count[k] ?? '';
        return `<button type="button" role="tab" data-act="tab" data-tab="${k}" class="${on ? 'on' : ''}" aria-selected="${on}">${t}${n !== '' ? `<span class="n">${n}</span>` : ''}</button>`;
    }).join('');
}

function meta(i, c) {
    const dir = c.DIRS.find((d) => d.id === i.dir);
    return [
        `<span class="mono">${esc(i.mc)} · ${i.loader ? `${esc(i.loader)} ${esc(i.loaderVer)}` : '原版'}</span>`,
        `<span>${i.lastPlayed ? '上次玩 ' + ago(i.lastPlayed) : '还没玩过'}</span>`,
        i.played ? `<span>共 ${hours(i.played)}</span>` : '',
        i.dir !== 'own' ? `<span title="${esc(dir.path)}">在 ${esc(dir.name)} 的目录里</span>` : '',
    ].join('');
}

export function statusChips(i, c) {
    const r = c.RUN.get(i.id);
    if (i.installing) return chip('acc', 'download', `安装中 ${i.installing.pct}%`);
    if (r?.phase === 'running') return chip('ok', 'play', '运行中');
    if (r?.phase === 'crashed') return chip('bad', 'alert', '崩溃了');
    if (r) return chip('acc', 'zap', '正在启动');
    const out = [];
    if (i.sync.on) out.push(i.sync.state === 'up' ? chip('acc', 'cloud', '上传中') : chip('ok', 'smartphone', '手机电脑已同步'));
    if (i.perf) out.push(chip('', 'zap', '流畅优先'));
    return out.join('');
}

export function tabBody(c) {
    switch (c.S.tab) {
        case 'mods': return mods(c);
        case 'worlds': return worlds(c);
        case 'shots': return shots(c);
        case 'servers': return servers(c);
        case 'logs': return logs(c);
        case 'config': return config(c);
        default: return overview(c);
    }
}

// ---------- 概览 ----------
function overview(c) {
    const i = c.inst;
    const r = c.RUN.get(i.id);
    const banner = r?.phase === 'crashed' && i.diag
        ? `<div class="banner" role="alert">${icon('alert', 20)}<div class="tx"><b>${esc(i.diag.title)}</b><p>${esc(i.diag.detail)}</p></div>
            <div class="actions"><button type="button" class="btn pri" id="crashFix" data-act="crash-fix">${icon('wrench')}${esc(i.diag.fix)}</button><button type="button" class="btn ghost" data-act="crash-dismiss">先不管</button></div></div>`
        : '';
    return `${banner}
    <div class="actions">
        <button type="button" class="btn" data-act="folder" data-v="">${icon('folder')}打开文件夹</button>
        <button type="button" class="btn" data-act="verify">${icon('refresh')}补全文件</button>
        <button type="button" class="btn" data-act="export">${icon('package')}导出整合包</button>
        <button type="button" class="btn" data-act="copy">${icon('copy')}复制一份</button>
        <button type="button" class="btn ghost danger" data-act="del-inst">${icon('trash')}删除</button>
    </div>
    <div class="grid2">${syncCard(i, r)}${perfCard(i)}${runCard(i, r)}${recentWorlds(i)}</div>`;
}

function syncCard(i, r) {
    const s = i.sync;
    if (!s.on) {
        return `<section class="card">
            <h3>${icon('smartphone')}手机电脑同步${chip('', '', '没开')}</h3>
            <p>打开后，这个实例的 Mod、配置和存档会同步到你手机上的启动器。在学校用手机接着玩，回家再用电脑接着玩。</p>
            <div class="actions"><button type="button" class="btn pri" data-act="sync-on">${icon('cloud')}开启同步</button></div>
        </section>`;
    }
    const state = s.state === 'up' ? chip('acc', 'cloud', '上传中') : chip('ok', 'check', '已同步');
    const first = i.worlds[0];
    return `<section class="card">
        <h3>${icon('smartphone')}手机电脑同步${state}<input type="checkbox" class="switch push" id="syncSwitch" checked aria-label="手机电脑同步"></h3>
        <dl class="facts">
            <dt>最近一次</dt><dd>${s.state === 'up' ? '正在上传刚才的改动' : `${ago(s.at)} · 电脑上传`}</dd>
            <dt>手机</dt><dd>${esc(s.device)}</dd>
            <dt>谁在玩</dt><dd>${r?.phase === 'running' ? '这台电脑（手机上打开会提示）' : '没人在玩'}</dd>
        </dl>
        ${first ? `<div class="actions"><button type="button" class="btn" data-act="sync-hist" data-v="${first.id}">${icon('history')}「${esc(first.name)}」的历史版本</button></div>` : ''}
    </section>`;
}

function perfCard(i) {
    if (!i.loader) {
        return `<section class="card">
            <h3>${icon('zap')}流畅优先${chip('', '', '原版')}</h3>
            <p>原版装不了优化 Mod。想更流畅，新建一个 Fabric 实例并勾上「流畅优先」。</p>
        </section>`;
    }
    const list = i.mods.filter((m) => m.perf);
    if (i.perf) {
        return `<section class="card">
            <h3>${icon('zap')}流畅优先${chip('ok', 'check', '已开启')}</h3>
            <p>加了 ${list.length} 个优化 Mod：${esc(list.slice(0, 3).map((m) => m.name).join('、'))} 等。不想要可以一键撤掉，只删这几个。</p>
            <div class="actions"><button type="button" class="btn" data-act="perf-off">撤掉这 ${list.length} 个</button></div>
        </section>`;
    }
    return `<section class="card">
        <h3>${icon('zap')}流畅优先${chip('', '', '没开')}</h3>
        <p>一键加上一组口碑好的优化 Mod，按 ${esc(i.loader)} 和 ${esc(i.mc)} 挑版本，随时能撤掉。</p>
        <div class="actions"><button type="button" class="btn pri" data-act="perf-on" id="perfOn">${icon('zap')}加上</button></div>
    </section>`;
}

function runCard(i, r) {
    let body;
    if (i.installing) {
        body = `${chip('acc', 'download', '正在安装')}</h3><p>装好就能玩，进度在右下角的按钮里。</p>`;
    } else if (r?.phase === 'running') {
        body = `${chip('ok', 'play', '运行中')}</h3><p>已经玩了 <b class="mono" id="runClock">${clock(Math.floor((Date.now() - r.started) / 1000))}</b>。${i.sync.on ? '结束游戏后会自动上传存档。' : '这个实例没开手机电脑同步。'}</p>`;
    } else if (r?.phase === 'crashed') {
        body = `${chip('bad', 'alert', '崩溃了')}</h3><p>原因和修复在上面。</p>`;
    } else if (r) {
        body = `${chip('acc', 'zap', '正在启动')}</h3><p>进度在右下角的按钮里。</p>`;
    } else if (!i.lastRun) {
        body = `${chip('', '', '还没玩过')}</h3><p>按右下角的启动，或者直接按 <kbd>Enter</kbd>。</p>`;
    } else if (i.lastRun.ok) {
        body = `${chip('', 'check', '正常退出')}</h3><p>${ago(i.lastRun.at)}，玩了 ${hours(i.lastRun.min)}。</p>`;
    } else {
        body = `${chip('bad', 'alert', '上次崩溃')}</h3><p>${ago(i.lastRun.at)}，启动时就崩了。日志里有原因。</p>`;
    }
    return `<section class="card"><h3>${icon('clock')}上次运行${body}
        <div class="actions"><button type="button" class="btn" data-act="tab" data-tab="logs">${icon('file')}看日志</button></div></section>`;
}

function recentWorlds(i) {
    const list = [...i.worlds].sort((a, b) => b.last - a.last).slice(0, 2);
    return `<section class="card">
        <h3>${icon('globe')}最近的存档</h3>
        ${list.length
            ? `<div class="mini-rows">${list.map((w) => `<div class="mrow"><span class="tx"><span class="nm">${esc(w.name)}</span><span class="sb">${esc(w.mode)} · ${ago(w.last)}</span></span><button type="button" class="btn sm" data-act="world-play" data-v="${esc(w.name)}">${icon('play', 13)}进入</button></div>`).join('')}</div>`
            : '<p>还没有存档。进游戏建一个，就会出现在这里。</p>'}
    </section>`;
}

// ---------- Mod ----------
function mods(c) {
    const i = c.inst;
    if (!i.loader) {
        return `<section class="card"><h3>原版装不了 Mod</h3><p>想装 Mod，就新建一个带 Fabric、Forge 或 NeoForge 的实例。</p>
            <div class="actions"><button type="button" class="btn pri" data-act="new-inst">${icon('plus')}新建实例</button></div></section>`;
    }
    const upd = i.mods.filter((m) => m.update).length;
    return `<div class="toolbar">
        <label class="filter grow" for="modQ">${icon('search', 14)}<input id="modQ" type="text" placeholder="筛选 Mod" value="${esc(c.S.modQ)}" autocomplete="off"></label>
        ${seg('mods-f', [['all', '全部'], ['on', '已启用'], ['off', '已禁用'], ['upd', '有更新']], c.S.modFilter)}
        <span class="sp"></span>
        ${upd ? `<button type="button" class="btn" data-act="mods-up" id="modsUp">${icon('arrowUp')}全部更新（${upd}）</button>` : ''}
        <button type="button" class="btn" data-act="add-mod">${icon('plus')}添加 Mod</button>
    </div>
    <p class="note" id="modCount">${modCount(i)}</p>
    <div class="rows" id="modRows">${modRows(c)}</div>`;
}

export function modCount(i) {
    const on = i.mods.filter((m) => m.on).length;
    return `启用 ${on} 个，共 ${i.mods.length} 个。开关立刻生效，下次启动就是新的组合。`;
}

export function modRows(c) {
    const i = c.inst;
    const q = c.S.modQ.trim().toLowerCase();
    const f = c.S.modFilter;
    const list = i.mods.filter((m) => (!q || m.name.toLowerCase().includes(q)) && (f === 'all' || (f === 'on' && m.on) || (f === 'off' && !m.on) || (f === 'upd' && m.update)));
    if (!list.length) return '<div class="row empty">没有符合条件的 Mod</div>';
    return list.map((m) => `<div class="row mod${m.on ? '' : ' off'}">
        <input type="checkbox" class="switch" data-mod="${m.id}" ${m.on ? 'checked' : ''} aria-label="启用 ${esc(m.name)}">
        <span class="tile" style="--h:${m.h}">${esc(m.name[0])}</span>
        <span class="tx"><span class="nm">${esc(m.name)}</span><span class="sb"><span class="mono">${esc(m.ver)}</span> · ${m.src}${m.note ? ' · ' + esc(m.note) : ''}</span></span>
        <span class="tags">${m.perf ? '<span class="chip">流畅优先</span>' : ''}${m.update ? `<button type="button" class="btn sm" data-act="mod-up" data-id="${m.id}">${icon('arrowUp', 13)}更新到 ${esc(m.update)}</button>` : ''}</span>
        <button type="button" class="iconbtn" data-act="mod-del" data-id="${m.id}" aria-label="删除 ${esc(m.name)}" title="删除">${icon('trash', 15)}</button>
    </div>`).join('');
}

// ---------- 存档 ----------
function worlds(c) {
    const i = c.inst;
    if (!i.worlds.length) return '<section class="card"><h3>还没有存档</h3><p>进游戏建一个世界，就会出现在这里。</p></section>';
    return `<p class="note">点「进入」会启动游戏并直接进这个世界，不用在游戏里再点一遍。${i.sync.on ? '开了同步，每次退出游戏后自动上传，「历史」里能退回任意一次。' : ''}</p>
    <div class="rows">${i.worlds.map((w) => `<div class="row world">
        ${block('grass', 32)}
        <span class="tx"><span class="nm">${esc(w.name)}</span><span class="sb">${esc(w.mode)} · ${esc(w.diff)} · ${ago(w.last)}</span></span>
        <span class="mono sb">${esc(w.size)}</span>
        <span class="tags">
            <button type="button" class="btn sm" data-act="world-play" data-v="${esc(w.name)}">${icon('play', 13)}进入</button>
            <button type="button" class="btn sm" data-act="backup" data-v="${esc(w.name)}">备份</button>
            ${i.sync.on && w.history.length ? `<button type="button" class="btn sm" data-act="sync-hist" data-v="${w.id}">${icon('history', 13)}历史</button>` : ''}
        </span>
    </div>`).join('')}</div>`;
}

// ---------- 截图 ----------
function shots(c) {
    const i = c.inst;
    if (!i.shots.length) return '<section class="card"><h3>还没有截图</h3><p>游戏里按 F2 截图，会出现在这里。</p></section>';
    return `<div class="shots">${i.shots.map((s) => `<figure class="shot" style="--a:${s.seed};--b:${(s.seed + 80) % 360}">
        <span class="sky"></span><span class="land"></span>
        <figcaption><span class="mono">${ago(s.at)}</span>
            <span class="shot-act"><button type="button" class="iconbtn sm" data-act="shot-copy" aria-label="复制截图" title="复制">${icon('copy', 14)}</button><button type="button" class="iconbtn sm" data-act="shot-del" data-id="${s.id}" aria-label="删除截图" title="删除">${icon('trash', 14)}</button></span>
        </figcaption>
    </figure>`).join('')}</div>`;
}

// ---------- 服务器 ----------
function servers(c) {
    const i = c.inst;
    if (!i.servers.length) return '<section class="card"><h3>还没有服务器</h3><p>在游戏的「多人游戏」里加过的服务器会出现在这里。</p></section>';
    return `<div class="toolbar"><p class="note">点「进入」会启动游戏并直接连上服务器。</p><span class="sp"></span><button type="button" class="btn" data-act="servers-ping">${icon('refresh')}刷新</button></div>
    <div class="rows" id="srvRows">${serverRows(c)}</div>`;
}

export function serverRows(c) {
    return c.inst.servers.map((s) => {
        const lvl = s.ping < 60 ? 'ok' : s.ping < 150 ? 'warn' : 'bad';
        return `<div class="row server">
            <span class="tile" style="--h:${(s.name.length * 53) % 360}">${icon('server', 16)}</span>
            <span class="tx"><span class="nm">${esc(s.name)} <span class="mono sb">${esc(s.addr)}</span></span><span class="sb">${esc(s.motd)}</span></span>
            <span class="sb mono">${esc(s.players)}</span>
            <span class="ping ${lvl}"><i></i><i></i><i></i><span class="mono">${s.ping} ms</span></span>
            <button type="button" class="btn sm" data-act="server-play" data-v="${esc(s.addr)}">${icon('play', 13)}进入</button>
        </div>`;
    }).join('');
}

// ---------- 日志 ----------
function logs(c) {
    const i = c.inst;
    const r = c.RUN.get(i.id);
    const live = r && r.phase !== 'crashed';
    const label = live ? '实时输出' : r?.phase === 'crashed' ? '崩溃前的输出' : i.lastRun ? '上次运行 · ' + ago(i.lastRun.at) : '还没有日志';
    return `<div class="toolbar">
        ${chip(live ? 'ok' : '', live ? 'play' : 'file', label)}
        ${seg('log-f', [['all', '全部'], ['warn', '警告'], ['error', '错误']], c.S.logLevel)}
        <span class="sp"></span>
        <button type="button" class="btn" data-act="log-copy">${icon('copy')}复制</button>
        <button type="button" class="btn" data-act="folder" data-v="logs">${icon('folder')}日志文件夹</button>
    </div>
    <pre class="log" id="logBox" tabindex="0">${logText(c)}</pre>`;
}

const LEVEL = { warn: /\/(WARN|ERROR)\]/, error: /\/ERROR\]/ };
export function logText(c) {
    const i = c.inst;
    const r = c.RUN.get(i.id);
    const lines = (r ? r.log : i.lastLog) || [];
    const f = c.S.logLevel;
    const shown = lines.filter((l) => f === 'all' || LEVEL[f].test(l));
    if (!shown.length) return `<span class="t">${f === 'all' ? '（还没有输出）' : f === 'warn' ? '（没有警告）' : '（没有错误）'}</span>`;
    return shown.map(logLine).join('\n');
}
export function logLine(l) {
    const cls = /\/ERROR\]/.test(l) ? 'e' : /\/WARN\]/.test(l) ? 'w' : '';
    const m = l.match(/^(\[[\d:]+\]) (.*)$/);
    return m ? `<span class="t">${m[1]}</span> <span class="${cls}">${esc(m[2])}</span>` : esc(l);
}

// ---------- 实例设置 ----------
function cfgRows(c, i) {
    return [
        { key: 'java', label: 'Java', hint: '自动会按这个版本的要求挑', opts: [['auto', `自动（Java ${i.javaNeed}）`], ...c.JAVAS.map((j) => [j.id, j.label])] },
        { key: 'mem', label: '内存', hint: '本机 16 GB，自动给 4 GB', opts: [['auto', '自动（4 GB）'], ['2', '2 GB'], ['4', '4 GB'], ['6', '6 GB'], ['8', '8 GB']] },
        { key: 'win', label: '窗口大小', opts: [['854x480', '854 × 480'], ['1280x720', '1280 × 720'], ['1920x1080', '1920 × 1080'], ['full', '全屏']] },
        { key: 'isolate', label: '版本隔离', hint: '存档、Mod 和配置放在这个实例自己的文件夹里。默认：装了加载器就隔离', type: 'switch' },
        { key: 'account', label: '用哪个账户', opts: [['default', '跟默认账户'], ...c.ACCOUNTS.map((a) => [a.id, `${a.name}（${a.label}）`])] },
        { key: 'join', label: '进游戏后自动连服务器', opts: [['none', '不自动连'], ...i.servers.map((s) => [s.addr, s.name])] },
        { key: 'jvm', label: 'JVM 参数', type: 'text', ph: '一般不用填，比如 -XX:+UseZGC' },
        { key: 'pre', label: '启动前先运行', type: 'text', ph: '启动游戏前执行的命令' },
    ];
}

function config(c) {
    const i = c.inst;
    const rows = cfgRows(c, i).map((row) => {
        const ovr = row.key in i.cfg;
        const val = row.key === 'isolate' ? c.isolate(i) : ovr ? i.cfg[row.key] : c.GLOBAL[row.key];
        let ctl;
        if (row.type === 'switch') {
            ctl = `<input type="checkbox" class="switch" id="cfg-${row.key}" data-cfg="${row.key}" ${val ? 'checked' : ''} aria-label="${row.label}">`;
        } else if (row.type === 'text') {
            ctl = `<input type="text" class="mono wide" id="cfg-${row.key}" data-cfg="${row.key}" value="${esc(val)}" placeholder="${esc(row.ph)}" autocomplete="off">`;
        } else {
            ctl = `<select id="cfg-${row.key}" data-cfg="${row.key}">${row.opts.map(([v, t]) => `<option value="${esc(v)}"${String(val) === String(v) ? ' selected' : ''}>${esc(t)}</option>`).join('')}</select>`;
        }
        return `<div class="kv">
            <div class="k"><b>${row.label}</b>${row.hint ? `<small>${row.hint}</small>` : ''}</div>
            <div class="v"><span class="inh${ovr ? ' ovr' : ''}">${ovr ? '这个实例单独设了' : '跟全局一样'}</span>${ovr ? `<button type="button" class="btn ghost sm" data-act="cfg-reset" data-k="${row.key}">恢复</button>` : ''}${ctl}</div>
        </div>`;
    }).join('');
    return `<p class="note">没改过的项跟着全局设置走；在这里改了，只影响这个实例。</p><section class="panel">${rows}</section>`;
}
