// 存档详情：信息 / 地图 / 查找 / 设置 / 指令台。只出 HTML，地图在 world.js 里画
import { esc, block, hours, ago } from '../ui.js';
import { icon } from '../icons.js';
import { lvChip } from './agentView.js';

const chip = (cls, ic, text) => `<span class="chip ${cls}">${ic ? icon(ic, 13) : ''}${esc(text)}</span>`;
const ico = (tone, name) => `<span class="ico ${tone}">${icon(name, 16)}</span>`;
const seg = (act, list, cur, extra = '') => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="${act}" data-v="${v}" ${extra} class="${String(cur) === v ? 'on' : ''}" aria-pressed="${String(cur) === v}">${t}</button>`).join('')}</div>`;
const n0 = (n) => Math.round(n).toLocaleString('zh-CN');
export const xyz = (p) => (p.length === 3 ? `${n0(p[0])}, ${n0(p[1])}, ${n0(p[2])}` : `${n0(p[0])}, ${n0(p[1])}`);

export const RULES = [
    ['keepInventory', '死亡不掉落'],
    ['doDaylightCycle', '昼夜更替'],
    ['mobGriefing', '生物会破坏方块'],
    ['doFireTick', '火会蔓延'],
];

export const STYPES = {
    village: ['村庄', 32], outpost: ['掠夺者前哨站', 350], desert_temple: ['沙漠神殿', 45], jungle_temple: ['丛林神庙', 120],
    swamp_hut: ['女巫小屋', 280], monument: ['海底神殿', 200], mansion: ['林地府邸', 15], shipwreck: ['沉船', 190],
    ruined_portal: ['废弃传送门', 0], stronghold: ['要塞', 260], ancient_city: ['远古城市', 300],
};

// 存档页：列表或者某个世界的详情
export function worldList(c) {
    const i = c.inst;
    const head = `<div class="toolbar">
        <p class="note grow">点「进入」直接进这个世界。点「详情」看种子、地图、找建筑，不用开游戏。${i.sync.on ? '开了同步，每次退出游戏后自动上传，「历史」里能退回任意一次。' : ''}</p>
        <button type="button" class="btn" data-act="nw-open">${icon('plus')}新建世界</button>
    </div>`;
    if (!i.worlds.length) return head + '<section class="card"><h3>还没有存档</h3><p>进游戏建一个世界，或者点「新建世界」在这里先建好。</p></section>';
    return `${head}<div class="rows">${i.worlds.map((w) => `<div class="row world" data-row="${w.id}">
        ${block(w.skyblock ? 'diamond' : 'grass', 32)}
        <span class="tx"><span class="nm">${esc(w.name)}</span><span class="sb">${esc(w.mode)} · ${esc(w.diff)} · ${w.fresh ? '还没进过' : esc(ago(w.last))}</span></span>
        <span class="mono sb">${esc(w.size)}</span>
        <span class="tags">
            <button type="button" class="btn sm" data-act="world-play" data-v="${esc(w.name)}">${icon('play', 13)}进入</button>
            <button type="button" class="btn sm" data-act="world-open" data-v="${w.id}">${icon('globe', 13)}详情</button>
            ${i.sync.on && w.history.length ? `<button type="button" class="btn sm" data-act="sync-hist" data-v="${w.id}">${icon('history', 13)}历史</button>` : ''}
        </span>
    </div>`).join('')}</div>`;
}

export function worldDetail(c, w) {
    const tabs = [['info', '信息'], ['map', '地图'], ['find', '查找'], ['edit', '设置'], ['cmd', '指令台']];
    return `<div class="wd-head">
        <button type="button" class="btn ghost sm" data-act="world-back">${icon('chevron', 14)}所有存档</button>
        <span class="wd-title">${block(w.skyblock ? 'diamond' : 'grass', 26)}<span class="tx"><span class="nm">${esc(w.name)}</span><span class="sb">${esc(w.mode)} · ${esc(w.diff)} · ${esc(w.size)}</span></span></span>
        <span class="sp"></span>
        <button type="button" class="btn sm" data-act="backup" data-v="${esc(w.name)}">备份</button>
        <button type="button" class="btn sm pri" data-act="world-play" data-v="${esc(w.name)}">${icon('play', 13)}进入</button>
    </div>
    <div class="wd-tabs">${seg('wtab', tabs, c.S.wtab)}</div>
    <div class="wd-body" id="wdBody">${wdBody(c, w)}</div>`;
}

export function wdBody(c, w) {
    if (w.fresh && c.S.wtab !== 'edit' && c.S.wtab !== 'info') {
        return `<section class="card"><h3>${ico('sky', 'globe')}还没进过这个世界</h3><p>地形会在你第一次进去时由游戏生成。进去玩一会儿再回来，这里就有地图和建筑了。</p>
            <div class="actions"><button type="button" class="btn pri" data-act="world-play" data-v="${esc(w.name)}">${icon('play')}进入</button></div></section>`;
    }
    switch (c.S.wtab) {
        case 'map': return c.S.ext.world ? mapView(c, w) : packGate(c, '地图');
        case 'find': return c.S.ext.world ? findView(c, w) : packGate(c, '查找建筑');
        case 'edit': return editView(c, w);
        case 'cmd': return cmdView(c, w);
        default: return infoView(c, w);
    }
}

// 功能包没装：入口还在，写清楚要装什么、多大
function packGate(c, what) {
    const busy = c.S.extBusy === 'world';
    return `<section class="card gate">
        <h3>${ico('violet', 'package')}${what}在「世界工具」功能包里</h3>
        <p>里面是地图渲染、按种子推算结构的程序（cubiomes）和方块颜色表，约 6 MB。官方签名，装完立刻能用，不用重启。</p>
        <div class="actions">${busy ? `<span class="bar w live" data-extp="world"><i style="--p:${c.S.extPct || 0}%"></i></span><span class="mono sb" data-extt="world">${c.S.extStage || ''} ${c.S.extPct || 0}%</span>` : `<button type="button" class="btn pri" data-act="ext-add" data-v="world">${icon('download')}装上世界工具（约 6 MB）</button>`}</div>
    </section>`;
}

// ---------- 信息 ----------
function infoView(c, w) {
    const i = c.inst;
    const hp = w.hp ?? 20;
    const food = w.food ?? 20;
    const pips = (n, cls) => Array.from({ length: 10 }, (_, k) => `<i class="${cls}${n >= (k + 1) * 2 ? '' : n >= k * 2 + 1 ? ' half' : ' empty'}"></i>`).join('');
    const rules = RULES.map(([k, t]) => `<div class="mrow"><span>${t}</span><span class="mono sb">${k}</span>${w.rules[k] ? chip('ok', '', '开') : chip('', '', '关')}</div>`).join('');
    return `<div class="grid2">
        <section class="card">
            <h3>${ico('sky', 'globe')}世界</h3>
            <dl class="facts">
                <dt>种子</dt><dd class="seedline"><span class="mono">${esc(w.seed)}</span><button type="button" class="iconbtn sm" data-act="seed-copy" aria-label="复制种子" title="复制种子">${icon('copy', 14)}</button></dd>
                <dt>版本</dt><dd class="mono">${esc(i.mc)}${i.loader ? ` · ${esc(i.loader)}` : ''}</dd>
                <dt>模式</dt><dd>${esc(w.mode)} · ${esc(w.diff)}${w.cheats ? ' · 允许作弊' : ''}</dd>
                <dt>世界类型</dt><dd>${w.skyblock ? '空岛（整合包生成的）' : esc(w.type || '默认')}</dd>
                <dt>游戏时长</dt><dd>${w.fresh ? '还没进过' : hours(w.playMin)}</dd>
                <dt>出生点</dt><dd class="mono">${xyz(w.spawn)}</dd>
            </dl>
            <p class="note">读的是存档里的 level.dat，不用开游戏。</p>
        </section>
        <section class="card">
            <h3>${ico('violet', 'user')}玩家 · ${esc(c.account(i).name)}</h3>
            ${w.fresh ? '<p>还没进过这个世界。</p>' : `<dl class="facts">
                <dt>位置</dt><dd><span class="mono">${xyz(w.pos)}</span> · 主世界</dd>
                <dt>生命</dt><dd class="pips">${pips(hp, 'heart')}<span class="mono sb">${hp}/20</span></dd>
                <dt>饱食度</dt><dd class="pips">${pips(food, 'food')}<span class="mono sb">${food}/20</span></dd>
                <dt>经验等级</dt><dd class="mono">${w.lvl}</dd>
            </dl>
            <div class="inv">${w.inv.map(([n, col, k]) => `<span class="slot" title="${esc(n)}${k ? ' × ' + k : ''}"><i style="--c:${col}"></i>${k ? `<b>${k}</b>` : ''}</span>`).join('')}${Array.from({ length: Math.max(0, 9 - w.inv.length) }, () => '<span class="slot"></span>').join('')}</div>`}
        </section>
        <section class="card">
            <h3>${ico('amber', 'sliders')}游戏规则</h3>
            <div class="mini-rows">${rules}</div>
            <div class="actions"><button type="button" class="btn sm" data-act="wtab" data-v="edit">${icon('wrench', 13)}去改</button></div>
        </section>
        <section class="card">
            <h3>${ico('bad', 'alert')}死过的地方</h3>
            ${w.deaths.length ? `<div class="mini-rows">${w.deaths.map((d, k) => `<div class="mrow"><span class="mono">${xyz(d)}</span><button type="button" class="btn sm" data-act="map-goto" data-x="${d[0]}" data-z="${d[1]}" data-k="death${k}">${icon('eye', 13)}在地图上看</button></div>`).join('')}</div>` : '<p>还没死过。</p>'}
        </section>
    </div>`;
}

// ---------- 地图 ----------
function mapView(c, w) {
    const m = c.S.map;
    return `<div class="mapbox">
        <div class="map-wrap" id="mapWrap"><canvas id="wmap" aria-label="世界地图：拖动平移，滚轮缩放"></canvas>
            <div class="map-tools">
                ${seg('map-layer', [['surface', '地表'], ['under', '地下 Y -30'], ['nether', '下界']], m.layer)}
                <span class="sp"></span>
                <div class="zoom"><button type="button" class="iconbtn sm" data-act="map-zoom" data-v="-1" aria-label="放大" title="放大">${icon('plus', 15)}</button><button type="button" class="iconbtn sm" data-act="map-zoom" data-v="1" aria-label="缩小" title="缩小"><svg class="ic" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M5 12h14"/></svg></button></div>
            </div>
            <div class="map-read" id="mapRead">拖动平移，滚轮缩放</div>
        </div>
        <div class="map-foot">
            <div class="legend"><span><i class="lg-spawn"></i>出生点</span><span><i class="lg-me"></i>你</span><span><i class="lg-death"></i>死过的地方</span><span><i class="lg-fog"></i>没去过（存档里没有）</span></div>
            <span class="sp"></span>
            <button type="button" class="btn sm" data-act="map-home">${icon('home', 13)}出生点</button>
            <button type="button" class="btn sm" data-act="map-me">${icon('user', 13)}我在哪</button>
            <span class="mono sb" id="mapScale"></span>
        </div>
        <p class="note">读存档里的区块画的俯视图：去过的地方是真的；灰掉的地方存档里还没有，这里按种子推算着画了个大概。Mod 方块用它材质的平均色。</p>
    </div>`;
}

// ---------- 查找 ----------
function findView(c, w) {
    const all = c.structs(w);
    const f = c.S.findType;
    const counts = {};
    all.forEach((s) => { counts[s.type] = (counts[s.type] || 0) + 1; });
    const types = Object.keys(STYPES).filter((t) => counts[t]);
    const list = all.filter((s) => f === 'all' || s.type === f);
    const gen = list.filter((s) => s.gen);
    const calc = w.skyblock ? [] : list.filter((s) => !s.gen);
    const row = (s) => `<div class="row find">
        <span class="stile" style="--h:${STYPES[s.type][1]}">${icon(s.type === 'village' ? 'home' : s.type === 'stronghold' || s.type === 'ancient_city' ? 'shield' : 'globe', 15)}</span>
        <span class="tx"><span class="nm">${esc(STYPES[s.type][0])}</span><span class="sb mono">X ${n0(s.x)} · Z ${n0(s.z)}${s.y != null ? ` · Y ${s.y}` : ''}</span></span>
        <span class="sb">离出生点 ${s.dist >= 1000 ? (s.dist / 1000).toFixed(1) + ' km' : n0(s.dist) + ' 格'}</span>
        <button type="button" class="btn sm" data-act="map-goto" data-x="${s.x}" data-z="${s.z}" data-k="${s.id}">${icon('eye', 13)}在地图上看</button>
    </div>`;
    return `${w.skyblock ? `<p class="callout warn">${icon('alert', 14)}<span>这个世界是空岛，地形不是按原版方式生成的。按种子推算的位置不准，所以只列存档里真有的。</span></p>` : ''}
    <div class="toolbar">
        ${seg('find-type', [['all', `全部 ${all.length}`], ...types.map((t) => [t, `${STYPES[t][0]} ${counts[t]}`])], f)}
    </div>
    <h3 class="sh">${icon('check', 14)}已经生成的区域 · 读存档，准</h3>
    <div class="rows">${gen.length ? gen.map(row).join('') : '<div class="row empty">去过的地方里没有这类建筑</div>'}</div>
    ${w.skyblock ? '' : `<h3 class="sh">${icon('sparkles', 14)}还没去过 · 按种子推算</h3>
    <div class="rows">${calc.length ? calc.map(row).join('') : '<div class="row empty">附近没推算出这类建筑</div>'}</div>
    <p class="note">推算用的是 cubiomes（MIT 许可），和 Chunkbase 一个原理，只对原版地形准；装了改地形的 Mod 就不准了。</p>`}`;
}

// ---------- 设置（局外修改） ----------
function editView(c, w) {
    const d = c.S.wdraft;
    const run = c.RUN.get(c.inst.id)?.phase === 'running';
    const sel = (k, opts) => `<select data-wd="${k}" aria-label="${k}"${run ? ' disabled' : ''}>${opts.map(([v, t]) => `<option value="${v}"${String(d[k]) === v ? ' selected' : ''}>${t}</option>`).join('')}</select>`;
    const sw = (k, on, label) => `<input type="checkbox" class="switch" data-wd="${k}"${on ? ' checked' : ''}${run ? ' disabled' : ''} aria-label="${label}">`;
    const dirty = c.wdirty(w);
    return `${run ? `<p class="callout warn">${icon('alert', 14)}<span>游戏正开着，存档被占用（session.lock）。退出游戏后再改。</span></p>` : ''}
    <section class="panel">
        <div class="kv"><div class="k"><b>游戏模式</b></div><div class="v">${sel('mode', [['生存', '生存'], ['创造', '创造'], ['冒险', '冒险'], ['旁观', '旁观']])}</div></div>
        <div class="kv"><div class="k"><b>难度</b></div><div class="v">${sel('diff', [['和平', '和平'], ['简单', '简单'], ['普通', '普通'], ['困难', '困难']])}</div></div>
        <div class="kv"><div class="k"><b>允许作弊</b><small>能用 /gamemode、/tp 这些命令</small></div><div class="v">${sw('cheats', d.cheats, '允许作弊')}</div></div>
        ${RULES.map(([k, t]) => `<div class="kv"><div class="k"><b>${t}</b><small class="mono">${k}</small></div><div class="v">${sw('r:' + k, d.rules[k], t)}</div></div>`).join('')}
        <div class="kv"><div class="k"><b>时间</b></div><div class="v">${sel('time', [['keep', '不改'], ['day', '白天'], ['noon', '正午'], ['night', '夜晚']])}</div></div>
        <div class="kv"><div class="k"><b>天气</b></div><div class="v">${sel('weather', [['keep', '不改'], ['clear', '晴'], ['rain', '下雨'], ['thunder', '雷雨']])}</div></div>
    </section>
    <div class="actions">
        <button type="button" class="btn pri" data-act="wd-save"${dirty && !run ? '' : ' disabled'}>${icon('check')}保存修改</button>
        ${dirty ? '<button type="button" class="btn ghost" data-act="wd-reset">不改了</button>' : ''}
        <span class="note">改之前自动备份整个世界；出了问题能撤销。</span>
    </div>`;
}

// ---------- 指令台（默认关） ----------
function cmdView(c, w) {
    if (!c.S.console) {
        return `<section class="card">
            <h3>${ico('violet', 'terminal')}局外指令台${chip('', '', '默认关着')}</h3>
            <p>不开游戏也能对这个世界执行命令：LeafStarter 在后台用同版本、同样的服务端 Mod 起一个看不见的本地服务器，加载这个世界，执行完存档、关掉。</p>
            <ul class="ag-points">
                <li>${icon('undo', 14)}<span>每次执行前先给世界建快照，不满意一键回滚</span></li>
                <li>${icon('sparkles', 14)}<span>可以让 AI 把一句话写成命令，执行前给你看</span></li>
                <li>${icon('alert', 14)}<span>只装在客户端的 Mod 不生效；第一次要下载服务端（约 48 MB，和开服用的是同一套）</span></li>
            </ul>
            <div class="actions"><button type="button" class="btn pri" data-act="wc-enable">${icon('terminal')}打开指令台</button></div>
        </section>`;
    }
    const W = c.S.wc;
    const run = c.RUN.get(c.inst.id)?.phase === 'running';
    const ai = c.aiFor('cmd');
    const st = { off: chip('', '', '本地服务器没开'), boot: chip('acc', 'zap', '正在起本地服务器'), on: chip('ok', 'check', '执行中'), dl: chip('acc', 'download', `下载服务端 ${W.pct || 0}%`) }[W.state];
    return `${run ? `<p class="callout warn">${icon('alert', 14)}<span>游戏正开着这个实例，存档被占用。退出游戏后再用指令台。</span></p>` : ''}
    <section class="card">
        <h3>${ico('violet', 'terminal')}指令台${st}</h3>
        <div class="ai-cmd">
            <label class="filter grow" for="wcAsk">${icon('sparkles', 14)}<input id="wcAsk" type="text" placeholder="说你想做什么，比如：在出生点造一个 9×9 的玻璃平台" autocomplete="off"${run || !ai ? ' disabled' : ''}></label>
            <button type="button" class="btn" data-act="wc-ask"${run || !ai ? ' disabled' : ''}>写成命令</button>
        </div>
        <small class="note">${ai ? `用「写命令」这个用途的 AI 源：${esc(ai.name)}。` : '没配 AI 源也能用：直接在下面输命令。'}试试：造玻璃平台、改成白天、天晴、给我 64 个钻石、回出生点、死亡不掉落。</small>
        <div id="wcConfirm">${W.ask ? confirmCard(W.ask) : ''}</div>
        <div class="console">
            <pre id="wcLog" tabindex="0">${W.log.length ? W.log.map((l) => l).join('\n') : '<span class="o">（还没执行过命令）</span>'}</pre>
            <div class="cin"><span class="mono">&gt;</span><input id="wcCmd" type="text" placeholder="直接输命令，回车执行，比如 /time set day" autocomplete="off"${run ? ' disabled' : ''}></div>
        </div>
        ${W.snaps.length ? `<div class="mini-rows">${W.snaps.map((s, k) => `<div class="mrow"><span class="tx"><span class="nm">${icon('history', 13)} 执行前快照 · ${esc(s.at)}</span><span class="sb mono">${esc(s.cmds.join('  '))}</span></span>${s.undone ? '<span class="chip">已回滚</span>' : `<button type="button" class="btn sm" data-act="wc-undo" data-k="${k}">${icon('undo', 13)}回滚</button>`}</div>`).join('')}</div>` : ''}
    </section>`;
}

export function confirmCard(a) {
    return `<div class="ag-confirm lv-rev${a.state === 'wait' ? ' wait' : ''}">
        <div class="h">${lvChip('rev')}<b>AI 写好了 ${a.cmds.length} 条命令</b></div>
        <p>${esc(a.say)}执行前先给世界建快照，不满意一键回滚。</p>
        <pre class="cmdbox">${a.cmds.map((l) => `<span class="p">&gt;</span> ${esc(l)}`).join('\n')}</pre>
        <div class="actions">${a.state === 'wait' ? '<button type="button" class="btn sm pri" data-act="wc-yes">执行</button><button type="button" class="btn sm ghost" data-act="wc-no">不要</button>' : `<span class="ag-res ${a.state}">${a.state === 'no' ? icon('x', 13) + '没执行' : icon('check', 13) + '已执行'}</span>`}</div>
    </div>`;
}
