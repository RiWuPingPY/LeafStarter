// 外壳：左栏实例列表、账户、启动按钮。启动按钮在任何页面都在同一个位置
import { esc, block, head, clock } from '../ui.js';
import { icon } from '../icons.js';

const match = (i, q) => `${i.name} ${i.mc} ${i.loader || '原版'}`.toLowerCase().includes(q);
// 定制版预览时左栏只有定制包带来的实例
const pool = (c) => c.INSTANCES.filter((i) => !!i.distro === !!c.S.distOn);

// 左栏当前看得见的实例，按显示顺序（列表顺序不随游玩时间变，位置好记）
export function visibleIds(c) {
    const q = c.S.filter.trim().toLowerCase();
    return c.DIRS.flatMap((d) => pool(c).filter((i) => i.dir === d.id && (!q || match(i, q))).map((i) => i.id));
}

export function railHTML(c) {
    const q = c.S.filter.trim().toLowerCase();
    const groups = c.DIRS
        .map((d) => ({ d, list: pool(c).filter((i) => i.dir === d.id && (!q || match(i, q))) }))
        .filter((g) => g.list.length);
    if (!groups.length) return `<p class="rail-empty">没有叫「${esc(c.S.filter)}」的实例。按 <kbd>Esc</kbd> 清空。</p>`;
    // 选中的高亮是单独一层，换实例时它滑过去，列表项本身不动
    return '<i class="rail-sel" id="railSel" aria-hidden="true"></i>' + groups
        .map(({ d, list }) => `<div class="group" title="${esc(d.path)}"><span>${esc(d.name)}</span><span>${list.length}</span></div>` + list.map((i) => itemHTML(i, c)).join(''))
        .join('');
}

function itemHTML(i, c) {
    const sel = i.id === c.S.inst;
    return `<button type="button" class="item${sel ? ' sel' : ''}" data-act="sel" data-id="${i.id}" role="option" aria-selected="${sel}">
        ${block(i.block, 32)}
        <span class="tx"><span class="nm">${esc(i.name)}</span><span class="sb">${esc(i.mc)} · ${i.loader ? esc(i.loader) : '原版'}</span></span>
        <span class="st" data-st="${i.id}">${railStatus(i, c)}</span>
    </button>`;
}

export function railStatus(i, c) {
    const r = c.RUN.get(i.id);
    if (i.installing) return `<span class="ring" style="--p:${i.installing.pct}" title="安装中 ${i.installing.pct}%"><svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="10" r="8"/><circle class="fg" cx="10" cy="10" r="8"/></svg></span>`;
    if (r?.phase === 'running') return '<i class="dot run" title="运行中"></i>';
    if (r?.phase === 'crashed') return '<i class="dot bad" title="崩溃了"></i>';
    if (r) return '<i class="dot busy" title="正在启动"></i>';
    if (i.sync.on) return `<span class="syncmark" title="已开手机电脑同步">${icon('smartphone', 13)}</span>`;
    return '';
}

export function acctHTML(c) {
    const a = c.ACCOUNTS.find((x) => x.id === c.S.account) || c.ACCOUNTS[0];
    return `${head(a.face, 30)}<span class="tx"><span class="nm">${esc(a.name)}</span><span class="sb">${esc(a.label)}${c.real ? '' : ' · 默认账户'}</span></span>${icon('chevron', 14)}`;
}

// 启动按钮：状态变了才整块重画，进度和文字原地改，免得按下去的那一刻按钮被换掉
export function launchKey(c) {
    const i = c.inst;
    if (!i) return 'none';
    if (i.installing) return i.id + ':install';
    const r = c.RUN.get(i.id);
    return i.id + ':' + (!r ? 'idle' : r.phase === 'running' ? 'run' : r.phase === 'crashed' ? 'bad' : 'busy');
}

export function launchVals(c) {
    const i = c.inst;
    if (!i) return { lbl: '新建实例', sub: '还没有可以启动的实例' };
    if (i.installing) return { lbl: `安装中 ${i.installing.pct}%`, sub: i.installing.stage, p: i.installing.pct };
    const r = c.RUN.get(i.id);
    const a = c.account(i);
    if (!r && c.real) {
        // 自己启动游戏还在做：先用原来的启动器打开
        const d = c.DIRS.find((x) => x.id === i.dir);
        return d?.exe ? { lbl: `用 ${d.name} 打开`, sub: `在里面选「${i.name}」` } : { lbl: '启动还在做', sub: '先用原来的启动器玩' };
    }
    if (!r) return { lbl: '启动', sub: `${i.name} · ${a.name}` };
    switch (r.phase) {
        case 'account': return { lbl: '检查账户', sub: a.id === 'msa' ? '微软登录还有效' : '离线账户', p: 6 };
        case 'files': return { lbl: `检查文件 ${r.pct}%`, sub: r.missing ? `补上缺的 ${r.missing} 个文件` : '文件都在', p: 6 + Math.round(r.pct * 0.74) };
        case 'java': return { lbl: `启动 Java ${r.java}`, sub: i.name, p: 84 };
        case 'window': return { lbl: '等游戏窗口', sub: r.opts?.v ? `进去后直接到「${r.opts.v}」` : i.name, p: 95 };
        case 'running': return { lbl: '运行中', sub: `${clock(Math.floor((Date.now() - r.started) / 1000))} · 点这里看日志` };
        case 'crashed': return { lbl: '崩溃了', sub: '点这里看原因和修复' };
    }
    return { lbl: '', sub: '' };
}

export function launchHTML(c) {
    const kind = launchKey(c).split(':')[1] || 'none';
    const v = launchVals(c);
    const inner = (ic) => `<span class="orb">${icon(ic, 18)}</span><span class="txt"><span class="lbl">${esc(v.lbl)}</span><span class="sub">${esc(v.sub)}</span></span>`;
    switch (kind) {
        case 'idle':
            return `<button type="button" class="go" data-act="launch" aria-keyshortcuts="Enter">${inner('play')}<kbd>Enter</kbd></button>`;
        case 'busy':
            return `<button type="button" class="go busy" style="--p:${v.p}%" aria-busy="true"><span class="fill"></span>${inner('zap')}</button>`;
        case 'install':
            return `<button type="button" class="go busy" style="--p:${v.p}%" aria-busy="true"><span class="fill"></span>${inner('download')}</button>`;
        case 'run':
            return `<button type="button" class="stopbtn" data-act="stop" title="结束游戏" aria-label="结束游戏">${icon('stop', 16)}<span>结束</span></button>`
                + `<button type="button" class="go run" data-act="run-log"><span class="orb live">${icon('play', 18)}</span><span class="txt"><span class="lbl">${esc(v.lbl)}</span><span class="sub">${esc(v.sub)}</span></span></button>`;
        case 'bad':
            return `<button type="button" class="go bad" data-act="crash-view">${inner('alert')}</button>`;
        default:
            return `<button type="button" class="go" data-act="new-inst">${inner('plus')}</button>`;
    }
}
