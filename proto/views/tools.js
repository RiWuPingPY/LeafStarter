// 工具类的界面：显卡与渲染器、中文情况、Mod 外链、补全中文、设置模板、局外新建世界。只出 HTML
import { esc } from '../ui.js';
import { icon } from '../icons.js';
import { renderers, modLinks, PC } from '../data.js';

const chip = (cls, ic, text) => `<span class="chip ${cls}">${ic ? icon(ic, 13) : ''}${esc(text)}</span>`;
const ico = (tone, name) => `<span class="ico ${tone}">${icon(name, 16)}</span>`;
const seg = (act, list, cur, extra = '') => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="${act}" data-v="${v}" ${extra} class="${String(cur) === v ? 'on' : ''}" aria-pressed="${String(cur) === v}">${t}</button>`).join('')}</div>`;
const num = (n) => n.toLocaleString('zh-CN');

// ---------- 显卡与渲染器（概览里的卡片） ----------
export function gpuCard(i, c) {
    const gpu = i.cfg.gpu || c.GLOBAL.gpu;
    const seen = i.gpuSeen;
    const wrong = seen === 'i' && gpu !== 'high';
    const head = wrong ? chip('warn', 'alert', '上次跑在核显上') : gpu === 'high' ? chip('ok', 'check', '用独显') : '';
    const seenText = (!seen ? '还没启动过' : seen === 'i' ? `${PC.gpus.i}（核显）` : `${PC.gpus.d}（独显）`) + (seen !== 'd' && gpu === 'high' ? '，下次启动换成独显' : '');
    const list = renderers(i.loader);
    const have = (n) => i.mods.find((m) => m.name === n);
    const rows = list.map((r) => {
        const m = have(r.name);
        const clash = r.conflicts?.filter((n) => have(n)?.on);
        const act = m
            ? m.on ? `<span class="chip ok">${icon('check', 12)}已装</span>` : '<span class="chip">装了，已禁用</span>'
            : clash?.length
                ? `<button type="button" class="btn sm" data-act="rd-clash" data-v="${esc(r.name)}">${icon('alert', 13)}有冲突</button>`
                : `<button type="button" class="btn sm" data-act="rd-add" data-v="${esc(r.name)}">${icon('plus', 13)}装上</button>`;
        return `<div class="mrow"><span class="tx"><span class="nm">${esc(r.name)}${r.exp ? ' <span class="chip warn xs">实验</span>' : ''}</span><span class="sb wrap">${esc(r.why)}</span></span>${act}</div>`;
    }).join('');
    return `<section class="card">
        <h3>${ico('sky', 'cpu')}显卡与渲染器${head}</h3>
        <dl class="facts">
            <dt>这台电脑</dt><dd>${esc(PC.gpus.d)} + ${esc(PC.gpus.i)}</dd>
            <dt>上次用的</dt><dd${wrong ? ' class="warnt"' : ''}>${esc(seenText)}</dd>
        </dl>
        ${wrong ? `<p>笔记本上游戏跑在核显，是最常见的「卡」。改成用独显只动 Windows「图形设置」里这个实例的 javaw.exe，随时能改回。</p>
            <div class="actions"><button type="button" class="btn pri" data-act="gpu-fix">${icon('zap')}改成用独显</button><button type="button" class="btn ghost" data-act="tab" data-tab="config">更多性能调度</button></div>` : ''}
        ${list.length ? `<div class="mini-rows rd">${rows}</div>` : '<p>原版装不了渲染器 Mod。显卡和性能调度照样有用，在「设置」里。</p>'}
    </section>`;
}

// ---------- 中文情况 ----------
export const zhState = (m, i) => (['ai', 'mt', 'off'].includes(m.zh) && !i.zhPack?.on ? 'none' : m.zh);
const ZHN = { own: '自带中文', cfpa: '社区汉化', ai: 'AI 补的', mt: '机器翻的', off: '离线翻的', none: '没中文' };

export function zhCount(i) {
    const n = { own: 0, cfpa: 0, fill: 0, none: 0, lines: 0 };
    for (const m of i.mods) {
        const z = zhState(m, i);
        if (z === 'own') n.own += 1;
        else if (z === 'cfpa') n.cfpa += 1;
        else if (z === 'none') {
            n.none += 1;
            n.lines += m.lines || 0;
        } else if (z !== 'na') n.fill += 1;
    }
    return n;
}

export function zhCard(i) {
    if (!i.loader) {
        return `<section class="card"><h3>${ico('ok', 'globe')}中文${chip('ok', 'check', '原版自带')}</h3><p>原版自带简体中文。装了 Mod 的实例，这里会列出哪些 Mod 没有中文，可以一键补上。</p></section>`;
    }
    const n = zhCount(i);
    const total = n.own + n.cfpa + n.fill + n.none || 1;
    const seg2 = (k, v) => (v ? `<i class="zb-${k}" style="flex:${v}" title="${v}"></i>` : '');
    const pack = i.zhPack;
    return `<section class="card">
        <h3>${ico('ok', 'globe')}中文${n.none ? chip('warn', '', `${n.none} 个 Mod 没中文`) : chip('ok', 'check', '都有中文')}</h3>
        <div class="zbar" aria-hidden="true">${seg2('own', n.own)}${seg2('cfpa', n.cfpa)}${seg2('fill', n.fill)}${seg2('none', n.none)}</div>
        <div class="zleg"><span><i class="zb-own"></i>自带 ${n.own}</span><span><i class="zb-cfpa"></i>社区汉化 ${n.cfpa}</span>${n.fill ? `<span><i class="zb-fill"></i>补的 ${n.fill}</span>` : ''}<span><i class="zb-none"></i>没中文 ${n.none}</span></div>
        ${pack ? `<div class="kvline"><span>${icon('package', 14)}汉化补全资源包 · ${pack.n} 条 · 放在最上面</span><input type="checkbox" class="switch" id="zhPackSw" ${pack.on ? 'checked' : ''} aria-label="汉化补全资源包"></div>` : ''}
        ${n.none ? `<p>没中文的 ${n.none} 个 Mod 共 ${num(n.lines)} 条文字。先用社区人工汉化，剩下的再用机器或 AI 补，生成一个资源包叠在最上面，不改 Mod 文件。</p>
            <div class="actions"><button type="button" class="btn pri" data-act="zh-open">${icon('sparkles')}补全中文</button></div>` : ''}
    </section>`;
}

// Mod 行里的中文标记：只标需要注意的，自带和社区汉化写在副标题里
export function zhTag(m, i) {
    const z = zhState(m, i);
    if (z === 'none') return '<span class="chip warn">没中文</span>';
    if (['ai', 'mt', 'off'].includes(z)) return `<span class="chip acc">${ZHN[z]}</span>`;
    return '';
}
export const zhSub = (m, i) => (['own', 'cfpa'].includes(zhState(m, i)) ? ` · ${ZHN[zhState(m, i)]}` : '');

// ---------- Mod 外链（小弹层） ----------
export function linksPop(m) {
    return `<div class="pop-h">${esc(m.name)} 的链接</div>`
        + modLinks(m).map(([ic, t, url]) => `<button type="button" class="pop-item" data-act="link-open" data-v="${esc(url)}">${icon(ic, 16)}<span class="tx"><span class="nm">${t}</span><span class="sb mono">${esc(url)}</span></span>${icon('link', 13)}</button>`).join('')
        + '<p class="pop-note">示例链接。正式版从 Modrinth / CurseForge 的接口拿到源码、问题反馈和 Wiki 地址；MC百科 只给搜索链接，不爬它的页面。</p>';
}

// ---------- 补全中文（右侧面板） ----------
const ENG = {
    ai: ['sparkles', 'AI 翻译', '带上 Mod 的用途和术语表，质量最好'],
    mt: ['globe', '机器翻译', '百度、腾讯、DeepL 等接口，多数每月有免费额度'],
    off: ['cpu', '离线翻译', '不联网、不花钱，质量一般；要先下载模型'],
};
const GLOSS = [['Iron Ingot', '铁锭'], ['Redstone Dust', '红石粉'], ['Crafting Table', '工作台'], ['Nether Star', '下界之星'], ['Ender Pearl', '末影珍珠']];

export function i18nDrawer(c) {
    const d = c.S.drawer;
    const i = c.byId(d.id);
    const none = i.mods.filter((m) => zhState(m, i) === 'none');
    const pick = none.filter((m) => d.pick.includes(m.id));
    const lines = pick.reduce((a, m) => a + (m.lines || 0), 0);
    const n = zhCount(i);
    let body;
    if (d.phase === 'plan') {
        const trSrc = c.aiFor('tr');
        const engNote = {
            ai: trSrc ? `用「翻译」这个用途的 AI 源：<b>${esc(trSrc.name)}</b> · <span class="mono">${esc(trSrc.model || '未填模型')}</span>` : '还没配 AI 源。<button type="button" class="link" data-act="ai-settings">去设置</button>',
            mt: '用设置里填的翻译接口（原型里当作已填好）',
            off: c.S.ext.tr ? '离线翻译模型已下载，在本机跑' : '模型约 180 MB，不随安装包带。从魔搭社区下载，不走 LeafStarter 的服务器',
        }[d.engine];
        const est = {
            ai: `约 ${num(Math.round(lines * 46 / 100) * 100)} token，在每次会话的上限内`,
            mt: `约 ${num(lines * 38)} 个字符，在多数接口的每月免费额度内`,
            off: `本机跑，约 ${Math.max(1, Math.round(lines / 400))} 分钟`,
        }[d.engine];
        body = `<div class="field"><span>三层来源，从准到粗</span>
                <ol class="tiers">
                    <li class="done"><b>Mod 自带中文</b><small>${n.own} 个，一个字都不动</small></li>
                    <li class="done"><b>社区人工汉化</b><small>${n.cfpa} 个，用 CFPA 团队的模组汉化资源包</small></li>
                    <li class="run"><b>机器或 AI 补齐</b><small>剩下 ${none.length} 个 Mod、${num(none.reduce((a, m) => a + (m.lines || 0), 0))} 条，只补缺的</small></li>
                </ol></div>
            <div class="field"><span>用什么翻</span>
                <div class="engs">${Object.entries(ENG).map(([k, [ic, t, s]]) => `<button type="button" class="eng${d.engine === k ? ' on' : ''}" data-act="zh-eng" data-v="${k}" aria-pressed="${d.engine === k}">${icon(ic, 18)}<span class="tx"><span class="nm">${t}</span><span class="sb wrap">${s}</span></span></button>`).join('')}</div>
                <small class="note">${engNote}</small>
                ${d.engine === 'off' && !c.S.ext.tr ? `<div class="actions">${c.S.extBusy === 'tr' ? `<span class="bar w live" data-extp="tr"><i style="--p:${c.S.extPct || 0}%"></i></span><span class="mono sb" data-extt="tr">${c.S.extPct || 0}%</span>` : `<button type="button" class="btn sm" data-act="ext-add" data-v="tr">${icon('download', 13)}下载离线翻译（约 180 MB）</button>`}</div>` : ''}
            </div>
            <div class="field"><span>翻哪几个 Mod</span>
                <div class="picks">${none.map((m) => `<label class="tick pk"><input type="checkbox" data-zhpick="${m.id}"${d.pick.includes(m.id) ? ' checked' : ''}><span class="tile sm" style="--h:${m.h}">${esc(m.name[0])}</span><span class="tx"><span class="nm">${esc(m.name)}</span><span class="sb">${num(m.lines || 0)} 条</span></span></label>`).join('')}</div></div>
            <div class="field"><span>术语表</span>
                <div class="gloss">${GLOSS.map(([a, b]) => `<span><span class="mono">${a}</span>${icon('chevron', 12)}<b>${b}</b></span>`).join('')}</div>
                <small class="note">从游戏自带的英文、中文语言文件自动对出约 6 千条原版术语，再叠上社区汉化里的 Mod 术语，「铁锭」不会被翻成「铁块」。</small></div>
            <div class="est">${icon('clock', 15)}<span><b>${num(lines)} 条</b> · ${est}</span></div>`;
    } else {
        const rows = d.run.map((r) => `<div class="zrow ${r.state}"><span class="tile sm" style="--h:${r.h}">${esc(r.name[0])}</span>
            <span class="tx"><span class="nm">${esc(r.name)}</span><span class="sb wrap">${r.state === 'wait' ? `${num(r.lines)} 条，排队` : r.state === 'run' ? `正在翻 ${num(r.done)} / ${num(r.lines)}` : `${num(r.lines - r.bad)} 条翻好${r.bad ? `，<b class="warnt">${r.bad} 条格式符对不上，退回英文</b>` : '，格式符都对得上'}`}</span>
            ${r.state === 'run' ? `<span class="bar live"><i style="--p:${Math.round((r.done / r.lines) * 100)}%"></i></span>` : ''}</span>
            <span class="sic">${r.state === 'done' ? icon('check', 12) : ''}</span></div>`).join('');
        body = `<div class="zrows">${rows}</div>
            ${d.phase === 'done' ? `<section class="card flat in">
                <h3>${ico('ok', 'package')}生成了资源包「LeafStarter 汉化补全」</h3>
                <p>放在资源包列表最上面，盖住英文；Mod 文件一个没动，关掉这个资源包就恢复原样。</p>
                <div class="samples">${SAMPLES.map(([k, en, zh, bad]) => `<div class="smp${bad ? ' bad' : ''}"><span class="mono sb">${esc(k)}</span><span class="mono">${esc(en)}</span><span>${bad ? `${icon('x', 12)}${esc(zh)} <small>少了 §e §r，退回英文</small>` : esc(zh)}</span></div>`).join('')}</div>
                <ul class="ag-points">
                    <li>${icon('shield', 14)}<span>译文只在这台电脑上，只给你自己用，不对外分发</span></li>
                    <li>${icon('refresh', 14)}<span>按原文缓存：Mod 更新后只翻新增和改过的条目</span></li>
                </ul>
            </section>` : ''}`;
    }
    const busy = d.phase === 'run';
    const can = d.phase === 'done' || (pick.length && !(d.engine === 'ai' && !c.aiFor('tr')) && !(d.engine === 'off' && !c.S.ext.tr));
    const lbl = d.phase === 'done' ? '完成' : busy ? '正在补…' : '开始补全';
    const sub = d.phase === 'done' ? '回到 Mod 列表' : busy ? '可以关掉面板，后台接着翻' : `${pick.length} 个 Mod · ${num(lines)} 条 · ${ENG[d.engine][1]}`;
    return `<section class="drawer" role="dialog" aria-label="补全中文">
        <header class="dr-head"><h2>补全中文 · ${esc(i.name)}</h2><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
        <div class="dr-body" id="zhBody">${body}</div>
        <footer class="dr-foot">
            <span class="hint">${d.phase === 'plan' ? '翻完逐条检查 %s、§a 这类格式符' : d.phase === 'run' ? '翻译在后台跑' : '资源包已经放好，下次启动生效'}</span>
            <button type="button" class="go${busy ? ' busy' : ''}" id="drCreate" data-act="dr-create"${can && !busy ? '' : ' disabled'}>${busy ? '<span class="fill" style="--p:' + d.pct + '%"></span>' : ''}${icon(d.phase === 'done' ? 'check' : 'sparkles', 22)}<span class="txt"><span class="lbl">${lbl}</span><span class="sub">${esc(sub)}</span></span>${busy ? '' : '<kbd>Enter</kbd>'}</button>
        </footer>
    </section>`;
}

const SAMPLES = [
    ['item.techreborn.industrial_centrifuge', 'Industrial Centrifuge', '工业离心机'],
    ['techreborn.tooltip.energy', '§aEnergy: %1$s / %2$s', '§a能量：%1$s / %2$s'],
    ['glimmer_render.option.sway', 'Foliage sway strength: %s', '植被摆动强度：%s'],
    ['glimmer_render.tip.glow', 'Glows §e%d§r blocks away', '在 %d 格外发光', true],
];

// ---------- 设置模板：键位、FOV 等，从一个实例套用到别的实例 ----------
export const TMPL_ITEMS = [
    ['keys', '键位', (i) => `${i.loader ? 63 : 42} 个${i.loader ? '，其中 Mod 的 21 个' : ''}`],
    ['fov', '视野（FOV）', () => '90'],
    ['sens', '鼠标灵敏度', () => '112%'],
    ['view', '视距', () => '12 区块'],
    ['sound', '音量', () => '主音量 70%，音乐 20%'],
    ['gui', '界面大小', () => '3'],
    ['lang', '语言', () => '简体中文'],
];
const oldKeys = (mc) => { const [, b] = mc.split('.').map(Number); return b <= 12; };

export function tmplDrawer(c) {
    const d = c.S.drawer;
    const src = c.byId(d.from);
    const targets = c.INSTANCES.filter((x) => x.id !== src.id);
    const picked = targets.filter((x) => d.to.includes(x.id));
    const items = TMPL_ITEMS.filter(([k]) => d.items[k]);
    const conv = picked.filter((x) => oldKeys(x.mc) !== oldKeys(src.mc));
    return `<section class="drawer" role="dialog" aria-label="设置模板">
        <header class="dr-head"><h2>套用设置</h2><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
        <div class="dr-body">
            <label class="field" for="tmFrom"><span>从哪个实例拿</span>
                <select id="tmFrom">${c.INSTANCES.map((x) => `<option value="${x.id}"${x.id === src.id ? ' selected' : ''}>${esc(x.name)}（${esc(x.mc)}）</option>`).join('')}</select></label>
            <div class="field"><span>要哪些（读它的 options.txt）</span>
                <div class="picks">${TMPL_ITEMS.map(([k, t, v]) => `<label class="tick pk"><input type="checkbox" data-tmitem="${k}"${d.items[k] ? ' checked' : ''}><span class="tx"><span class="nm">${t}</span><span class="sb">${esc(v(src))}</span></span></label>`).join('')}</div></div>
            <div class="field"><span>套用到</span>
                <div class="picks">${targets.map((x) => `<label class="tick pk"><input type="checkbox" data-tmto="${x.id}"${d.to.includes(x.id) ? ' checked' : ''}>${'<i class="blk blk-' + x.block + '" style="--s:22px" aria-hidden="true"></i>'}<span class="tx"><span class="nm">${esc(x.name)}</span><span class="sb">${esc(x.mc)} · ${x.loader ? esc(x.loader) : '原版'}${oldKeys(x.mc) !== oldKeys(src.mc) ? ' · 键位要换算' : ''}</span></span></label>`).join('')}</div></div>
            ${conv.length && d.items.keys ? `<p class="callout">${icon('refresh', 14)}<span>${esc(conv.map((x) => x.name).join('、'))} 是 ${oldKeys(conv[0].mc) ? '1.12 及以前' : '1.13 以后'}的版本，键位写法不一样（数字键码 ↔ <span class="mono">key.keyboard.*</span>），会自动换算；对不上的 Mod 键位跳过。</span></p>` : ''}
            <label class="check" for="tmDefault"><input type="checkbox" id="tmDefault"${d.asDefault ? ' checked' : ''}>
                <span><b>以后新建实例也用这套</b><small>存成默认模板</small></span></label>
            <p class="note">同步到手机时不带键位（触屏操作不一样）。套用前会备份各实例原来的 options.txt，能撤销。</p>
        </div>
        <footer class="dr-foot">
            <span class="hint">${items.length} 项设置</span>
            <button type="button" class="go" id="drCreate" data-act="dr-create"${picked.length && items.length ? '' : ' disabled'}>${icon('keyboard', 22)}<span class="txt"><span class="lbl">套用设置</span><span class="sub">${picked.length ? `到 ${picked.length} 个实例 · ` : '先勾套用到哪 · '}${esc(items.map(([, t]) => t).join('、') || '先勾要哪些')}</span></span><kbd>Enter</kbd></button>
        </footer>
    </section>`;
}

// ---------- 局外新建世界 ----------
export const canMakeWorld = (mc) => !oldKeys(mc);

export function worldDrawer(c) {
    const d = c.S.drawer;
    const i = c.byId(d.id);
    const ok = canMakeWorld(i.mc);
    return `<section class="drawer" role="dialog" aria-label="新建世界">
        <header class="dr-head"><h2>新建世界 · ${esc(i.name)}</h2><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
        <div class="dr-body">
            ${ok ? '' : `<p class="callout warn">${icon('alert', 14)}<span>${esc(i.mc)} 的存档格式还没做模板，这个版本请进游戏里建。1.13 以后的版本都能在这里建。</span></p>`}
            <label class="field" for="nwName"><span>名字</span><input id="nwName" type="text" value="${esc(d.name)}" autocomplete="off"></label>
            <div class="field"><span>种子</span>
                <div class="inrow"><input id="nwSeed" type="text" class="mono grow" value="${esc(d.seed)}" placeholder="留空就随机" autocomplete="off"><button type="button" class="btn sm" data-act="nw-dice">${icon('refresh', 13)}随机一个</button></div></div>
            <div class="field"><span>游戏模式</span>${seg('nw-set', [['生存', '生存'], ['创造', '创造'], ['冒险', '冒险'], ['极限', '极限']], d.mode, 'data-k="mode"')}</div>
            <div class="field"><span>难度</span>${seg('nw-set', [['和平', '和平'], ['简单', '简单'], ['普通', '普通'], ['困难', '困难']], d.diff, 'data-k="diff"')}</div>
            <div class="field"><span>世界类型</span>${seg('nw-set', [['默认', '默认'], ['超平坦', '超平坦'], ['巨型生物群系', '巨型生物群系'], ['放大化', '放大化']], d.type, 'data-k="type"')}</div>
            <label class="check" for="nwCheats"><input type="checkbox" id="nwCheats"${d.cheats ? ' checked' : ''}><span><b>允许作弊</b><small>能用 /gamemode、/tp 这些命令</small></span></label>
            <label class="check" for="nwStruct"><input type="checkbox" id="nwStruct"${d.structs ? ' checked' : ''}><span><b>生成建筑</b><small>村庄、神殿、要塞……</small></span></label>
            <p class="note">这里只写好 level.dat（种子、模式、规则），地形在你第一次进去时由游戏生成。</p>
        </div>
        <footer class="dr-foot">
            <span class="hint">建好就能点「进入」</span>
            <button type="button" class="go" id="drCreate" data-act="dr-create"${ok && d.name.trim() ? '' : ' disabled'}>${icon('globe', 22)}<span class="txt"><span class="lbl">建好世界</span><span class="sub">${esc(d.mode)} · ${esc(d.diff)} · ${esc(d.type)}</span></span><kbd>Enter</kbd></button>
        </footer>
    </section>`;
}
