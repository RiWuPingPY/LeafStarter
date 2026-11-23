// 插件页：已装 / 插件市场 / 定制版制作器；装插件前的权限确认；关于页。只出 HTML
import { esc, block } from '../ui.js';
import { icon } from '../icons.js';
import { OFFICIAL, MARKET, PERM, LAYER, TIER, distManifest } from '../plugins.js';

const chip = (cls, ic, text) => `<span class="chip ${cls}">${ic ? icon(ic, 12) : ''}${esc(text)}</span>`;
const seg = (act, list, cur) => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="${act}" data-v="${v}" class="${String(cur) === v ? 'on' : ''}" aria-pressed="${String(cur) === v}">${t}</button>`).join('')}</div>`;
const n0 = (n) => (n >= 10000 ? (n / 10000).toFixed(1) + ' 万' : n.toLocaleString('zh-CN'));
export const tierChip = (t) => `<span class="tier t-${t}">${icon(TIER[t][1], 12)}${TIER[t][0]}</span>`;
export const layerChip = (l) => `<span class="chip xs">${LAYER[l]}</span>`;
const permRow = (p) => {
    const [t, lv] = PERM[p.split(':')[0]] || [p, 'low'];
    const host = p.includes(':') && p.startsWith('net:') ? p.slice(4) : '';
    return `<li class="pv-${lv}">${icon(lv === 'high' ? 'alert' : lv === 'mid' ? 'globe' : 'eye', 14)}<span>${esc(host ? t.replace('{host}', host) : t)}</span>${lv === 'high' ? '<b>高危</b>' : ''}</li>`;
};

export function pluginsView(c) {
    const S = c.S;
    const tab = S.ptab;
    const n = Object.keys(S.plug.inst).length + OFFICIAL.length + (S.plug.dev ? 1 : 0);
    return `<div class="page" id="page">
        <div class="page-h"><h1>插件</h1><span class="chip">Everything is Plugins</span><span class="sp"></span>
            ${seg('ptab', [['installed', `已装 ${n}`], ['market', '插件市场'], ['dist', '定制版']], tab)}</div>
        ${S.safe ? `<div class="banner calm" role="status">${icon('shield', 20)}<div class="tx"><b>安全模式：这次没加载任何第三方插件</b><p>插件把启动器弄得打不开时用它：按住 Ctrl 打开 LeafStarter 就是安全模式。官方插件照常能用。</p></div><div class="actions"><button type="button" class="btn pri" data-act="safe-off">正常打开</button></div></div>` : ''}
        ${tab === 'market' ? market(c) : tab === 'dist' ? distBuilder(c) : installed(c)}
    </div>`;
}

// ---------- 已装 ----------
function installed(c) {
    const S = c.S;
    const P = S.plug;
    const revoked = Object.keys(P.revoked).filter((id) => P.inst[id]);
    const off = (id) => OFFICIAL.find((o) => o.id === id);
    const offRows = OFFICIAL.map((o) => {
        const pack = o.kind === 'pack';
        const has = pack ? S.ext[o.id] : !P.off[o.id];
        const busy = S.extBusy === o.id;
        const act = pack
            ? busy ? `<span class="bar w live" data-extp="${o.id}"><i style="--p:${S.extPct || 0}%"></i></span><span class="mono sb" data-extt="${o.id}">${S.extStage} ${S.extPct}%</span>`
                : has ? `<button type="button" class="btn sm ghost" data-act="ext-del" data-v="${o.id}">移除</button>` : `<button type="button" class="btn sm" data-act="ext-add" data-v="${o.id}">${icon('download', 13)}${o.ondemand ? '下载' : '装上'}（${o.size}）</button>`
            : `<input type="checkbox" class="switch" data-poff="${o.id}"${has ? ' checked' : ''} aria-label="启用${o.name}">`;
        return `<div class="prow${has ? '' : ' off'}"><span class="ptile official">${icon(o.ic, 17)}</span>
            <span class="tx"><span class="nm">${o.name}${tierChip('official')}${pack ? layerChip('native') : '<span class="chip xs">内置 · 能关</span>'}</span><span class="sb wrap">${o.desc}${pack ? ` · ${o.size}` : ''}</span></span>
            <span class="tags">${act}</span></div>`;
    }).join('');
    const mine = S.plug.dev ? devRow(S) : '';
    const third = Object.entries(P.inst).map(([id, st]) => {
        const p = MARKET.find((x) => x.id === id);
        if (!p) return '';
        const rv = P.revoked[id];
        const live = st.on && !rv && !S.safe;
        return `<div class="prow${live ? '' : ' off'}${rv ? ' revoked' : ''}" data-row="p-${id}"><span class="ptile" style="--h:${p.h}">${esc(p.name[0])}</span>
            <span class="tx"><span class="nm">${esc(p.name)}${tierChip(rv ? 'revoked' : p.tier)}${layerChip(p.layer)}<span class="mono sb">${esc(st.ver || p.ver)}</span></span>
                <span class="sb wrap">${rv ? `<b class="badt">已被插件市场吊销，自动停用了：</b>${esc(rv)}` : `${esc(p.by)} · ${esc(p.short || p.desc)}`}</span>
                ${p.adds ? `<span class="sb wrap adds">${icon('plug', 12)}${esc(p.adds)}</span>` : ''}
                <span class="perms">${p.perms.length ? p.perms.map((x) => `<span class="chip xs pv-${(PERM[x.split(':')[0]] || [, 'low'])[1]}">${esc((PERM[x.split(':')[0]] || [x])[0].replace('{host}', x.slice(4)).replace(/（.*）/, ''))}</span>`).join('') : '<span class="chip xs">不跑代码，不要权限</span>'}</span></span>
            <span class="tags">${rv ? `<button type="button" class="btn sm ghost danger" data-act="plug-del" data-v="${id}">卸载</button>` : `<input type="checkbox" class="switch" data-pon="${id}"${st.on ? ' checked' : ''}${S.safe ? ' disabled' : ''} aria-label="启用${esc(p.name)}"><button type="button" class="iconbtn" data-act="plug-del" data-v="${id}" aria-label="卸载${esc(p.name)}" title="卸载">${icon('trash', 15)}</button>`}</span></div>`;
    }).join('');
    return `${revoked.length ? `<div class="banner" role="alert">${icon('alert', 20)}<div class="tx"><b>${revoked.length} 个插件被吊销了，已经自动停用</b><p>插件市场发现它们有问题，会通过吊销名单通知每个启动器。它们拿不到你的登录令牌和密钥（插件接口里本来就没有），建议直接卸载。</p></div></div>` : ''}
    <div class="toolbar">
        <button type="button" class="btn" data-act="plug-check"${S.plugChecking ? ' disabled' : ''}>${icon('refresh')}${S.plugChecking ? '正在检查…' : '检查更新和吊销名单'}</button>
        <button type="button" class="btn" data-act="safe-on"${S.safe ? ' disabled' : ''}>${icon('shield')}以安全模式重新打开</button>
        <span class="sp"></span>
        <label class="tick"><input type="checkbox" id="devMode"${S.plug.dev ? ' checked' : ''}>开发者模式</label>
    </div>
    <h3 class="sh">${icon('check', 14)}官方插件 · 功能包也在这里</h3>
    <p class="note">「一切皆插件」先用在自己身上：这些官方功能和第三方插件用同一套接口、同一套权限。用不着的关掉，界面上的入口就跟着消失。</p>
    <div class="plist">${offRows}</div>
    <h3 class="sh">${icon('plug', 14)}第三方插件</h3>
    <div class="plist">${mine}${third || '<div class="prow empty">还没装第三方插件。去「插件市场」看看。</div>'}</div>
    <p class="note">任何插件都拿不到：账户登录令牌、保管库里的密钥、我们的 CurseForge Key。插件崩了只停用它自己。</p>`;
}

function devRow(S) {
    const d = S.devp;
    return `<div class="prow${d.err ? ' off revoked' : ''}"><span class="ptile dev">${icon('terminal', 16)}</span>
        <span class="tx"><span class="nm">我的第一个插件${tierChip('dev')}${layerChip('script')}<span class="mono sb">0.0.${d.n}</span></span>
            <span class="sb wrap">${d.err ? `<b class="badt">出错了，已停用：</b><span class="mono">${esc(d.err)}</span>` : '从 D:\\code\\my-plugin 加载 · 改了代码自动热重载'}</span></span>
        <span class="tags">${d.err ? '<button type="button" class="btn sm" data-act="dev-fix">改好了，重新启用</button>' : '<button type="button" class="btn sm" data-act="dev-reload">热重载</button><button type="button" class="btn sm ghost" data-act="dev-crash">让它出错</button>'}</span></div>`;
}

// ---------- 市场 ----------
function market(c) {
    const S = c.S;
    const q = S.pq.trim().toLowerCase();
    const list = MARKET.filter((p) => !p.hidden && (S.pcat === 'all' || p.layer === S.pcat) && (!q || `${p.name} ${p.desc} ${p.by}`.toLowerCase().includes(q)));
    return `<div class="toolbar">
        <label class="filter grow" for="plugQ">${icon('search', 14)}<input id="plugQ" type="text" placeholder="搜插件、主题、崩溃规则……" value="${esc(S.pq)}" autocomplete="off"></label>
        ${seg('pcat', [['all', '全部'], ['look', '外观'], ['decl', '声明式扩展'], ['script', '脚本插件'], ['native', '原生插件']], S.pcat)}
    </div>
    <p class="note">示例插件。市场的索引放在 GitHub 上，只存插件信息、签名和 SHA-256；文件由作者自己托管（GitHub Releases、Gitee、蓝奏云都行），下完对得上哈希才装。</p>
    <div class="pgrid" id="plugGrid">${list.map((p) => {
        const st = S.plug.inst[p.id];
        return `<article class="pcard"><div class="ph-row"><span class="ptile lg" style="--h:${p.h}">${esc(p.name[0])}</span>
            <span class="tx"><span class="nm">${esc(p.name)}</span><span class="sb">${esc(p.by)}</span></span></div>
            <p>${esc(p.desc)}</p>
            <div class="pmeta">${tierChip(p.tier)}${layerChip(p.layer)}<span class="sb">${n0(p.dl)} 次安装 · ${p.size}</span></div>
            <div class="actions">${st ? `<button type="button" class="btn sm" disabled>${icon('check', 13)}已装</button>` : `<button type="button" class="btn sm pri" data-act="plug-get" data-v="${p.id}">${icon('download', 13)}装上</button>`}</div>
        </article>`;
    }).join('') || '<p class="note">没找到。</p>'}</div>`;
}

// ---------- 装插件前：权限确认（右侧面板） ----------
export function installDrawer(c) {
    const d = c.S.drawer;
    const p = MARKET.find((x) => x.id === d.id);
    const high = p.perms.some((x) => (PERM[x.split(':')[0]] || [])[1] === 'high');
    const ok = !high || d.ack;
    return `<section class="drawer" role="dialog" aria-label="装上插件">
        <header class="dr-head"><h2>装上「${esc(p.name)}」</h2><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
        <div class="dr-body">
            <div class="ph-row"><span class="ptile lg" style="--h:${p.h}">${esc(p.name[0])}</span><span class="tx"><span class="nm">${esc(p.name)} <span class="mono sb">${p.ver}</span></span><span class="sb">${esc(p.by)}</span></span></div>
            <div class="pmeta">${tierChip(p.tier)}${layerChip(p.layer)}</div>
            <p class="note">${esc(p.desc)}</p>
            ${p.tier === 'community' ? `<p class="callout warn">${icon('alert', 14)}<span>社区插件：只过了自动扫描，没人看过它的代码。只装你信得过的作者的。</span></p>` : ''}
            <div class="field"><span>它要的权限</span>
                ${p.perms.length ? `<ul class="perm-list">${p.perms.map(permRow).join('')}</ul>` : `<p class="callout">${icon('check', 14)}<span>${p.layer === 'look' ? '外观插件只是一包颜色、字体和图片，不跑代码，什么权限都不要。' : '纯数据，不跑代码，什么权限都不要。'}</span></p>`}</div>
            ${high ? `<p class="callout bad">${icon('alert', 14)}<span><b>高危权限</b>：往游戏里加 javaagent，等于在游戏进程里运行它的代码，而游戏进程拿着你的登录令牌。只给你非常信任的、认证过的开发者。</span></p>
                <label class="check" for="pAck"><input type="checkbox" id="pAck"${d.ack ? ' checked' : ''}><span><b>我知道它能在游戏里运行代码</b><small>以后它更新时如果要更多权限，会再问你</small></span></label>` : ''}
            <div class="field"><span>它永远拿不到</span>
                <ul class="perm-list never"><li>${icon('x', 14)}<span>账户登录令牌、保管库里的密钥（插件接口里根本没有）</span></li><li>${icon('x', 14)}<span>别的插件的数据；也不能悄悄装别的插件</span></li></ul></div>
            <dl class="facts">
                <dt>下载</dt><dd>${esc(p.host)}</dd>
                <dt>签名</dt><dd>作者 ${icon('check', 12)} · 插件市场 ${icon('check', 12)}</dd>
                <dt>校验</dt><dd class="mono">SHA-256 ${p.sha}…（下完再对一遍）</dd>
            </dl>
        </div>
        <footer class="dr-foot">
            <span class="hint">装完立刻生效，不用重启</span>
            <button type="button" class="go" id="drCreate" data-act="dr-create"${ok ? '' : ' disabled'}>${icon('download', 22)}<span class="txt"><span class="lbl">装上</span><span class="sub">${p.size} · ${p.perms.length ? `${p.perms.length} 项权限` : '不要权限'}</span></span><kbd>Enter</kbd></button>
        </footer>
    </section>`;
}

// ---------- 定制版制作器 ----------
const BLOCKS = ['diamond', 'grass', 'brass', 'nether', 'copper', 'stone'];
const COLORS = ['#0E7C86', '#2F7D5B', '#C2410C', '#BE123C', '#7C3AED', '#1D4ED8'];
const KEEP = [['download', '下载页'], ['mp', '联机'], ['ai', 'AI 助手'], ['skin', '皮肤工作台'], ['world', '世界工具'], ['sync', '手机电脑同步']];

function distBuilder(c) {
    const D = c.S.dist;
    const packs = c.CATALOG.pack;
    return `<p class="note">给服主和整合包作者：不写代码，做一个「自家专用启动器」。导出的是一个签名的定制包，放在 LeafStarter.exe 旁边就生效；程序还是我们签名的那一个，不用重新编译，杀毒软件不误报，主程序照样自动更新。</p>
    <div class="dist-grid">
        <section class="card">
            <h3><span class="ico violet">${icon('package', 16)}</span>定制包</h3>
            <label class="field" for="dName"><span>启动器名字</span><input id="dName" type="text" value="${esc(D.name)}" autocomplete="off"></label>
            <div class="field"><span>图标</span><div class="picks-row">${BLOCKS.map((b) => `<button type="button" class="pick-blk${D.icon === b ? ' on' : ''}" data-act="d-icon" data-v="${b}" aria-label="${b}">${block(b, 28)}</button>`).join('')}</div></div>
            <div class="field"><span>主题色</span><div class="picks-row">${COLORS.map((col) => `<button type="button" class="swatch${D.color === col ? ' on' : ''}" style="--c:${col}" data-act="d-color" data-v="${col}" aria-label="${col}"></button>`).join('')}</div></div>
            <div class="field"><span>字体</span>${seg('d-font', [['sans', '默认黑体'], ['serif', '衬线体']], D.font)}</div>
            <label class="field" for="dPack"><span>预装的整合包</span><select id="dPack">${packs.map((p) => `<option value="${p.id}"${D.pack === p.id ? ' selected' : ''}>${esc(p.name)}（${esc(p.mc)} · ${esc(p.loader)}）</option>`).join('')}</select></label>
            <div class="field"><span>服务器</span><div class="inrow"><input id="dSrvName" type="text" value="${esc(D.srv)}" placeholder="名字" autocomplete="off"><input id="dSrvAddr" type="text" class="mono" value="${esc(D.addr)}" placeholder="地址" autocomplete="off"></div></div>
            <label class="field" for="dNotice"><span>主页公告</span><textarea id="dNotice" rows="2">${esc(D.notice)}</textarea></label>
            <div class="field"><span>留下哪些功能</span><div class="picks keep">${KEEP.map(([k, t]) => `<label class="tick pk"><input type="checkbox" data-dkeep="${k}"${D.keep[k] ? ' checked' : ''}><span class="tx"><span class="nm">${t}</span></span></label>`).join('')}</div></div>
            <label class="check" for="dMin"><input type="checkbox" id="dMin"${D.minimal ? ' checked' : ''}><span><b>极简：打开就一个启动按钮</b><small>没有左栏、没有标签页，只有公告、服务器和启动</small></span></label>
            <label class="field" for="dAuthor"><span>作者（显示在「关于」里）</span><input id="dAuthor" type="text" value="${esc(D.author)}" autocomplete="off"></label>
        </section>
        <section class="card dist-side">
            <h3><span class="ico sky">${icon('eye', 16)}</span>预览</h3>
            <div id="distMock">${distMock(c)}</div>
            <div class="actions"><button type="button" class="btn pri" data-act="dist-try">${icon('play')}换成这个定制版看看</button><button type="button" class="btn" data-act="dist-export">${icon('file')}导出定制包</button></div>
            <ul class="ag-points">
                <li>${icon('shield', 14)}<span>定制包要用作者的密钥签名；「关于」里显示作者和签名</span></li>
                <li>${icon('check', 14)}<span>「基于 LeafStarter」由程序固定显示，定制包里关不掉</span></li>
                <li>${icon('refresh', 14)}<span>主程序跟官方更新；定制包跟作者填的地址更新</span></li>
            </ul>
        </section>
    </div>`;
}

export function distMock(c) {
    const D = c.S.dist;
    const pack = c.CATALOG.pack.find((p) => p.id === D.pack);
    const nav = [['实例', true], ['下载', D.keep.download], ['任务', !D.minimal], ['设置', !D.minimal]].filter(([, v]) => v).map(([t]) => `<i>${t}</i>`).join('');
    return `<div class="mock${D.font === 'serif' ? ' serif' : ''}" style="--mc:${D.color}">
        <div class="mk-top">${block(D.icon, 14)}<b>${esc(D.name || '没起名字')}</b><span class="sp"></span>${nav}</div>
        <div class="mk-body">
            ${D.minimal ? '' : `<div class="mk-rail"><span class="on">${block(D.icon, 12)}${esc(pack?.name || '')}</span></div>`}
            <div class="mk-main">
                <div class="mk-hero" style="--mc:${D.color}"><b>${esc(pack?.name || '')}</b><small>${esc(pack?.mc || '')} · ${esc(pack?.loader || '')}</small></div>
                ${D.notice ? `<div class="mk-card"><b>公告</b>${esc(D.notice)}</div>` : ''}
                <div class="mk-card"><b>${esc(D.srv || '服务器')}</b><span class="mono">${esc(D.addr)}</span></div>
                <div class="mk-go">${icon('play', 12)}启动</div>
            </div>
        </div>
        <div class="mk-foot">基于 LeafStarter</div>
    </div>`;
}

export function distJSON(c) {
    return JSON.stringify(distManifest(c.S.dist, c.CATALOG.pack.find((p) => p.id === c.S.dist.pack)), null, 2);
}

// ---------- 关于 ----------
export function aboutDrawer(c) {
    const S = c.S;
    const D = S.distOn ? S.dist : null;
    const deps = [['MIT 或 Apache-2.0', 238], ['MIT', 62], ['Unicode-3.0', 19], ['Unlicense 或 MIT', 10], ['BSD-3-Clause', 7], ['MPL-2.0', 5], ['Zlib', 4], ['ISC', 4], ['其他宽松许可', 7]];
    return `<section class="drawer" role="dialog" aria-label="关于">
        <header class="dr-head"><h2>关于</h2><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
        <div class="dr-body">
            ${D ? `<div class="about-h">${block(D.icon, 44)}<span class="tx"><b>${esc(D.name)}</b><small>定制包作者：${esc(D.author)} · 签名 ${icon('check', 12)}</small></span></div>
                <div class="based">${'<i class="leaf" aria-hidden="true"></i>'}<span><b>基于 LeafStarter 0.1.0</b><small>这一行由程序固定显示，定制包里关不掉</small></span></div>` : `<div class="about-h"><i class="leaf big" aria-hidden="true"></i><span class="tx"><b>LeafStarter</b><small>0.1.0 · 交互原型</small></span></div>`}
            <dl class="facts">
                <dt>许可</dt><dd>Apache-2.0 · Copyright 2026 PingYi</dd>
                <dt>源码</dt><dd class="mono">github.com/RiWuPingPY/LeafStarter</dd>
                <dt>名称与署名</dt><dd>改源码另发请换名字，写明「基于 LeafStarter」（TRADEMARKS.md）</dd>
            </dl>
            <div class="field"><span>第三方作品</span>
                <div class="mini-rows"><div class="mrow"><span class="tx"><span class="nm">Lucide 图标</span><span class="sb">ISC；从 Feather 继承的图标为 MIT</span></span></div>
                <div class="mrow"><span class="tx"><span class="nm">Rust 依赖 356 个</span><span class="sb wrap">${deps.map(([k, v]) => `${k} ${v}`).join(' · ')}</span></span></div></div>
                <small class="note">示例数字。正式版由 cargo-about 在打包时生成完整清单，放进安装目录。</small></div>
            <p class="note">LeafStarter${D ? ` 和「${esc(D.name)}」` : ''}不是 Minecraft 官方产品，与 Mojang 和 Microsoft 无关。</p>
        </div>
        <footer class="dr-foot"><span class="hint">${S.safe ? '安全模式' : `已启用 ${c.plugCount()} 个第三方插件`}</span></footer>
    </section>`;
}
