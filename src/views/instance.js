// 实例页：头部、标签、各标签的内容。这里只出 HTML，事件统一在 app.js 里接
import { esc, ago, hours, clock, block } from '../ui.js';
import { icon } from '../icons.js';
import { mpView } from './multiplayer.js';
import { crashBanner } from './fix.js';
import { worldList, worldDetail } from './world.js';
import { gpuCard, zhCard, zhTag, zhSub } from './tools.js';
import { realOverview, realWorlds, realPending, realEmpty, realConfig, modUpdates, zhPanel, zhChip } from './real.js';
import { realServers } from './server-real.js';
import { editionHome } from '../edition-real.js';

export const TABS = [['overview', '概览'], ['mods', 'Mod'], ['worlds', '存档'], ['shots', '截图'], ['servers', '联机'], ['logs', '日志'], ['config', '设置']];

const chip = (cls, ic, text) => `<span class="chip ${cls}">${ic ? icon(ic, 13) : ''}${esc(text)}</span>`;
const seg = (act, list, cur) => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="${act}" data-v="${v}" class="${cur === v ? 'on' : ''}" aria-pressed="${cur === v}">${t}</button>`).join('')}</div>`;

export function instanceView(c) {
    const i = c.inst;
    if (!i && c.real) return realEmpty(c);
    if (!i) {
        return `<div class="page"><section class="card"><h3>还没有实例</h3><p>点左上角的 ＋，或按 <kbd>Ctrl K</kbd> 输入「新建」。</p></section></div>`;
    }
    // 头图：每个实例一幅自己的像素风景，云在上面慢慢飘
    return `<header class="hero" id="hero">
        <canvas class="scene" id="heroScene" aria-hidden="true"></canvas>
        <div class="clouds" aria-hidden="true"><canvas id="heroClouds"></canvas></div>
        <div class="hero-in">
            <div class="hero-title">${block(i.block, 40)}<div class="ititle"><h1>${esc(i.name)}</h1><div class="meta">${meta(i, c)}</div></div></div>
            <div class="chips" id="hstat">${statusChips(i, c)}</div>
        </div>
    </header>
    ${minimal(c) ? '' : `<nav class="tabs" id="tabs" role="tablist" aria-label="实例内容">${tabsHTML(c)}<i class="ind" id="tabInd" aria-hidden="true"></i></nav>`}
    <div class="body" id="body">${tabBody(c)}</div>`;
}

const ico = (tone, name) => `<span class="ico ${tone}">${icon(name, 16)}</span>`;

// 简洁界面：藏起截图，和没装的功能包的入口
// 真程序里先只有概览、Mod、存档：截图、联机、日志、实例设置还没有后端
const REAL_TABS = ['overview', 'mods', 'worlds', 'servers', 'logs', 'config'];
export const visibleTabs = (c) => (c.real ? TABS.filter(([k]) => REAL_TABS.includes(k) && !(k === 'servers' && c.plugOff('mp') && c.plugOff('server'))) : TABS.filter(([k]) => !(c.S.simple && (k === 'shots' || (k === 'servers' && !c.S.ext.mp))) && !(k === 'servers' && c.plugOff('mp'))));
const minimal = (c) => c.S.distOn && c.S.dist.minimal;

export function tabsHTML(c) {
    const i = c.inst;
    const count = { mods: i.loader ? i.mods.length : '', worlds: i.worlds.length, shots: i.shots.length, servers: c.real ? (c.R.srv?.list.filter((x) => x.inst === i.id).length || '') : i.servers.length };
    // 「联机」标签：房间开着亮绿点，正在开服亮紫点
    const live = i.room?.phase === 'open' ? '<i class="dot run tabdot" title="房间开着"></i>' : i.srv?.phase === 'setup' ? '<i class="dot busy tabdot" title="正在开服"></i>' : '';
    return visibleTabs(c).map(([k, t]) => {
        const on = c.S.tab === k;
        const n = count[k] ?? '';
        const tail = k === 'servers' && live ? live : n !== '' ? `<span class="n">${n}</span>` : '';
        return `<button type="button" role="tab" data-act="tab" data-tab="${k}" class="${on ? 'on' : ''}" aria-selected="${on}">${t}${tail}</button>`;
    }).join('');
}

function meta(i, c) {
    const dir = c.DIRS.find((d) => d.id === i.dir);
    return [
        `<span class="mono">${esc(i.mc)} · ${i.loader ? `${esc(i.loader)} ${esc(i.loaderVer)}` : '原版'}</span>`,
        c.real ? '' : `<span>${i.lastPlayed ? '上次玩 ' + ago(i.lastPlayed) : '还没玩过'}</span>`,
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
    if (minimal(c)) return c.real ? editionHome(c) : distroCards(c, c.inst);
    const tab = visibleTabs(c).some(([k]) => k === c.S.tab) ? c.S.tab : 'overview';
    if (c.real && tab === 'overview') return realOverview(c);
    if (c.real && tab === 'worlds') return realWorlds(c);
    if (c.real && tab === 'config') return realConfig(c);
    if (c.real && tab === 'servers') return realServers(c);
    switch (tab) {
        case 'mods': return mods(c);
        case 'worlds': return worlds(c);
        case 'shots': return shots(c);
        case 'servers': return c.S.ext.mp ? mpView(c) : mpGate(c);
        case 'logs': return logs(c);
        case 'config': return config(c);
        default: return overview(c);
    }
}

// ---------- 概览 ----------
function overview(c) {
    const i = c.inst;
    const r = c.RUN.get(i.id);
    return `${crashBanner(c)}${i.distro ? distroCards(c, i) : ''}
    <div class="actions">
        <button type="button" class="btn" data-act="folder" data-v="">${icon('folder')}打开文件夹</button>
        <button type="button" class="btn" data-act="verify">${icon('refresh')}补全文件</button>
        <button type="button" class="btn" data-act="export">${icon('package')}导出整合包</button>
        <button type="button" class="btn" data-act="copy">${icon('copy')}复制一份</button>
        <button type="button" class="btn ghost danger" data-act="del-inst">${icon('trash')}删除</button>
    </div>
    <div class="grid2">${syncCard(i, r)}${perfCard(i)}${gpuCard(i, c)}${c.plugOff('zh') ? '' : zhCard(i)}${runCard(i, r)}${recentWorlds(i)}${c.plugOn('srvcard') && i.servers.length ? srvCard(i) : ''}</div>`;
}

// 定制版：服主的公告和服务器，放在最上面
function distroCards(c, i) {
    const D = c.S.dist;
    const s = i.servers[0];
    return `${D.notice ? `<section class="card notice"><h3>${ico('sky', 'sparkles')}公告<span class="chip xs">来自定制包 · ${esc(D.author)}</span></h3><p>${esc(D.notice)}</p></section>` : ''}
    ${s ? `<section class="card"><h3>${ico('violet', 'server')}${esc(s.name)}<span class="chip ok">${esc(s.players)} 在线</span></h3>
        <div class="mini-rows"><div class="mrow"><span class="tx"><span class="nm mono">${esc(s.addr)}</span><span class="sb">进游戏后自动连上这个服务器 · 延迟 ${s.ping} ms</span></span><button type="button" class="btn sm pri" data-act="server-play" data-v="${esc(s.addr)}">${icon('play', 13)}进服</button></div></div></section>` : ''}`;
}

// 插件「服务器状态卡」画的卡片（正式版里它跑在隔离环境里，只能拿到服务器地址）
function srvCard(i) {
    return `<section class="card plug-card"><h3>${ico('sky', 'plug')}服务器状态<span class="chip xs">插件 · 服务器状态卡</span></h3>
        <div class="mini-rows">${i.servers.map((s) => `<div class="mrow"><span class="tx"><span class="nm">${esc(s.name)}</span><span class="sb">${esc(s.players)} 在线 · ${s.ping} ms · 今天上过线：小明、阿哲</span></span></div>`).join('')}</div>
        <p class="note">这张卡片是第三方插件画的，跑在隔离环境里：它只拿得到服务器地址，拿不到你的账户。</p></section>`;
}

// 联机功能包没装：入口还在，写清楚要装什么
function mpGate(c) {
    const busy = c.S.extBusy === 'mp';
    return `<section class="card gate">
        <h3>${ico('violet', 'package')}联机是一个功能包</h3>
        <p>里面是 P2P 组网程序 EasyTier（约 14 MB）。它会在后台跑、会开端口，所以单独成包：不用联机的人不装，它也不会被杀毒软件误报连累主程序。官方签名，装完立刻能用。</p>
        <div class="actions">${busy ? `<span class="bar w live" data-extp="mp"><i style="--p:${c.S.extPct || 0}%"></i></span><span class="mono sb" data-extt="mp">${c.S.extStage || ''} ${c.S.extPct || 0}%</span>` : `<button type="button" class="btn pri" data-act="ext-add" data-v="mp">${icon('download')}装上联机（约 14 MB）</button>`}</div>
    </section>`;
}

function syncCard(i, r) {
    const s = i.sync;
    if (!s.on) {
        return `<section class="card">
            <h3>${ico('ok', 'smartphone')}手机电脑同步${chip('', '', '没开')}</h3>
            <p>打开后，这个实例的 Mod、配置和存档会同步到你手机上的启动器。在学校用手机接着玩，回家再用电脑接着玩。</p>
            <div class="actions"><button type="button" class="btn pri" data-act="sync-on">${icon('cloud')}开启同步</button></div>
        </section>`;
    }
    const state = s.state === 'up' ? chip('acc', 'cloud', '上传中') : chip('ok', 'check', '已同步');
    const first = i.worlds[0];
    return `<section class="card">
        <h3>${ico('ok', 'smartphone')}手机电脑同步${state}<input type="checkbox" class="switch push" id="syncSwitch" checked aria-label="手机电脑同步"></h3>
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
            <h3>${ico('amber', 'zap')}流畅优先${chip('', '', '原版')}</h3>
            <p>原版装不了优化 Mod。想更流畅，新建一个 Fabric 实例并勾上「流畅优先」。</p>
        </section>`;
    }
    const list = i.mods.filter((m) => m.perf);
    if (i.perf) {
        return `<section class="card">
            <h3>${ico('amber', 'zap')}流畅优先${chip('ok', 'check', '已开启')}</h3>
            <p>加了 ${list.length} 个优化 Mod：${esc(list.slice(0, 3).map((m) => m.name).join('、'))} 等。不想要可以一键撤掉，只删这几个。</p>
            <div class="actions"><button type="button" class="btn" data-act="perf-off">撤掉这 ${list.length} 个</button></div>
        </section>`;
    }
    return `<section class="card">
        <h3>${ico('amber', 'zap')}流畅优先${chip('', '', '没开')}</h3>
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
    return `<section class="card"><h3>${ico('violet', 'clock')}上次运行${body}
        <div class="actions"><button type="button" class="btn" data-act="tab" data-tab="logs">${icon('file')}看日志</button></div></section>`;
}

function recentWorlds(i) {
    const list = [...i.worlds].sort((a, b) => b.last - a.last).slice(0, 2);
    return `<section class="card">
        <h3>${ico('sky', 'globe')}最近的存档</h3>
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
    const wait = c.real ? realPending(c, 'Mod') : '';
    if (wait) return wait;
    if (c.real) {
        return `<div class="toolbar">
        <label class="filter grow" for="modQ">${icon('search', 14)}<input id="modQ" type="text" placeholder="筛选 Mod" value="${esc(c.S.modQ)}" autocomplete="off"></label>
        ${seg('mods-f', [['all', '全部'], ['on', '已启用'], ['off', '已禁用']], c.S.modFilter)}
        <span class="sp"></span>
        <button type="button" class="btn" data-act="mod-check">${icon('refresh')}检查更新</button>
        <button type="button" class="btn" data-act="zh-real">${icon('globe')}中文</button>
        <button type="button" class="btn" data-act="res-go">${icon('plus')}添加 Mod</button>
        <button type="button" class="btn" data-act="folder" data-v="mods">${icon('folder')}mods 文件夹</button>
    </div>
    ${modUpdates(c)}
    ${zhPanel(c)}
    <p class="note" id="modCount">${modCount(i, c)}</p>
    <div class="rows" id="modRows">${modRows(c)}</div>`;
    }
    const upd = i.mods.filter((m) => m.update).length;
    return `<div class="toolbar">
        <label class="filter grow" for="modQ">${icon('search', 14)}<input id="modQ" type="text" placeholder="筛选 Mod" value="${esc(c.S.modQ)}" autocomplete="off"></label>
        ${seg('mods-f', [['all', '全部'], ['on', '已启用'], ['off', '已禁用'], ['upd', '有更新']], c.S.modFilter)}
        <span class="sp"></span>
        ${upd ? `<button type="button" class="btn" data-act="mods-up" id="modsUp">${icon('arrowUp')}全部更新（${upd}）</button>` : ''}
        ${!c.plugOff('zh') && i.mods.some((m) => zhTag(m, i).includes('没中文')) ? `<button type="button" class="btn" data-act="zh-open">${icon('globe')}补全中文</button>` : ''}
        <button type="button" class="btn" data-act="add-mod">${icon('plus')}添加 Mod</button>
    </div>
    <p class="note" id="modCount">${modCount(i)}</p>
    <div class="rows" id="modRows">${modRows(c)}</div>`;
}

export function modCount(i, c) {
    const on = i.mods.filter((m) => m.on).length;
    if (i.real) return `启用 ${on} / ${i.mods.length}`;
    return `启用 ${on} 个，共 ${i.mods.length} 个。开关立刻生效，下次启动就是新的组合。`;
}

export function modRows(c) {
    const i = c.inst;
    const q = c.S.modQ.trim().toLowerCase();
    const f = c.S.modFilter;
    const list = i.mods.filter((m) => (!q || m.name.toLowerCase().includes(q)) && (f === 'all' || (f === 'on' && m.on) || (f === 'off' && !m.on) || (f === 'upd' && m.update)));
    if (!list.length) return `<div class="row empty">${i.mods.length ? '没有符合条件的 Mod' : 'mods 文件夹里还没有 Mod'}</div>`;
    if (i.real) {
        return list.map((m) => `<div class="row mod${m.on ? '' : ' off'}" data-row="${m.id}">
        <input type="checkbox" class="switch" data-mod="${m.id}" ${m.on ? 'checked' : ''} aria-label="启用 ${esc(m.name)}">
        ${m.icon ? `<img class="tile mic" src="${m.icon}" alt="" draggable="false">` : `<span class="tile" style="--h:${m.h}">${esc(m.name[0] || '?')}</span>`}
        <span class="tx" title="${esc(m.file)}"><span class="nm">${esc(m.name)}</span><span class="sb">${[m.ver ? `<span class="mono">${esc(m.ver)}</span>` : '', m.meta?.authors?.length ? esc(m.meta.authors.slice(0, 2).join('、')) : ''].filter(Boolean).join(' · ')}</span></span>
        <span class="tags">${m.missing?.length ? `<span class="chip warn" title="Mod 文件里写着要这些前置，这个实例里没找到：${esc(m.missing.join('、'))}">可能缺 ${esc(m.missing.slice(0, 2).join('、'))}</span>` : ''}${zhChip(c, m)}</span>
        <button type="button" class="iconbtn" data-act="mod-links" data-id="${m.id}" aria-label="${esc(m.name)} 的链接" title="链接：项目页、源码、Modrinth、MC百科">${icon('link', 15)}</button>
    </div>`).join('');
    }
    return list.map((m) => `<div class="row mod${m.on ? '' : ' off'}" data-row="${m.id}">
        <input type="checkbox" class="switch" data-mod="${m.id}" ${m.on ? 'checked' : ''} aria-label="启用 ${esc(m.name)}">
        <span class="tile" style="--h:${m.h}">${esc(m.name[0])}</span>
        <span class="tx"><span class="nm">${esc(m.name)}</span><span class="sb"><span class="mono">${esc(m.ver)}</span> · ${m.src}${zhSub(m, i)}${m.note ? ' · ' + esc(m.note) : ''}</span></span>
        <span class="tags">${zhTag(m, i)}${m.perf ? '<span class="chip">流畅优先</span>' : ''}${m.update ? `<button type="button" class="btn sm" data-act="mod-up" data-id="${m.id}">${icon('arrowUp', 13)}更新到 ${esc(m.update)}</button>` : ''}</span>
        <button type="button" class="iconbtn" data-act="mod-links" data-id="${m.id}" aria-label="${esc(m.name)} 的链接" title="链接：项目页、源码、MC百科">${icon('link', 15)}</button>
        <button type="button" class="iconbtn" data-act="mod-del" data-id="${m.id}" aria-label="删除 ${esc(m.name)}" title="删除">${icon('trash', 15)}</button>
    </div>`).join('');
}

// ---------- 存档 ----------
function worlds(c) {
    const w = c.S.world && c.inst.worlds.find((x) => x.id === c.S.world);
    return w ? worldDetail(c, c.wmeta(w)) : worldList(c);
}

// ---------- 截图 ----------
function shots(c) {
    const i = c.inst;
    if (!i.shots.length) return '<section class="card"><h3>还没有截图</h3><p>游戏里按 F2 截图，会出现在这里。</p></section>';
    return `<div class="shots">${i.shots.map((s) => `<figure class="shot" data-row="${s.id}">
        <canvas data-scene="${i.block}" data-seed="${s.id}" aria-hidden="true"></canvas>
        <figcaption><span class="mono">${ago(s.at)}</span>
            <span class="shot-act"><button type="button" class="iconbtn sm" data-act="shot-copy" aria-label="复制截图" title="复制">${icon('copy', 14)}</button><button type="button" class="iconbtn sm" data-act="shot-del" data-id="${s.id}" aria-label="删除截图" title="删除">${icon('trash', 14)}</button></span>
        </figcaption>
    </figure>`).join('')}</div>`;
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

const SCHED = (c) => [
    { key: 'gpu', label: '用哪块显卡', hint: '笔记本上游戏跑在核显，是最常见的卡。改的是 Windows「图形设置」里这个实例的 javaw.exe', opts: [['auto', '自动（不干预）'], ['high', `高性能：${c.PC.gpus.d}`], ['save', `省电：${c.PC.gpus.i}`]] },
    { key: 'pcore', label: '只用性能核', hint: `${c.PC.cpu}。不一定更快：生成区块要用很多线程，可能反而更慢`, type: 'switch', warn: true },
    { key: 'noeco', label: '切出去不降速', hint: 'Windows 11 会把后台程序降成效率模式；边看攻略边挂机时帧数会掉', type: 'switch' },
    { key: 'prio', label: '进程优先级', opts: [['normal', '正常'], ['above', '高于正常']] },
    { key: 'power', label: '游戏期间用高性能电源计划', hint: '这一项会改系统设置；游戏一退就改回原来的', type: 'switch' },
];

function cfgPanel(c, i, list) {
    return list.map((row) => {
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
}

function config(c) {
    const i = c.inst;
    const t = c.S.tmplLast;
    return `<p class="note">没改过的项跟着全局设置走；在这里改了，只影响这个实例。</p>
    <section class="panel">${cfgPanel(c, i, cfgRows(c, i))}</section>
    <section class="panel"><h2 class="ph">性能调度 · 默认都不干预，每一项都能改回去</h2>${cfgPanel(c, i, SCHED(c))}</section>
    <section class="panel"><h2 class="ph">设置模板</h2>
        <div class="kv"><div class="k"><b>键位、FOV、灵敏度、视距……</b><small>从一个实例的 options.txt 里挑几项，套用到别的实例；新旧版本的键位写法不同，会自动换算。${t ? `上次：从「${esc(t.from)}」套用到 ${t.n} 个实例。` : ''}</small></div>
            <div class="v"><button type="button" class="btn sm" data-act="tmpl-open" data-v="out">${icon('keyboard', 13)}把这里的设置套用到别处</button><button type="button" class="btn sm" data-act="tmpl-open" data-v="in">从别的实例拿</button></div></div>
    </section>`;
}
