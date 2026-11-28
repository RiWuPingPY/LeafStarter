// LeafStarter 原型：状态、绘制、快捷键，以及模拟的启动、安装、同步
import { DIRS, INSTANCES, ACCOUNTS, JAVAS, GLOBAL, CATALOG, VERSIONS, TASKS, PC, perfMods, newInstance, loaderVersion, bootLog, crashLog, conflictLog, conflictCrash, makeMod } from './data.js';
import { $, $$, esc, store, toast, undoLast, ago, stamp, clock, block, head, ask, askText } from './ui.js';
import { icon, hydrate } from './icons.js';
import { railHTML, railStatus, acctHTML, launchHTML, launchKey, launchVals, visibleIds } from './views/shell.js';
import { instanceView, tabBody, tabsHTML, statusChips, modRows, modCount, logText } from './views/instance.js';
import { serverRows, hostDrawer, importDrawer } from './views/multiplayer.js';
import { paintScene, paintClouds } from './scene.js';
import { burst } from './fx.js';
import { downloadView, dlList, dlTarget, targetBar, tasksView, taskRows, settingsView, drawerView, vlist } from './views/pages.js';
import { createMp } from './mp.js';
import { createFix } from './fix.js';
import { createAgent } from './agent.js';
import { createAgentReal, createAiSettings } from './agent-real.js';
import { createServers } from './server-real.js';
import { createWorldTool, worldToolDrawer } from './worldtool-real.js';
import { createEdition, applyEdition, edMakerDrawer } from './edition-real.js';
import { createPluginHost } from './plugins-real.js';
import { createMp as createMpReal, mpJoinDrawer } from './mp-real.js';
import { createUpdate, realAboutDrawer, aboutInput } from './update-real.js';
import { realPluginsView, plugConsentDrawer } from './views/plugins-real.js';
import { srvAddDrawer, panelAddDrawer, srvConsoleDrawer, srvCfgDrawer } from './views/server-real.js';
import { createWorld } from './world.js';
import { createSkin } from './skin.js';
import { createTools } from './tools.js';
import { createAiSrc } from './aisrc.js';
import { createPlugins } from './plugins.js';
import { pluginsView, installDrawer, aboutDrawer, distMock, distJSON } from './views/plugins.js';
import { skinView } from './views/skin.js';
import { i18nDrawer, tmplDrawer, worldDrawer } from './views/tools.js';
import * as real from './real.js';
import { REAL } from './real.js';
import { realDlRows, realAcctPop, realMsaDrawer, realOffDrawer, realYggDrawer, modLinksPop, worldHistPop, lanCodePop, realLanDrawer, realResDrawer, resTarget, gbOf } from './views/real.js';

// 在 LeafStarter 里运行时，先把后端的真数据（游戏目录、版本、Java、任务）装进来，再画第一屏。
// 浏览器里直接打开（线上原型）时 REAL 是 false，界面照旧用 data.js 的示例数据
if (REAL) await real.boot();

// ---------- 状态 ----------
const PAGES = ['home', 'download', 'tasks', 'plugins', 'settings', 'skin'];
const S = {
    page: store.get('page', 'home'),
    inst: store.get('inst', 'survival'),
    tab: store.get('tab', 'overview'),
    account: store.get('account', 'msa'),
    theme: store.get('theme', 'system'),
    direct: store.get('direct', false),
    after: store.get('after', 'hide'),
    src: store.get('src', 'auto'),
    motion: store.get('motion', 'full'),
    mpMode: store.get('mpMode', 'now'),
    // AI 源在 aisrc.js 里；Key 本身原型里不存，只记「填没填过」（正式版进加密保管库）
    aiCap: store.get('aiCap', '10w'),
    aiPreview: store.get('aiPreview', true),
    mcp: store.get('mcp', false),
    mcpRev: store.get('mcpRev', true),
    relay: store.get('relay', 'public'),
    sakura: store.get('sakura', false) === true,
    filter: '',
    modQ: '',
    modFilter: 'all',
    logLevel: 'all',
    dl: { type: 'mod', q: '', target: null, loading: false, installed: {} },
    dlv: 'release',
    dlvq: '',
    drawer: null,
};
if (!PAGES.includes(S.page)) S.page = 'home';
if (!INSTANCES.some((i) => i.id === S.inst)) S.inst = INSTANCES[0]?.id ?? null;
const SAVED = ['page', 'inst', 'tab', 'account', 'theme', 'direct', 'after', 'src', 'motion', 'mpMode', 'aiCap', 'aiPreview', 'mcp', 'mcpRev', 'relay', 'sakura', 'simple', 'console'];
const save = () => SAVED.forEach((k) => store.set(k, S[k]));
// 真程序里 AI 源在后端（设置的 ai 部分），这里只看有没有
const realAiFor = (u) => {
    const a = real.R.ai;
    if (!a) return null;
    return a.sources.find((s) => s.id === a.uses[u]) || a.sources[0] || null;
};
const aiReady = () => (REAL ? !!realAiFor('chat') : !!aisrc.aiFor('chat'));
const aiFor = (u) => (REAL ? realAiFor(u) : aisrc.aiFor(u));

const RUN = new Map(); // 实例 id → 正在启动 / 运行 / 崩溃的状态
const byId = (id) => INSTANCES.find((i) => i.id === id);
const cur = () => byId(S.inst);
const account = (i) => {
    const id = i?.cfg.account && i.cfg.account !== 'default' ? i.cfg.account : S.account;
    return ACCOUNTS.find((a) => a.id === id) || ACCOUNTS[0];
};
const eff = (i, k) => (k in i.cfg ? i.cfg[k] : GLOBAL[k]);
const javaMajor = (i) => {
    const v = eff(i, 'java');
    return v === 'auto' ? i.javaNeed : JAVAS.find((j) => j.id === v)?.major ?? i.javaNeed;
};
const isolated = (i) => ('isolate' in i.cfg ? i.cfg.isolate : !!i.loader);
const ctx = () => ({ S, RUN, TASKS, DIRS, INSTANCES, ACCOUNTS, JAVAS, GLOBAL, CATALOG, VERSIONS, PC, inst: cur(), byId, account, eff, isolate: isolated, aiReady, aiFor, wmeta: world.meta, structs: world.structs, wdirty: world.wdirty, plugOn: plugins.on, plugOff: plugins.off, plugCount: plugins.count, real: REAL, R: real.R, loaderInfo: real.loaderInfo });

// ---------- 重画时保住焦点和光标：换掉的元素如果正被聚焦，画完把焦点还给新的那个 ----------
function focusSel(a) {
    if (!a || a === document.body) return null;
    if (a.id) return '#' + CSS.escape(a.id);
    const ds = a.dataset || {};
    if (!ds.act && !ds.page) return null;
    let s = ds.act ? `[data-act="${CSS.escape(ds.act)}"]` : `[data-page="${CSS.escape(ds.page)}"]`;
    for (const k of ['v', 'id', 'tab', 'k']) if (ds[k] != null) s += `[data-${k}="${CSS.escape(ds[k])}"]`;
    return s;
}
function withFocus(fn) {
    const a = document.activeElement;
    const sel = focusSel(a);
    const range = a && typeof a.selectionStart === 'number' ? [a.selectionStart, a.selectionEnd] : null;
    fn();
    if (!sel || (a.isConnected && document.activeElement === a)) return;
    const el = document.querySelector(sel);
    if (!el) return;
    el.focus({ preventScroll: true });
    if (range && el.setSelectionRange) {
        try { el.setSelectionRange(...range); } catch {}
    }
}

// ---------- 绘制 ----------
function paintTop() {
    $$('#pages [data-page]').forEach((b) => {
        const on = b.dataset.page === S.page;
        b.classList.toggle('on', on);
        if (on) b.setAttribute('aria-current', 'page');
        else b.removeAttribute('aria-current');
    });
    placeInd($('#navInd'), $(`#pages [data-page="${S.page}"]`), true);
    const n = TASKS.filter((t) => t.state === 'run').length;
    const badge = $('#taskBadge');
    badge.hidden = !n;
    badge.textContent = n;
    const ai = $('#aiBtn');
    if (ai) ai.hidden = plugins.off('ai');
}

// 指示条、左栏高亮：目标不动，它们滑过去。animate 为 false 时直接到位（首次绘制、改窗口大小）
function placeInd(ind, target, animate) {
    if (!ind) return;
    if (!target) {
        ind.style.opacity = 0;
        return;
    }
    if (!animate) ind.classList.add('still');
    ind.style.opacity = 1;
    ind.style.width = `${target.offsetWidth}px`;
    ind.style.transform = `translateX(${target.offsetLeft}px)`;
    if (!animate) {
        void ind.offsetWidth;
        ind.classList.remove('still');
    }
}

function placeRailSel(animate) {
    const sel = $('#railSel');
    if (!sel) return;
    const it = $('#railList .item.sel');
    if (!it) {
        sel.style.opacity = 0;
        return;
    }
    if (!animate) sel.classList.add('still');
    sel.style.opacity = 1;
    sel.style.height = `${it.offsetHeight}px`;
    sel.style.transform = `translateY(${it.offsetTop}px)`;
    if (!animate) {
        void sel.offsetWidth;
        sel.classList.remove('still');
    }
}

// 风景：头图（带站在地上的玩家）和截图缩略图
function paintScenes(heroToo = true) {
    const i = cur();
    const hero = $('#heroScene');
    if (heroToo && hero && i) {
        paintScene(hero, i.block, i.id, { block: 12, face: account(i).face });
        const cl = $('#heroClouds');
        if (cl) paintClouds(cl, i.block, i.id, 12);
    }
    $$('#view canvas[data-scene]').forEach((cv) => paintScene(cv, cv.dataset.scene, cv.dataset.seed, { block: 7 }));
    world.after();
    skin.after();
}

function paintRail() {
    const list = $('#railList');
    const top = list.scrollTop;
    withFocus(() => { list.innerHTML = railHTML(ctx()); });
    list.scrollTop = top;
    placeRailSel(false);
}

function markSelected() {
    $$('#railList .item').forEach((b) => {
        const on = b.dataset.id === S.inst;
        b.classList.toggle('sel', on);
        b.setAttribute('aria-selected', on);
    });
    placeRailSel(true);
}

const ensureVisible = (id) => $(`#railList .item[data-id="${CSS.escape(id)}"]`)?.scrollIntoView({ block: 'nearest' });

function paintAcct() {
    $('#acct').innerHTML = acctHTML(ctx());
}

let lastLaunch = '';
function paintLaunch(force) {
    const c = ctx();
    const key = launchKey(c);
    const el = $('#launch');
    if (force || key !== lastLaunch) {
        withFocus(() => { el.innerHTML = launchHTML(c); });
        lastLaunch = key;
        return;
    }
    const go = el.querySelector('.go');
    if (!go) return;
    const v = launchVals(c);
    go.querySelector('.lbl').textContent = v.lbl;
    go.querySelector('.sub').textContent = v.sub;
    if (v.p != null) go.style.setProperty('--p', v.p + '%');
}

const scrolls = new Map();
let lastScrollKey = null;
const scroller = () => $('#view .body') || $('#view .page');
const scrollKey = () => (S.page === 'home' ? `home:${S.inst}:${S.tab}` : S.page);
// anim：切页、换实例时给内容一个进场；数据变了原地重画时不播
function paintView(anim) {
    const sc = scroller();
    if (sc && lastScrollKey) scrolls.set(lastScrollKey, sc.scrollTop);
    const c = ctx();
    const v = $('#view');
    v.classList.remove('enter', 'enter-body');
    withFocus(() => {
        v.innerHTML = S.page === 'home' ? instanceView(c) : S.page === 'download' ? downloadView(c) : S.page === 'tasks' ? tasksView(c) : S.page === 'skin' ? skinView(c) : S.page === 'plugins' ? (REAL ? realPluginsView(c) : pluginsView(c)) : settingsView(c);
    });
    if (anim) {
        void v.offsetWidth;
        v.classList.add('enter');
    }
    paintScenes();
    placeInd($('#tabInd'), $('#tabs .on'), false);
    const k = scrollKey();
    const sc2 = scroller();
    if (sc2) sc2.scrollTop = scrolls.get(k) || 0;
    lastScrollKey = k;
    const box = $('#logBox');
    if (box) box.scrollTop = box.scrollHeight;
}

// 只换实例页标签下面那块（联机、二分这些会频繁变的东西），头图和标签栏不动
function paintBody() {
    const body = $('#body');
    if (S.page !== 'home' || !body) return;
    const top = body.scrollTop;
    withFocus(() => { body.innerHTML = tabBody(ctx()); });
    body.scrollTop = top;
    paintScenes(false);
}

// 标签上的数字、小圆点（房间开着、正在开服）
function paintTabs() {
    const t = $('#tabs');
    if (S.page !== 'home' || !t || !cur()) return;
    withFocus(() => { t.innerHTML = tabsHTML(ctx()) + '<i class="ind" id="tabInd" aria-hidden="true"></i>'; });
    placeInd($('#tabInd'), $('#tabs .on'), false);
}

const DRAWERS = { host: hostDrawer, import: importDrawer, i18n: i18nDrawer, tmpl: tmplDrawer, world: worldDrawer, pinst: installDrawer, about: aboutDrawer, msa: realMsaDrawer, off: realOffDrawer, ygg: realYggDrawer, lan: realLanDrawer, res: realResDrawer };
function paintDrawer() {
    const c = ctx();
    const k = S.drawer?.kind;
    // 面板已经开着、只是内容变了：不再播一遍滑入
    const had = $('#drawer .drawer:not(.out)');
    const body = had?.querySelector('.dr-body');
    const top = body?.scrollTop || 0;
    withFocus(() => { $('#drawer').innerHTML = (DRAWERS[k] || drawerView)(c); });
    if (had) {
        $('#drawer .drawer')?.classList.add('still');
        const nb = $('#drawer .dr-body');
        if (nb) nb.scrollTop = top;
    }
}

function paintTasks() {
    if (S.page !== 'tasks') return;
    const el = $('#taskList');
    if (el) el.innerHTML = taskRows(ctx());
}

function paintDl() {
    if (S.page !== 'download') return;
    const c = ctx();
    const l = $('#dlList');
    if (l) l.innerHTML = dlList(c);
    const bar = $('#dlTargetBar');
    if (bar && !bar.contains(document.activeElement)) bar.innerHTML = targetBar(c);
}

// 某个实例的状态变了：左栏小标记、启动按钮、头部状态原地改；结构变了（开始运行、崩溃、结束）才重画整页
function refresh(id, structural) {
    const i = byId(id);
    if (!i) return;
    const c = ctx();
    const st = $(`[data-st="${CSS.escape(id)}"]`);
    const ring = st?.querySelector('.ring');
    if (ring && i.installing) {
        ring.style.setProperty('--p', i.installing.pct);
        ring.title = `安装中 ${i.installing.pct}%`;
    } else if (st) st.innerHTML = railStatus(i, c);
    if (id !== S.inst) return;
    paintLaunch();
    const hs = $('#hstat');
    if (hs) hs.innerHTML = statusChips(i, c);
    if (structural && S.page === 'home') paintView();
}

// ---------- 导航 ----------
function goPage(p) {
    if (!PAGES.includes(p)) return;
    S.page = p;
    save();
    toggleRail(false);
    paintTop();
    paintView('page');
    // 下载页默认就是 Mod 搜索（照概念版）：第一次进来先把热门的列出来
    if (REAL && p === 'download') {
        const r = real.R.res;
        if (r.kind !== 'game' && !r.hits.length && !r.loading && !r.error) resSearch();
    }
}

function setTab(t) {
    const stay = S.page === 'home' && $('#tabs');
    S.tab = t;
    if (REAL && t === 'servers') servers?.enter();
    S.page = 'home';
    save();
    paintTop();
    if (!stay) return paintView('page');
    // 标签栏不重画：下划线滑过去，只换下面的内容
    $$('#tabs [data-tab]').forEach((b) => {
        const on = b.dataset.tab === t;
        b.classList.toggle('on', on);
        b.setAttribute('aria-selected', on);
    });
    placeInd($('#tabInd'), $('#tabs .on'), true);
    const sc = scroller();
    if (sc && lastScrollKey) scrolls.set(lastScrollKey, sc.scrollTop);
    const v = $('#view');
    v.classList.remove('enter', 'enter-body');
    withFocus(() => { $('#body').innerHTML = tabBody(ctx()); });
    void v.offsetWidth;
    v.classList.add('enter-body');
    paintScenes(false);
    lastScrollKey = scrollKey();
    $('#body').scrollTop = scrolls.get(lastScrollKey) || 0;
    const box = $('#logBox');
    if (box) box.scrollTop = box.scrollHeight;
}

// 只换选中的实例，不换页面（下载页里「装到」会跟着变）
function select(id) {
    if (!id || id === S.inst) return;
    S.inst = id;
    S.modQ = '';
    world.reset();
    save();
    markSelected();
    if (REAL && S.tab === 'servers') servers?.enter();
    if (S.page === 'home') paintView('inst');
    else if (S.page === 'download' && !S.dl.target) paintDl();
    paintLaunch();
    ensureDetails(cur());
}

function selectGo(id) {
    select(id);
    if (S.page !== 'home') goPage('home');
    toggleRail(false);
    ensureVisible(id);
}

function moveSel(delta, moveFocus) {
    const ids = visibleIds(ctx());
    if (!ids.length) return;
    const k = ids.indexOf(S.inst);
    const next = ids[k < 0 ? 0 : Math.max(0, Math.min(ids.length - 1, k + delta))];
    select(next);
    ensureVisible(next);
    if (moveFocus) $(`#railList .item[data-id="${CSS.escape(next)}"]`)?.focus();
}

function toggleRail(force) {
    $('#app').classList.toggle('rail-open', force ?? !$('#app').classList.contains('rail-open'));
}

// 启动按钮的「主动作」：空闲时启动，运行中看日志，崩溃了看原因
function primary() {
    const go = $('#launch .go');
    if (go?.dataset.act) go.click();
}

// ---------- 任务 ----------
function addTask(title, sub) {
    const t = { id: 't' + Date.now() + Math.random().toString(36).slice(2, 6), title, sub, state: 'run', pct: 0, at: Date.now() };
    TASKS.unshift(t);
    paintTop();
    paintTasks();
    return t;
}

function runTask(t, ms, done, onTick) {
    const t0 = performance.now();
    const h = setInterval(() => {
        t.pct = Math.min(100, Math.round(((performance.now() - t0) / ms) * 100));
        onTick?.(t.pct);
        if (t.pct >= 100) {
            clearInterval(h);
            t.state = 'done';
            t.stage = '';
            t.at = Date.now();
            done?.();
            paintTop();
        }
        paintTasks();
    }, 80);
}

// ---------- 模拟：启动 ----------
function launch(id = S.inst, opts = {}) {
    const i = byId(id);
    if (!i) return openDrawer();
    if (REAL) return realLaunch(i);
    if (i.installing) return;
    if (i.bisect && i.bisect.phase !== 'done') return toast('正在二分查冲突，等它做完');
    const prev = RUN.get(id);
    if (prev && prev.phase !== 'crashed') return toast('游戏已经在运行了');
    if (prev) stopTimers(prev);
    const java = javaMajor(i);
    const r = { phase: 'account', pct: 0, missing: i.id === 'survival' ? 3 : 0, java, opts, started: 0, log: [], timers: [], boot: bootLog(i, account(i).name, java), line: 0 };
    r.boot.splice(1, 0, ...schedLines(i), ...plugins.launchLines(i));
    RUN.set(id, r);
    const later = (ms, fn) => r.timers.push(setTimeout(fn, ms));
    const log = (n) => {
        for (let k = 0; k < n && r.line < r.boot.length; k++) pushLog(id, r, r.boot[r.line++]);
    };
    refresh(id, true);
    later(350, () => {
        r.phase = 'files';
        const t0 = performance.now();
        r.ticker = setInterval(() => {
            r.pct = Math.min(100, Math.round((performance.now() - t0) / 11));
            refresh(id);
            if (r.pct < 100) return;
            clearInterval(r.ticker);
            r.phase = 'java';
            log(2);
            refresh(id);
            later(450, () => {
                r.phase = 'window';
                log(3);
                refresh(id);
                later(800, () => decide(i, r));
            });
        }, 50);
    });
}

// 性能调度：启动时做了什么，写进日志第一屏，看得见
function schedLines(i) {
    const out = [];
    const g = eff(i, 'gpu');
    if (g === 'high') out.push(`[LeafStarter/INFO]: 显卡：javaw.exe 设为高性能（${PC.gpus.d}）`);
    if (g === 'save') out.push(`[LeafStarter/INFO]: 显卡：javaw.exe 设为省电（${PC.gpus.i}）`);
    if (eff(i, 'pcore')) out.push('[LeafStarter/INFO]: CPU：游戏进程只用 6 个性能核');
    if (eff(i, 'noeco')) out.push('[LeafStarter/INFO]: 切到后台也不降速（关掉了效率模式节流）');
    if (eff(i, 'prio') === 'above') out.push('[LeafStarter/INFO]: 进程优先级：高于正常');
    if (eff(i, 'power')) out.push('[LeafStarter/INFO]: 电源计划临时换成「高性能」，游戏退出后改回');
    return out;
}

function decide(i, r) {
    if (i.crashDemo && r.java < i.javaNeed) {
        r.phase = 'crashed';
        r.log = crashLog(i).map((l) => `${stamp()} ${l}`);
        i.lastRun = { ok: false, at: Date.now(), min: 0 };
        i.lastLog = r.log;
        refresh(i.id, true);
        return;
    }
    // 规则认不出的那种崩溃（Mod 冲突）
    if (conflictCrash(i)) {
        r.phase = 'crashed';
        r.log = conflictLog(i).map((l) => `${stamp()} ${l}`);
        i.lastRun = { ok: false, at: Date.now(), min: 0 };
        i.lastLog = r.log;
        refresh(i.id, true);
        return;
    }
    r.phase = 'running';
    r.started = Date.now();
    i.lastPlayed = Date.now();
    // 这次实际用的显卡（正式版看系统的 GPU 计数器）
    const g = eff(i, 'gpu');
    if (g === 'high') i.gpuSeen = 'd';
    else if (g === 'save') i.gpuSeen = 'i';
    else i.gpuSeen ??= 'd';
    r.logTimer = setInterval(() => {
        if (r.line < r.boot.length) pushLog(i.id, r, r.boot[r.line++]);
    }, 650);
    refresh(i.id, true);
    const l = $('#launch');
    l.classList.remove('celebrate');
    void l.offsetWidth;
    if (i.id === S.inst) {
        l.classList.add('celebrate');
        burst($('#launch .go'), $('#app'));
    }
    mp.onRunning(i);
}

// 往正在运行的游戏日志里加一行（联机演示用）
function logTo(id, line) {
    const r = RUN.get(id);
    if (r) pushLog(id, r, line);
}

function pushLog(id, r, line) {
    r.log.push(`${stamp()} ${line}`);
    if (id !== S.inst || S.page !== 'home' || S.tab !== 'logs') return;
    const box = $('#logBox');
    if (!box) return;
    const follow = box.scrollTop + box.clientHeight >= box.scrollHeight - 12;
    box.innerHTML = logText(ctx());
    if (follow) box.scrollTop = box.scrollHeight;
}

function stopTimers(r) {
    r.timers.forEach(clearTimeout);
    clearInterval(r.ticker);
    clearInterval(r.logTimer);
}

function stop(id) {
    const r = RUN.get(id);
    if (!r || r.phase !== 'running') return;
    stopTimers(r);
    RUN.delete(id);
    const i = byId(id);
    const min = Math.max(1, Math.round((Date.now() - r.started) / 60000));
    i.lastRun = { ok: true, at: Date.now(), min };
    i.lastPlayed = Date.now();
    i.played += min;
    i.lastLog = r.log;
    // 进过哪个世界就更新哪个；没指定就算最近玩的那个
    const w = i.worlds.find((x) => x.name === r.opts.v) || [...i.worlds].sort((a, b) => b.last - a.last)[0];
    if (w) w.last = Date.now();
    if (i.sync.on) upload(i, '退出游戏后自动上传');
    mp.onStop(i);
    refresh(id, true);
}

// AI 修冲突：Glimmer Render 更新到 0.4.3（能撤回）
function fixGlimmer(i) {
    const g = i.mods.find((m) => m.name === 'Glimmer Render');
    if (!g) return;
    const before = { ver: g.ver, update: g.update, on: g.on };
    g.ver = '0.4.3';
    delete g.update;
    g.on = true;
    fix.stop(i);
    if (RUN.get(i.id)?.phase === 'crashed') RUN.delete(i.id);
    refresh(i.id, true);
    toast('Glimmer Render 更新到了 0.4.3', {
        undo: () => {
            Object.assign(g, before);
            if (cur() === i) paintView();
        },
    });
}

// AI 配的整合包：新建一个实例开始装
function createPack() {
    const i = newInstance({ name: uniqueName('科技向 1.20.1'), mc: '1.20.1', loader: 'Fabric', perf: true, sync: false, dir: 'own' });
    i.block = 'brass';
    i.mods.push(...[['Create Fabric', '0.5.1-f'], ['Tech Reborn', '5.8.11'], ['Reborn Core', '5.8.11'], ['Applied Energistics 2', '15.2.13'], ['EMI', '1.1.18'], ['Jade', '11.12.3']]
        .map(([n, v]) => makeMod(n, v, 'Modrinth')));
    INSTANCES.push(i);
    paintRail();
    install(i);
    ensureVisible(i.id);
}

function fixCrash(id) {
    const i = byId(id);
    delete i.cfg.java;
    RUN.delete(id);
    toast(`改回了自动选择：Java ${i.javaNeed}`);
    launch(id);
}

// ---------- 模拟：同步、安装 ----------
function upload(i, note) {
    i.sync.state = 'up';
    refresh(i.id, true);
    const t = addTask(`同步 ${i.name}`, '上传改动过的文件');
    runTask(t, 1400, () => {
        i.sync.state = 'ok';
        i.sync.at = Date.now();
        const w = [...i.worlds].sort((a, b) => b.last - a.last)[0];
        if (w) w.history.unshift({ dev: '电脑', at: Date.now(), note });
        refresh(i.id, true);
    });
}

function install(i) {
    const stages = [[0, '读取版本信息'], [8, '下载游戏文件'], [78, i.loader ? `安装 ${i.loader}` : '整理文件'], [93, i.perf ? '加上优化 Mod' : '写入版本信息']];
    i.installing = { pct: 0, stage: stages[0][1] };
    const t = addTask(`安装 ${i.name}`, `${i.mc} · ${i.loader || '原版'}`);
    refresh(i.id, true);
    runTask(t, 4200, () => {
        delete i.installing;
        toast(`「${i.name}」装好了`);
        refresh(i.id, true);
    }, (p) => {
        i.installing.pct = p;
        i.installing.stage = stages.filter(([at]) => p >= at).at(-1)[1];
        t.stage = i.installing.stage;
        refresh(i.id);
    });
}

// ---------- 新建实例 ----------
function uniqueName(base) {
    let n = base;
    for (let k = 2; INSTANCES.some((i) => i.name === n); k++) n = `${base} (${k})`;
    return n;
}

// 右侧面板：新建实例（默认）、云服务器开服（kind: host）、导入服务器（kind: import）
function openDrawer(pre = {}) {
    closePop();
    agent.close();
    if (pre.kind) {
        S.drawer = pre;
        paintDrawer();
        requestAnimationFrame(() => $('#drawer input[type="text"]')?.focus());
        return;
    }
    S.drawer = { kind: 'new', mc: '1.20.1', loader: 'Fabric', perf: true, sync: false, dir: REAL ? defaultDir() : 'own', vq: '', name: '', touched: false, ...pre };
    autoName();
    paintDrawer();
    requestAnimationFrame(() => $('#drName')?.select());
}

function drawerPrimary() {
    const k = S.drawer?.kind;
    if (k === 'host' || k === 'import') mp.drawerPrimary();
    else if (k === 'i18n' || k === 'tmpl' || k === 'world') tools.primary();
    else if (k === 'pinst') plugins.primary();
    else if (k === 'about') closeDrawer();
    else if (k === 'msa') msaPrimary();
    else if (k === 'off') offCreate();
    else if (k === 'ygg') yggLogin();
    else if (k === 'res') resPrimary();
    else createFromDrawer();
}

function autoName() {
    const d = S.drawer;
    if (!d.touched) d.name = uniqueName(`${d.mc} ${d.loader || '原版'}`);
}

function closeDrawer() {
    if (!S.drawer) return false;
    S.drawer = null;
    const d = $('#drawer .drawer');
    if (!d) return true;
    d.classList.add('out');
    setTimeout(() => {
        if (!S.drawer) $('#drawer').innerHTML = '';
    }, 170);
    return true;
}

function createFromDrawer() {
    const d = S.drawer;
    if (REAL) return realCreate(d);
    if (!d || d.kind !== 'new' || !d.name.trim() || (d.loader && loaderVersion(d.loader, d.mc) === null)) return;
    const i = newInstance({ ...d, name: d.name.trim() });
    INSTANCES.push(i);
    closeDrawer();
    S.inst = i.id;
    S.tab = 'overview';
    S.page = 'home';
    save();
    paintTop();
    paintRail();
    paintView('inst');
    install(i);
    ensureVisible(i.id);
}

// ---------- 小弹层（账户、存档历史） ----------
let pop = null;
function openPop(anchor, html, place) {
    closePop();
    const app = $('#app');
    const ar = app.getBoundingClientRect();
    const r = anchor.getBoundingClientRect();
    const el = document.createElement('div');
    el.className = 'pop';
    el.innerHTML = html;
    app.append(el);
    if (place === 'above') {
        el.style.left = `${r.left - ar.left}px`;
        el.style.width = `${r.width}px`;
        el.style.bottom = `${ar.bottom - r.top + 6}px`;
    } else {
        const w = el.offsetWidth;
        el.style.left = `${Math.max(8, Math.min(r.right - ar.left - w, ar.width - w - 8))}px`;
        // 下面放不下就翻到上面；两边都放不下，就贴着更宽的那边，自己滚动
        const below = ar.bottom - r.bottom - 14;
        const above = r.top - ar.top - 14;
        if (el.offsetHeight <= below || below >= above) {
            el.style.top = `${r.bottom - ar.top + 6}px`;
            el.style.maxHeight = `${below}px`;
        } else {
            el.style.bottom = `${ar.bottom - r.top + 6}px`;
            el.style.maxHeight = `${above}px`;
        }
    }
    pop = { el, anchor };
    el.querySelector('button')?.focus({ preventScroll: true });
}

const popAnchor = () => pop?.anchor;
function closePop(refocus = true) {
    if (!pop) return false;
    pop.el.remove();
    if (refocus && pop.anchor.isConnected) pop.anchor.focus({ preventScroll: true });
    pop = null;
    return true;
}

function openAcct(anchor) {
    const html = ACCOUNTS.map((a) => `<button type="button" class="pop-item" data-act="acct-switch" data-id="${a.id}">${head(a.face, 22)}<span class="tx"><span class="nm">${esc(a.name)}</span><span class="sb">${esc(a.label)}</span></span>${a.id === S.account ? icon('check', 14) : ''}</button>`).join('')
        + `<div class="pop-sep"></div>${plugins.off('skin') ? '' : `<button type="button" class="pop-item" data-act="skin">${icon('user')}<span class="tx"><span class="nm">皮肤工作台</span><span class="sb">自己画，或者让 AI 生成，一键换上</span></span></button>`}`
        + `<button type="button" class="pop-item" data-act="acct-add">${icon('plus')}<span class="tx"><span class="nm">添加账户</span><span class="sb">微软正版 · 离线 · 外置登录</span></span></button>`;
    openPop(anchor, html, 'above');
}

function openHistory(anchor, wid) {
    const w = cur()?.worlds.find((x) => x.id === wid);
    if (!w) return;
    const html = `<div class="pop-h">「${esc(w.name)}」的历史版本</div>`
        + w.history.map((h, k) => `<div class="pop-row">${icon(h.dev.startsWith('手机') ? 'smartphone' : 'monitor', 15)}<span class="tx"><span class="nm">${ago(h.at)} · ${esc(h.dev)}</span>${h.note ? `<span class="sb">${esc(h.note)}</span>` : ''}</span>${k === 0 ? '<span class="chip ok">现在</span>' : `<button type="button" class="btn sm" data-act="restore" data-v="${w.id}" data-k="${k}">退回到这里</button>`}</div>`).join('')
        + '<p class="pop-note">退回前，现在的版本会先存进历史，不会丢。</p>';
    openPop(anchor, html, 'below');
}

// 正式版会打开的文件夹
function folderOf(i, sub) {
    const dir = DIRS.find((d) => d.id === i.dir).path;
    return `${dir}${isolated(i) ? `\\versions\\${i.name}` : ''}${sub ? `\\${sub}` : ''}`;
}

// ---------- 直接启动的预览：屏幕角落的小窗，倒数 3 秒 ----------
let mini = null;
function previewDirect() {
    cancelDirect();
    const i = cur();
    if (!i) return;
    let n = 3;
    const el = document.createElement('div');
    el.className = 'mini';
    el.setAttribute('role', 'status');
    const draw = () => {
        el.innerHTML = `${block(i.block, 36)}<span class="tx"><b>${n} 秒后启动「${esc(i.name)}」</b><small>正式版里这是屏幕角落的小窗，主界面不会出现</small></span><button type="button" class="btn sm" data-act="mini-cancel">取消 <kbd>Esc</kbd></button>`;
    };
    draw();
    $('#app').append(el);
    const h = setInterval(() => {
        n -= 1;
        if (n > 0) return draw();
        cancelDirect();
        launch(i.id);
    }, 1000);
    mini = { el, h };
}

function cancelDirect() {
    if (!mini) return false;
    clearInterval(mini.h);
    mini.el.remove();
    mini = null;
    return true;
}

// ---------- 下载页 ----------
let dlTimer;
function dlSearch() {
    S.dl.loading = true;
    paintDl();
    clearTimeout(dlTimer);
    dlTimer = setTimeout(() => {
        S.dl.loading = false;
        paintDl();
    }, 240);
}

function dlInstall(id) {
    const c = ctx();
    const type = S.dl.type;
    const it = CATALOG[type].find((x) => x.id === id);
    if (!it) return;
    if (type === 'pack') {
        const i = newInstance({ name: uniqueName(it.name), mc: it.mc, loader: it.loader, perf: false, sync: false, dir: 'own' });
        i.block = 'diamond';
        INSTANCES.push(i);
        S.dl.installed[`pack:${id}`] = 'done';
        paintRail();
        install(i);
        paintDl();
        toast(`「${it.name}」正在装成新实例，左边能看到进度`);
        return;
    }
    const t = dlTarget(c);
    const key = `${t.id}:${id}`;
    S.dl.installed[key] = 'busy';
    paintDl();
    setTimeout(() => {
        S.dl.installed[key] = 'done';
        if (type === 'mod' && !t.mods.some((m) => m.name === it.name)) t.mods.push({ id: 'm' + Date.now(), name: it.name, ver: '最新版', src: it.src, on: true, h: it.h });
        paintDl();
    }, 700);
}

// ---------- 命令面板 ----------
const pal = { list: [], active: 0, prevFocus: null };

function commands(q) {
    if (REAL) return realCommands(q);
    const i = cur();
    const out = [];
    const sub = (x) => `${x.mc} · ${x.loader || '原版'}`;
    if (i) out.push({ g: '常用', ic: 'play', t: `启动 ${i.name}`, k: 'Enter', run: () => launch() });
    out.push({ g: '常用', ic: 'plus', t: '新建实例', run: () => openDrawer() });
    out.push({ g: '常用', ic: 'sparkles', t: '问 AI 助手', sub: '查崩溃、开服卡住、配整合包', run: () => agent.open() });
    out.push({ g: '常用', ic: 'user', t: '皮肤工作台', sub: '自己画，或者让 AI 生成，一键换上', run: () => goPage('skin') });
    if (i) {
        out.push({ g: '联机', ic: 'users', t: `邀请朋友一起玩「${i.name}」`, run: () => { S.mpMode = 'now'; save(); setTab('servers'); } });
        out.push({ g: '联机', ic: 'server', t: `给「${i.name}」开一个常驻服务器`, run: () => { S.mpMode = 'host'; save(); setTab('servers'); } });
    }
    const sky = INSTANCES.find((x) => x.crashDemo === 'conflict');
    if (sky) out.push({ g: '修复', ic: 'split', t: `二分查冲突 Mod：${sky.name}`, sub: '不用 AI', run: () => { selectGo(sky.id); fix.start(sky); } });
    INSTANCES.forEach((x) => out.push({ g: '实例', ic: 'layers', t: `打开 ${x.name}`, sub: sub(x), run: () => selectGo(x.id) }));
    INSTANCES.forEach((x) => x !== i && out.push({ g: '启动', ic: 'play', t: `启动 ${x.name}`, sub: sub(x), run: () => { selectGo(x.id); launch(x.id); } }));
    if (i) {
        out.push({ g: '这个实例', ic: 'folder', t: `打开「${i.name}」的 mods 文件夹`, run: () => toast(`正式版会打开：${folderOf(i, 'mods')}`) });
        [['mods', 'Mod 列表'], ['worlds', '存档'], ['logs', '日志'], ['config', '实例设置']].forEach(([k, t]) => out.push({ g: '这个实例', ic: 'list', t: `看「${i.name}」的${t}`, run: () => setTab(k) }));
        if (i.loader) out.push({ g: '这个实例', ic: 'globe', t: `给「${i.name}」补全中文`, sub: '没中文的 Mod 用社区汉化、机器或 AI 补上', run: () => { setTab('mods'); tools.zhOpen(i); } });
        out.push({ g: '这个实例', ic: 'keyboard', t: `把「${i.name}」的键位和 FOV 套用到别的实例`, run: () => { setTab('config'); ACT['tmpl-open']({ dataset: { v: 'out' } }); } });
        out.push({ g: '这个实例', ic: 'cpu', t: `「${i.name}」用独显、性能调度`, run: () => setTab('config') });
        i.worlds.forEach((w) => out.push({ g: '存档', ic: 'globe', t: `看「${w.name}」的地图和种子`, sub: i.name, run: () => { setTab('worlds'); world.open(w); paintBody(); } }));
    }
    out.push({ g: '设置', ic: 'sparkles', t: 'AI 源和高级参数', run: () => { goPage('settings'); requestAnimationFrame(() => $('#aiPanel')?.scrollIntoView({ block: 'start' })); } });
    out.push({ g: '设置', ic: 'package', t: '功能包：联机、世界工具、离线翻译', run: () => { goPage('settings'); requestAnimationFrame(() => $('#extPanel')?.scrollIntoView({ block: 'start' })); } });
    [['新建 1.20.1 Fabric 实例', { mc: '1.20.1', loader: 'Fabric' }], ['新建 1.21.1 NeoForge 实例', { mc: '1.21.1', loader: 'NeoForge' }], ['新建 1.12.2 Forge 实例', { mc: '1.12.2', loader: 'Forge', perf: false }]]
        .forEach(([t, pre]) => out.push({ g: '新建', ic: 'plus', t, run: () => openDrawer(pre) }));
    out.push({ g: '页面', ic: 'download', t: '下载 Mod', k: 'Ctrl 2', run: () => { S.dl.type = 'mod'; goPage('download'); } });
    out.push({ g: '页面', ic: 'package', t: '下载整合包', run: () => { S.dl.type = 'pack'; goPage('download'); } });
    out.push({ g: '页面', ic: 'list', t: '任务', k: 'Ctrl 3', run: () => goPage('tasks') });
    out.push({ g: '页面', ic: 'plug', t: '插件', sub: '已装的插件、插件市场、定制版', k: 'Ctrl 4', run: () => { S.ptab = 'installed'; goPage('plugins'); } });
    out.push({ g: '页面', ic: 'sliders', t: '设置', k: 'Ctrl 5', run: () => goPage('settings') });
    out.push({ g: '插件', ic: 'download', t: '插件市场', sub: '主题、崩溃规则、镜像、脚本插件', run: () => { S.ptab = 'market'; goPage('plugins'); } });
    out.push({ g: '插件', ic: 'package', t: '定制版制作器', sub: '给服主和整合包作者：做自家专用启动器', run: () => { S.ptab = 'dist'; goPage('plugins'); } });
    out.push({ g: '插件', ic: 'shield', t: '以安全模式重新打开', sub: '不加载任何第三方插件', run: () => ACT['safe-on']() });
    out.push({ g: '其他', ic: 'file', t: '关于 LeafStarter', sub: '版本、许可证、第三方许可', run: () => openDrawer({ kind: 'about' }) });
    if (i && plugins.on('packdoc')) out.push({ g: '插件', ic: 'plug', t: `整合包体检：${i.name}`, sub: '插件「整合包体检」提供', run: () => ACT['pack-doctor']() });
    ACCOUNTS.forEach((a) => a.id !== S.account && out.push({ g: '账户', ic: 'user', t: `切换到 ${a.name}（${a.label}）`, run: () => switchAcct(a.id) }));
    [['system', '跟随系统'], ['light', '浅色'], ['dark', '深色']].forEach(([v, t]) => v !== S.theme && out.push({ g: '外观', ic: 'sun', t: `主题改成${t}`, run: () => setPref('theme', v) }));
    if (q.trim().length >= 2) CATALOG.mod.forEach((m) => out.push({ g: '下载', ic: 'download', t: `下载 ${m.name}`, sub: m.sum, run: () => { S.dl.type = 'mod'; S.dl.q = m.name; goPage('download'); } }));
    return out;
}

function rank(x, q) {
    const t = `${x.t} ${x.sub || ''}`.toLowerCase();
    let score = 0;
    for (const w of q.toLowerCase().split(/\s+/).filter(Boolean)) {
        const k = t.indexOf(w);
        if (k >= 0) {
            score += k === 0 ? 4 : 3 - Math.min(k, 40) / 40;
            continue;
        }
        let j = 0;
        for (const ch of t) if (ch === w[j]) j += 1;
        if (j < w.length) return 0;
        score += 1;
    }
    return score;
}

function renderPalette() {
    const q = $('#palInput').value;
    const all = commands(q);
    pal.list = q.trim()
        ? all.map((x) => [rank(x, q), x]).filter(([s]) => s > 0).sort((a, b) => b[0] - a[0]).slice(0, 12).map(([, x]) => x)
        : all.filter((x) => ['常用', '实例', '页面'].includes(x.g));
    pal.active = Math.min(pal.active, Math.max(0, pal.list.length - 1));
    const box = $('#palList');
    if (!pal.list.length) {
        box.innerHTML = `<p class="pal-empty">没有和「${esc(q)}」对得上的</p>`;
        return;
    }
    let g = null;
    box.innerHTML = pal.list.map((x, k) => {
        const head = !q.trim() && x.g !== g ? `<div class="pal-group">${esc((g = x.g))}</div>` : '';
        return `${head}<button type="button" class="pal-item${k === pal.active ? ' on' : ''}" data-pal="${k}" role="option" aria-selected="${k === pal.active}" tabindex="-1">${icon(x.ic, 16)}<span class="tx"><span class="nm">${esc(x.t)}</span>${x.sub ? `<span class="sb">${esc(x.sub)}</span>` : ''}</span>${x.k ? `<span class="k">${x.k.split(' ').map((s) => `<kbd>${s}</kbd>`).join('')}</span>` : ''}</button>`;
    }).join('');
}

function markPal() {
    $$('#palList .pal-item').forEach((b) => {
        const on = Number(b.dataset.pal) === pal.active;
        b.classList.toggle('on', on);
        b.setAttribute('aria-selected', on);
        if (on) b.scrollIntoView({ block: 'nearest' });
    });
}

function openPalette() {
    const p = $('#palette');
    if (!p.hidden) return;
    closePop(false);
    pal.prevFocus = document.activeElement;
    pal.active = 0;
    $('#palInput').value = '';
    p.hidden = false;
    renderPalette();
    $('#palInput').focus();
}

function closePalette(refocus = true) {
    const p = $('#palette');
    if (p.hidden) return false;
    p.hidden = true;
    if (refocus && pal.prevFocus?.isConnected) pal.prevFocus.focus({ preventScroll: true });
    return true;
}

function runPal(k) {
    const x = pal.list[k];
    if (!x) return;
    closePalette(false);
    x.run();
}

// ---------- 其他动作 ----------
function switchAcct(id) {
    S.account = id;
    save();
    closePop();
    paintAcct();
    paintLaunch(true);
    if (S.page === 'settings') paintView();
    else paintScenes();
}

function setPref(k, v) {
    S[k] = v;
    save();
    if (k === 'theme') {
        if (v === 'system') document.documentElement.removeAttribute('data-theme');
        else document.documentElement.dataset.theme = v;
    }
    if (k === 'motion') document.documentElement.dataset.motion = v;
    if (k === 'simple') {
        $('#app').classList.toggle('simple', v);
        if (v) agent.close();
    }
    if (S.page === 'settings') paintView();
    if (k.startsWith('ai')) agent.rerender();
}

function setSync(on) {
    const i = cur();
    if (!i) return;
    i.sync.on = on;
    if (on) return upload(i, '开启同步，第一次上传');
    refresh(i.id, true);
    paintRail();
    toast(`「${i.name}」不再同步。云端的历史版本还留着`);
}

function setCfg(el) {
    const i = cur();
    const key = el.dataset.cfg;
    const val = el.type === 'checkbox' ? el.checked : el.value;
    const def = key === 'isolate' ? !!i.loader : GLOBAL[key];
    if (String(val) === String(def)) delete i.cfg[key];
    else i.cfg[key] = val;
    paintView();
    paintLaunch(true);
}

function delInst() {
    const i = cur();
    if (!i) return;
    if (RUN.get(i.id)?.phase === 'running') return toast('先结束游戏，再删这个实例');
    if (i.installing) return toast('等它装完再删');
    const ids = visibleIds(ctx());
    const k = ids.indexOf(i.id);
    const idx = INSTANCES.indexOf(i);
    INSTANCES.splice(idx, 1);
    RUN.delete(i.id);
    S.inst = ids[k + 1] || ids[k - 1] || INSTANCES[0]?.id || null;
    save();
    paintRail();
    paintView();
    paintLaunch();
    toast(`「${i.name}」移到了回收站`, {
        undo: () => {
            INSTANCES.splice(idx, 0, i);
            S.inst = i.id;
            save();
            paintRail();
            paintView();
            paintLaunch();
        },
    });
}

function copyInst() {
    const i = cur();
    if (!i) return;
    const { installing, room, srv, bisect, ...rest } = i;
    const k = structuredClone(rest);
    Object.assign(k, { id: 'i' + Date.now().toString(36), name: uniqueName(`${i.name} 副本`), lastPlayed: 0, played: 0, lastRun: null });
    k.sync.on = false;
    INSTANCES.splice(INSTANCES.indexOf(i) + 1, 0, k);
    S.inst = k.id;
    save();
    paintRail();
    paintView();
    paintLaunch();
    ensureVisible(k.id);
    toast(`复制了一份「${k.name}」`, {
        undo: () => {
            INSTANCES.splice(INSTANCES.indexOf(k), 1);
            if (S.inst === k.id) S.inst = i.id;
            save();
            paintRail();
            paintView();
            paintLaunch();
        },
    });
}

function perfOff() {
    const i = cur();
    const before = i.mods.slice();
    const n = i.mods.filter((m) => m.perf).length;
    i.mods = i.mods.filter((m) => !m.perf);
    i.perf = false;
    paintView();
    toast(`撤掉了 ${n} 个优化 Mod`, {
        undo: () => {
            i.mods = before;
            i.perf = true;
            if (cur() === i) paintView();
        },
    });
}

function perfOn(btn) {
    const i = cur();
    btn.disabled = true;
    btn.textContent = '正在加…';
    setTimeout(() => {
        const have = new Set(i.mods.map((m) => m.name));
        const add = perfMods(i.loader).filter((m) => !have.has(m.name));
        i.mods.push(...add);
        i.perf = true;
        if (cur() === i && S.page === 'home') paintView();
        toast(`加上了 ${add.length} 个优化 Mod`);
    }, 700);
}

function toggleMod(id, on, input) {
    const m = cur()?.mods.find((x) => x.id === id);
    if (!m) return;
    if (REAL) return realToggleMod(cur(), m, on, input);
    m.on = on;
    input.closest('.row')?.classList.toggle('off', !on);
    const count = $('#modCount');
    if (count) count.textContent = modCount(cur());
}

function updateMods(ids) {
    const i = cur();
    const list = i.mods.filter((m) => m.update && (!ids || ids.includes(m.id)));
    if (!list.length) return;
    if (list.length === 1) {
        const [m] = list;
        m.ver = m.update;
        delete m.update;
        paintView();
        return toast(`${m.name} 更新到了 ${m.ver}`);
    }
    const btn = $('#modsUp');
    if (btn) {
        btn.disabled = true;
        btn.textContent = '正在更新…';
    }
    const t = addTask(`更新 ${list.length} 个 Mod`, `${i.name} · ${list.map((m) => m.name).join('、')}`);
    runTask(t, 900, () => {
        list.forEach((m) => {
            m.ver = m.update;
            delete m.update;
        });
        if (cur() === i && S.page === 'home') paintView();
        toast(`更新了 ${list.length} 个 Mod`);
    });
}

// 先让那一行收拢（看得见是哪一行没了），再真正移除；撤销时那一行展开回来
function leaveThen(id, fn) {
    const row = $(`[data-row="${CSS.escape(id)}"]`);
    if (row?.classList.contains('leave')) return;
    if (!row) return fn();
    row.classList.add('leave');
    setTimeout(fn, 170);
}

function arrive(id) {
    $(`[data-row="${CSS.escape(id)}"]`)?.classList.add('arrive');
}

function delMod(id) {
    const i = cur();
    if (!i.mods.some((m) => m.id === id)) return;
    leaveThen(id, () => {
        const k = i.mods.findIndex((m) => m.id === id);
        const [m] = i.mods.splice(k, 1);
        paintView();
        toast(`删除了 ${m.name}`, {
            undo: () => {
                i.mods.splice(k, 0, m);
                if (cur() !== i) return;
                paintView();
                arrive(m.id);
            },
        });
    });
}

function restore(wid, k) {
    const w = cur()?.worlds.find((x) => x.id === wid);
    const h = w?.history[k];
    if (!h) return;
    closePop(false);
    const before = w.history.slice();
    w.history.unshift({ dev: '电脑', at: Date.now(), note: `退回到 ${ago(h.at)}（${h.dev}）的版本` });
    toast(`「${w.name}」退回到了 ${ago(h.at)} 的版本`, { undo: () => { w.history = before; } });
}

function delShot(id) {
    const i = cur();
    if (!i.shots.some((s) => s.id === id)) return;
    leaveThen(id, () => {
        const k = i.shots.findIndex((s) => s.id === id);
        const [s] = i.shots.splice(k, 1);
        paintView();
        toast('截图移到了回收站', {
            undo: () => {
                i.shots.splice(k, 0, s);
                if (cur() === i) paintView();
            },
        });
    });
}

function copyLog() {
    const i = cur();
    const text = (RUN.get(i.id)?.log || i.lastLog || []).join('\n');
    navigator.clipboard?.writeText(text).then(() => toast('日志复制好了'), () => toast('浏览器不让复制，选中文字后按 Ctrl C'));
}

function verify(btn) {
    const html = btn.innerHTML;
    btn.disabled = true;
    btn.innerHTML = `${icon('refresh')}正在检查…`;
    setTimeout(() => {
        btn.disabled = false;
        btn.innerHTML = html;
        toast('检查了 1,824 个文件，都在，没有要补的');
    }, 900);
}

// ---------- 联机、二分、AI：各自一个模块，通过这个接口回到这里画界面、起任务 ----------
const api = {
    S, RUN, INSTANCES, cur, byId, ctx, save, aiReady, aiFor, store,
    paintView, paintBody, paintTabs, paintDrawer, paintRail, paintTasks, paintTop, paintAcct, paintScenes, refresh,
    openDrawer, closeDrawer, launch, setTab, goPage, selectGo, logTo, addTask, runTask, arrive,
    openPop, closePop, popAnchor, fixGlimmer, createPack,
    acct: () => ACCOUNTS.find((a) => a.id === S.account) || ACCOUNTS[0],
    face: () => account(cur()).face,
    acctName: () => account(cur()).name,
    aiDemo: () => aisrc.demo(),
    install, paintLaunch,
    paintDistMock: () => {
        const m = $('#distMock');
        if (m) m.innerHTML = distMock(ctx());
    },
    paintPlugGrid: () => paintView(),
};
const aisrc = createAiSrc(api);
const mp = createMp(api);
const fix = createFix(api);
const agent = REAL ? createAgentReal(api) : createAgent(api);
const aiSet = REAL ? createAiSettings(api) : null;
const servers = REAL ? createServers(api) : null;
const wtool = REAL ? createWorldTool(api) : null;
const edition = REAL ? createEdition(api) : null;
const plug = REAL ? createPluginHost(api) : null;
const mpr = REAL ? createMpReal(api) : null;
const upd = REAL ? createUpdate(api) : null;
if (REAL) api.launchJoin = (i, addr) => realLaunch(i, addr);
if (REAL) {
    api.defaultDir = () => defaultDir();
    api.applyLook = () => plugins.applyLook();
}
if (REAL) api.refreshWorlds = (i) => real.loadWorldInfo(i);
if (REAL) {
    real.R.aiUi = aiSet.E;
    real.R.mcpLog = agent.A.mcpLog;
    Object.assign(DRAWERS, { srvadd: srvAddDrawer, paneladd: panelAddDrawer, srvcon: srvConsoleDrawer, srvcfg: srvCfgDrawer, worldtool: worldToolDrawer, edmaker: edMakerDrawer, plugconsent: plugConsentDrawer, mpjoin: mpJoinDrawer, about: realAboutDrawer });
}
const world = createWorld(api);
const skin = createSkin(api);
const tools = createTools(api);
const plugins = createPlugins(api);
api.plugOff = (id) => plugins.off(id);
api.mp = mp;
api.agent = agent;

const ACT = {
    sel: (el) => selectGo(el.dataset.id),
    tab: (el) => setTab(el.dataset.tab),
    rail: () => toggleRail(),
    pal: () => openPalette(),
    acct: (el) => (pop?.anchor === el ? closePop() : openAcct(el)),
    'acct-switch': (el) => switchAcct(el.dataset.id),
    'acct-add': () => {
        closePop();
        toast('原型里不接真实登录。正式版会给你一串代码，在手机或浏览器上输入就登录好了');
    },
    launch: () => launch(),
    stop: () => stop(S.inst),
    'run-log': () => setTab('logs'),
    'crash-view': () => {
        setTab('overview');
        requestAnimationFrame(() => $('#crashFix')?.focus());
    },
    'crash-fix': () => fixCrash(S.inst),
    'crash-dismiss': () => {
        RUN.delete(S.inst);
        refresh(S.inst, true);
    },
    'new-inst': () => openDrawer(),
    'dr-close': () => closeDrawer(),
    'dr-mc': (el) => {
        S.drawer.mc = el.dataset.v;
        autoName();
        paintDrawer();
    },
    'dr-loader': (el) => {
        S.drawer.loader = el.dataset.v || null;
        autoName();
        paintDrawer();
    },
    'dr-create': () => drawerPrimary(),
    folder: (el) => toast(`正式版会打开：${folderOf(cur(), el.dataset.v)}`),
    verify: (el) => verify(el),
    export: () => toast('正式版会导出成 .mrpack 整合包，可以勾选带上哪些文件夹'),
    copy: () => copyInst(),
    'del-inst': () => delInst(),
    'sync-on': () => setSync(true),
    'sync-hist': (el) => (pop?.anchor === el ? closePop() : openHistory(el, el.dataset.v)),
    restore: (el) => restore(el.dataset.v, Number(el.dataset.k)),
    'perf-off': () => perfOff(),
    'perf-on': (el) => perfOn(el),
    'mods-f': (el) => {
        S.modFilter = el.dataset.v;
        paintView();
    },
    'mod-up': (el) => updateMods([el.dataset.id]),
    'mods-up': () => updateMods(),
    'mod-del': (el) => delMod(el.dataset.id),
    'add-mod': () => {
        S.dl.type = 'mod';
        S.dl.target = null;
        goPage('download');
    },
    'world-play': (el) => launch(S.inst, { v: el.dataset.v }),
    'server-play': (el) => launch(S.inst, { v: el.dataset.v }),
    backup: (el) => toast(`正式版会备份到：${folderOf(cur(), 'backups')}\\${el.dataset.v}.zip`),
    'servers-ping': () => {
        cur().servers.forEach((s) => { s.ping = Math.max(2, Math.round(s.ping * (0.8 + Math.random() * 0.4))); });
        const rows = $('#srvRows');
        if (rows) rows.innerHTML = serverRows(ctx());
    },
    'shot-copy': () => toast('截图复制好了（示例）'),
    'shot-del': (el) => delShot(el.dataset.id),
    'log-f': (el) => {
        S.logLevel = el.dataset.v;
        paintView();
    },
    'log-copy': () => copyLog(),
    'cfg-reset': (el) => {
        delete cur().cfg[el.dataset.k];
        paintView();
        paintLaunch(true);
    },
    'dl-type': (el) => {
        S.dl.type = el.dataset.v;
        paintView();
        dlSearch();
    },
    'dl-install': (el) => dlInstall(el.dataset.id),
    'tasks-clear': () => {
        for (let k = TASKS.length - 1; k >= 0; k--) if (TASKS[k].state !== 'run') TASKS.splice(k, 1);
        paintTasks();
    },
    set: (el) => setPref(el.dataset.k, el.dataset.v),
    'preview-direct': () => previewDirect(),
    'mini-cancel': () => cancelDirect(),
    'mcp-copy': () => {
        const text = $('#mcpSnip')?.textContent || '';
        navigator.clipboard?.writeText(text).then(() => toast('配置复制好了，粘进 AI 客户端的 MCP 设置里'), () => toast('浏览器不让复制，选中文字后按 Ctrl C'));
    },
};
Object.assign(ACT, mp.actions, fix.actions, agent.actions, world.actions, skin.actions, tools.actions, aisrc.actions, plugins.actions);
ACT['pop-close'] = () => closePop();
ACT['plug-fix'] = () => fixGlimmer(cur());
ACT['dist-export'] = (el) => openPop(el, `<div class="pop-h">定制包（预览）</div><p class="pop-note big">把它和作者签名一起存成 <span class="mono">${esc(S.dist.name)}.lsdist</span>，放在 LeafStarter.exe 旁边就生效。</p><pre class="diff mono" id="distJson">${esc(distJSON(ctx()))}</pre><div class="pop-act"><button type="button" class="btn sm" data-act="dist-copy">${icon('copy', 13)}复制</button></div>`, 'below');
ACT['dist-copy'] = () => {
    const text = $('#distJson')?.textContent || '';
    navigator.clipboard?.writeText(text).then(() => toast('定制包内容复制好了'), () => toast('浏览器不让复制，选中文字后按 Ctrl C'));
};

// ---------- 真程序：接后端的动作；还没做的统一提示「还在做」，不假装做成了 ----------
function defaultDir() {
    const ok = DIRS.filter((x) => x.access === 'direct');
    const last = store.get('lastDir');
    return (ok.find((x) => x.id === last) || ok[0])?.id || '';
}

// 游戏真在运行（崩溃了留着看信息的那次不算）
const isRunning = (i) => !!i && !!RUN.get(i.id) && RUN.get(i.id).phase !== 'crashed';

// Mod 组的操作：做完重新读这个实例的 Mod 和组
async function mgDo(i, fn, ok) {
    try {
        if (fn) await fn();
        toast(ok);
    } catch (e) {
        toast(String(e));
    }
    i.loaded = false;
    ensureDetails(i);
}

// 实例的 Mod、存档：选中时才去读
function ensureDetails(i) {
    if (!REAL || !i?.real || i.loaded || i.loading || i.installing) return;
    i.loading = true;
    real.loadDetails(i)
        .catch((e) => { i.loadError = String(e); })
        .finally(() => {
            i.loading = false;
            if (S.page === 'home' && S.inst === i.id) {
                paintTabs();
                paintBody();
            }
        });
}

// 扫描结果变了：实例还在就留着选中，不在了就选第一个
function afterTargets() {
    if (!byId(S.inst)) S.inst = INSTANCES[0]?.id || null;
    save();
    paintRail();
    paintView();
    paintLaunch(true);
    if (S.drawer?.kind === 'new') paintDrawer();
    ensureDetails(cur());
}

async function realRescan(msg) {
    try {
        await real.rescan();
        if (msg) toast(msg);
    } catch (e) {
        toast(`扫描失败：${e}`);
    }
}

// 启动：先在界面上进入「检查账户」，后端的进度用 launch-state 事件推回来
function realLaunch(i, join) {
    if (i.installing) return;
    if (real.R.bisect.get(i.id)?.phase === 'running') return toast('正在查是哪个 Mod，查完再启动');
    // 崩溃了的那次留着给人看崩溃信息，不算在运行：直接重新启动
    const prev = RUN.get(i.id);
    if (prev && prev.phase !== 'crashed') return toast('游戏已经在运行了');
    if (!real.R.accounts.length) {
        openPop($('#acct'), realAcctPop(ctx()), 'above');
        return toast('先添加一个账户：微软正版或者离线');
    }
    const r = { phase: 'account', pct: 0, log: [], started: 0, detail: '检查账户', run: 0 };
    RUN.set(i.id, r);
    refresh(i.id, true);
    real.launch(i, S.account, join).then(
        (run) => {
            r.run = run;
            real.reloadPrefs();
        },
        (e) => {
            RUN.delete(i.id);
            refresh(i.id, true);
            toast(String(e));
        },
    );
}

// 后端推来的启动进度，换成界面的阶段：account → files → java → window → running
function onLaunchState(p) {
    const i = byId(p.key);
    let r = RUN.get(p.key);
    if (!r) {
        if (p.phase === 'exited' || p.phase === 'failed') return;
        r = { phase: 'account', pct: 0, log: [], started: 0, run: p.run };
        RUN.set(p.key, r);
    }
    const prev = r.phase;
    r.run = p.run;
    if (p.detail) r.detail = p.detail;
    if (p.java) r.java = p.java;
    const ended = p.phase === 'exited' || p.phase === 'failed';
    switch (p.phase) {
        case 'account': r.phase = 'account'; break;
        case 'files': r.phase = 'files'; r.pct = p.pct; break;
        case 'java': r.phase = 'java'; break;
        case 'starting': r.phase = 'window'; break;
        case 'running':
            if (r.phase !== 'running') {
                r.phase = 'running';
                r.started = Date.now();
            }
            break;
        case 'exited':
            RUN.delete(p.key);
            if (i) {
                i.lastLog = r.log;
                i.lastRun = { ok: true, at: Date.now(), min: r.started ? Math.round((Date.now() - r.started) / 60000) : 0 };
            }
            toast(`「${i?.name || '游戏'}」${p.detail}`);
            break;
        case 'crashed':
            r.phase = 'crashed';
            r.crash = { code: p.exitCode, report: p.crashReport, diag: p.diag };
            if (i) {
                i.lastLog = r.log;
                i.lastRun = { ok: false, at: Date.now(), min: 0 };
            }
            if (p.crashReport) real.crashInfo(p.crashReport).then((k) => {
                r.crash = { ...r.crash, ...k };
                if (S.inst === p.key && S.page === 'home') paintBody();
            });
            toast(`「${i?.name || '游戏'}」崩溃了`);
            break;
        case 'failed':
            RUN.delete(p.key);
            if (i) i.lastLog = r.log;
            toast(`没能启动：${p.detail}`);
            break;
    }
    refresh(p.key, ended || r.phase !== prev);
}

// 游戏日志：存进运行记录；正看着「日志」标签就原地刷新
function onGameLog(p) {
    const r = RUN.get(p.key);
    const i = byId(p.key);
    const list = r ? r.log : i ? (i.lastLog ||= []) : null;
    if (!list) return;
    list.push(...p.lines);
    if (list.length > 5000) list.splice(0, list.length - 5000);
    if (S.page === 'home' && S.inst === p.key && S.tab === 'logs') {
        const box = $('#logBox');
        if (box) {
            const bottom = box.scrollTop + box.clientHeight >= box.scrollHeight - 40;
            box.innerHTML = logText(ctx());
            if (bottom) box.scrollTop = box.scrollHeight;
        }
    }
}

// ---------- 账户 ----------
function afterAccounts(id) {
    S.account = id || real.R.defaultAccount;
    save();
    paintAcct();
    paintLaunch(true);
    if (S.page === 'settings') paintView();
}

async function msaStart() {
    closePop();
    S.drawer = { kind: 'msa', state: 'loading' };
    paintDrawer();
    try {
        const dc = await real.invoke('msa_begin');
        if (S.drawer?.kind !== 'msa') return;
        S.drawer = { kind: 'msa', state: 'wait', code: dc.userCode, uri: dc.verificationUri, expires: Date.now() + dc.expiresIn * 1000 };
    } catch (e) {
        if (S.drawer?.kind !== 'msa') return;
        S.drawer = { kind: 'msa', state: 'err', msg: String(e) };
    }
    paintDrawer();
}

function msaPrimary() {
    const d = S.drawer;
    if (d.state === 'wait') {
        navigator.clipboard?.writeText(d.code).catch(() => {});
        real.invoke('open_path', { path: d.uri }).catch(() => {});
        toast('代码复制好了，在打开的网页里粘贴');
    } else if (d.state === 'err') msaStart();
    else if (d.state === 'ok') closeDrawer();
}

async function onMsaResult(p) {
    if (p.ok) {
        afterAccounts(await real.reloadAccounts());
        afterAccounts(p.account.id);
        toast(`登录好了：${p.account.name}`);
        if (S.drawer?.kind === 'msa') {
            S.drawer = { kind: 'msa', state: 'ok', name: p.account.name };
            paintDrawer();
        }
    } else if (S.drawer?.kind === 'msa') {
        S.drawer = { kind: 'msa', state: 'err', msg: p.error };
        paintDrawer();
    } else toast(`微软登录没成功：${p.error}`);
}

async function offStart() {
    closePop();
    const r = await real.invoke('offline_allowed').catch(() => ({ ok: true }));
    S.drawer = { kind: 'off', name: '', blocked: r.ok ? '' : r.reason };
    paintDrawer();
    requestAnimationFrame(() => $('#offName')?.focus());
}

function yggStart() {
    closePop();
    S.drawer = { kind: 'ygg', state: 'form', server: store.get('yggServer', ''), user: '' };
    paintDrawer();
    requestAnimationFrame(() => $(S.drawer.server ? '#yggUser' : '#yggServer')?.focus());
}

async function yggLogin() {
    const d = S.drawer;
    if (d?.kind !== 'ygg' || d.state === 'busy' || d.state === 'choose') return;
    const server = ($('#yggServer')?.value || '').trim();
    const user = ($('#yggUser')?.value || '').trim();
    const password = $('#yggPass')?.value || '';
    if (!server || !user || !password) {
        S.drawer = { ...d, server, user, err: '服务器、账号、密码都要填' };
        return paintDrawer();
    }
    S.drawer = { ...d, server, user, state: 'busy', err: '' };
    paintDrawer();
    try {
        yggDone(await real.invoke('ygg_login', { server, username: user, password }));
        store.set('yggServer', server);
    } catch (e) {
        S.drawer = { ...S.drawer, state: 'form', err: String(e) };
        paintDrawer();
        requestAnimationFrame(() => $('#yggPass')?.focus());
    }
}

async function yggDone(r) {
    if (r.step === 'choose') {
        S.drawer = { ...S.drawer, state: 'choose', ticket: r.ticket, server: r.server, profiles: r.profiles, err: '' };
        return paintDrawer();
    }
    afterAccounts(await real.reloadAccounts());
    if (real.R.accounts.length === 1) afterAccounts(r.account.id);
    closeDrawer();
    toast(`登录好了：${r.account.name}（${r.account.server}）`);
}

async function offCreate() {
    const d = S.drawer;
    if (d?.kind !== 'off' || d.blocked) return;
    const name = ($('#offName')?.value || d.name || '').trim();
    try {
        const a = await real.invoke('add_offline_account', { name });
        afterAccounts(await real.reloadAccounts());
        if (real.R.accounts.length === 1) afterAccounts(a.id);
        closeDrawer();
        toast(`加好了：${a.name}`);
    } catch (e) {
        S.drawer = { ...d, name, err: String(e) };
        paintDrawer();
    }
}

async function realCreate(d) {
    if (!d || d.kind !== 'new' || !d.name.trim()) return;
    const dir = DIRS.find((x) => x.id === d.dir && x.access === 'direct');
    if (!dir) return toast('先选一个游戏目录');
    if (d.loader && !loaderVersion(d.loader, d.mc)) return;
    let i;
    try {
        i = await real.install(dir, d.name.trim(), d.mc, d.loader);
    } catch (e) {
        return toast(String(e));
    }
    store.set('lastDir', dir.id);
    closeDrawer();
    S.inst = i.id;
    S.tab = 'overview';
    S.page = 'home';
    save();
    paintTop();
    paintRail();
    paintView('inst');
    paintLaunch(true);
    ensureVisible(i.id);
}

// 后端的任务进度：更新任务页、左栏的进度环和启动按钮
function onTask(snap) {
    const was = TASKS.find((x) => x.bid === snap.id)?.state;
    const t = real.upsertTask(snap);
    // 不是装实例的任务（装 Mod、补中文、建 Mod 组……）做完了说一声结果，不用跑去任务页看
    if (was === 'run' && snap.state !== 'running' && !real.R.taskOf.has(snap.id)) {
        if (snap.state === 'done') toast(snap.note ? `${snap.title}：${snap.note}` : `${snap.title}：好了`, { ms: snap.note ? 9000 : 4000 });
        else if (snap.state === 'failed') toast(`${snap.title}没成：${snap.error || '原因在「任务」页'}`, { ms: 9000 });
    }
    const iid = real.R.taskOf.get(snap.id);
    const i = iid && byId(iid);
    if (i) {
        if (snap.state === 'running') {
            i.installing = { pct: t.pct, stage: [t.stage || '准备中', t.speed].filter(Boolean).join(' · ') };
            refresh(i.id);
            if (S.inst === i.id && S.page === 'home' && S.tab === 'overview') paintBody();
        } else {
            real.R.taskOf.delete(snap.id);
            delete i.installing;
            if (snap.state === 'done') {
                toast(`「${i.name}」装好了`); // 后端随后会发 targets-dirty，那时重新扫描
                refresh(i.id, true);
            } else {
                INSTANCES.splice(INSTANCES.indexOf(i), 1);
                toast(snap.state === 'cancelled' ? `「${i.name}」取消了` : `「${i.name}」没装成，原因在「任务」页`);
                afterTargets();
            }
        }
    }
    paintTop();
    paintTasks();
}

function realToggleMod(i, m, on, input) {
    input.disabled = true;
    real.setMod(i, m, on).then(
        () => {
            input.closest('.row')?.classList.toggle('off', !on);
            const count = $('#modCount');
            if (count) count.textContent = modCount(i);
        },
        (e) => {
            input.checked = !on;
            toast(String(e));
        },
    ).finally(() => { input.disabled = false; });
}

// 实例设置的一项改了：更新内存里的一份，存到版本文件夹
async function saveCfg(t) {
    const i = cur();
    if (!i) return;
    const g = { ...(i.cfgReal || {}) };
    const v = t.value;
    if (t.dataset.rcfg === 'memory') g.memoryMb = v ? Number(v) : null;
    else if (t.dataset.rcfg === 'java') g.javaPath = v;
    else if (t.dataset.rcfg === 'jvm') g.jvmArgs = v.trim();
    else if (t.dataset.rcfg === 'gpu') g.gpuHigh = t.checked;
    else if (t.dataset.rcfg === 'prio') g.priorityHigh = t.checked;
    else if (t.dataset.rcfg === 'throttle') g.noThrottle = t.checked;
    else if (t.dataset.rcfg === 'window') {
        g.fullscreen = v === 'full';
        const [w, h] = v && v !== 'full' ? v.split('x').map(Number) : [null, null];
        g.width = w;
        g.height = h;
    }
    try {
        await real.invoke('set_instance_config', { dir: i.dir, name: i.name, cfg: g });
        i.cfgReal = g;
        toast('存好了，下次启动生效');
    } catch (e) {
        toast(String(e));
    }
}

// 设置页「启动」一栏
async function saveLaunchPrefs(patch) {
    const p = { ...real.R.prefs, ...patch };
    try {
        await real.invoke('set_launch_prefs', { prefs: { directLaunch: p.directLaunch, afterLaunch: p.afterLaunch } });
        real.R.prefs = p;
        if ('directLaunch' in patch) toast(p.directLaunch ? '直接启动开了：下次双击图标就直接进游戏，按住 Shift 双击才打开界面' : '直接启动关了');
    } catch (e) {
        toast(String(e));
    }
    if (S.page === 'settings') paintView();
}

// 直接启动出了事（没能启动、崩溃、找不到实例），或者从跳转列表启动：停到那个实例上
function onDirectNotice(p) {
    if (p.key && byId(p.key)) selectGo(p.key);
    if (p.msg) toast(p.msg, { ms: 8000 });
}

// ---------- 资源中心 ----------
async function resSearch(more = false) {
    const r = real.R.res;
    const t = resTarget(ctx());
    // 连着搜几次（比如默认的热门列表还没回来就搜了别的）：只认最后一次的结果
    const seq = (r.seq = (r.seq || 0) + 1);
    r.loading = true;
    r.error = '';
    if (!more) {
        r.hits = [];
        r.via = '';
        r.unfit = false;
    }
    const from = r.hits.length;
    paintView();
    try {
        if (r.source === 'curseforge' && !real.R.cf) {
            r.loading = false;
            return paintView();
        }
        const q = { source: r.source, text: r.q, kind: r.kind, sort: r.sort, offset: more ? r.hits.length : 0, mc: r.fit && t ? t.mc : null, loader: r.fit && t && r.kind === 'mod' ? (t.loader || '').toLowerCase() : null };
        const res = await real.invoke('content_search', { q });
        if (seq !== r.seq) return;
        r.hits = more ? [...r.hits, ...res.hits] : res.hits;
        r.total = res.total;
        if (!more) {
            r.via = res.via || '';
            r.unfit = !!res.unfit;
        }
    } catch (e) {
        if (seq !== r.seq) return;
        r.error = String(e);
    }
    r.loading = false;
    // 只有这一次重画带淡入，之后的重画不再播
    r.fresh = true;
    r.freshFrom = from;
    if (S.page === 'download') paintView();
    r.fresh = false;
}

async function resOpen(id) {
    const t = resTarget(ctx());
    const r = real.R.res;
    S.drawer = { kind: 'res', state: 'loading', id, versions: null };
    paintDrawer();
    try {
        const project = await real.invoke('content_project', { id, source: r.source });
        S.drawer = { ...S.drawer, state: 'ok', project };
        paintDrawer();
        const pack = r.kind === 'modpack';
        S.drawer.pack = pack;
        const versions = await real.invoke('content_versions', { id, source: r.source, mc: pack ? null : t?.mc || null, loader: !pack && r.kind === 'mod' ? (t?.loader || '').toLowerCase() || null : null });
        if (S.drawer?.id === id) {
            S.drawer.versions = versions;
            paintDrawer();
        }
    } catch (e) {
        S.drawer = { ...S.drawer, state: 'err', msg: String(e) };
        paintDrawer();
    }
}

async function resPlan(version) {
    const d = S.drawer;
    const t = resTarget(ctx());
    if (!t) return toast('先选一个实例');
    const back = { ...d };
    S.drawer = { ...d, state: 'loading' };
    paintDrawer();
    try {
        const plan = await real.invoke('content_plan', { dir: t.dir, name: t.name, kind: real.R.res.kind, project: d.id, version: version || null, source: real.R.res.source });
        S.drawer = { ...back, state: 'plan', plan };
    } catch (e) {
        S.drawer = { ...back, state: 'ok' };
        toast(String(e), { ms: 6000 });
    }
    paintDrawer();
}

// 整合包装成新实例：问名字，放进选好的游戏目录
async function packInstall(version) {
    const d = S.drawer;
    const dir = $('#packDir')?.value || DIRS.find((x) => x.access === 'direct')?.id;
    const name = (await askText('新实例叫什么：', d.project?.title || '整合包'))?.trim();
    if (!name || !dir) return;
    try {
        await real.invoke('pack_install_modrinth', { project: d.id, version: version || null, dir, name });
        closeDrawer();
        toast(`开始装「${name}」，进度在「任务」页`);
    } catch (e) {
        toast(String(e));
    }
}

async function resPrimary() {
    const d = S.drawer;
    if (d.pack && d.state === 'ok') return packInstall(null);
    if (d.state === 'ok') return resPlan(null);
    if (d.state !== 'plan') return;
    const t = resTarget(ctx());
    try {
        await real.invoke('content_install', { dir: t.dir, name: t.name, kind: real.R.res.kind, items: d.plan });
        closeDrawer();
        toast('开始装了，进度在「任务」页');
    } catch (e) {
        toast(String(e));
    }
}

async function saveNet(patch) {
    try {
        real.R.sources = await real.invoke('save_net_settings', { settings: { ...real.R.sources.settings, ...patch } });
    } catch (e) {
        toast(String(e));
    }
    if (S.page === 'settings') paintView();
}

function realCommands(q) {
    const i = cur();
    const out = [];
    const sub = (x) => `${x.mc} · ${x.loader || '原版'}`;
    if (i) out.push({ g: '常用', ic: 'play', t: `启动 ${i.name}`, k: 'Enter', run: () => launch() });
    out.push({ g: '账户', ic: 'user', t: '添加微软账户', run: () => msaStart() });
    out.push({ g: '账户', ic: 'user', t: '添加离线账户', run: () => offStart() });
    real.R.accounts.forEach((a) => a.id !== S.account && out.push({ g: '账户', ic: 'user', t: `切换到 ${a.name}`, sub: real.acctLabel(a), run: () => ACT['acct-switch']({ dataset: { id: a.id } }) }));
    out.push({ g: '常用', ic: 'plus', t: '新建实例', run: () => openDrawer() });
    INSTANCES.forEach((x) => out.push({ g: '实例', ic: 'layers', t: `打开 ${x.name}`, sub: sub(x), run: () => selectGo(x.id) }));
    if (i) {
        out.push({ g: '这个实例', ic: 'folder', t: `打开「${i.name}」的文件夹`, run: () => ACT.folder({ dataset: { v: '' } }) });
        if (i.loader) out.push({ g: '这个实例', ic: 'folder', t: `打开「${i.name}」的 mods 文件夹`, run: () => ACT.folder({ dataset: { v: 'mods' } }) });
        out.push({ g: '这个实例', ic: 'list', t: `看「${i.name}」的 Mod`, run: () => setTab('mods') });
        out.push({ g: '这个实例', ic: 'globe', t: `看「${i.name}」的存档`, run: () => setTab('worlds') });
    }
    out.push({ g: '页面', ic: 'download', t: '下载游戏版本', k: 'Ctrl 2', run: () => goPage('download') });
    out.push({ g: '页面', ic: 'list', t: '任务', k: 'Ctrl 3', run: () => goPage('tasks') });
    out.push({ g: '页面', ic: 'sliders', t: '设置', k: 'Ctrl 5', run: () => goPage('settings') });
    out.push({ g: '设置', ic: 'refresh', t: '重新扫描游戏目录', run: () => ACT['real-rescan']() });
    out.push({ g: '设置', ic: 'folder', t: '添加 .minecraft 文件夹', run: () => ACT['real-add-dir']() });
    [['system', '跟随系统'], ['light', '浅色'], ['dark', '深色']].forEach(([v, t]) => v !== S.theme && out.push({ g: '外观', ic: 'sun', t: `主题改成${t}`, run: () => setPref('theme', v) }));
    out.push({ g: '其他', ic: 'file', t: '关于 LeafStarter', sub: '版本、许可证、第三方许可', run: () => openDrawer({ kind: 'about' }) });
    plug?.palette().forEach((x) => out.push(x));
    return out;
}

if (REAL) {
    // 这些动作在真程序里照原样就能用；别的都先提示「还在做」
    const OK = new Set(['sel', 'tab', 'rail', 'pal', 'pop-close', 'new-inst', 'dr-mc', 'dr-loader', 'dr-create', 'launch', 'mods-f', 'set', 'about', 'run-log', 'log-f', 'log-copy', 'crash-view', 'crash-dismiss']);
    for (const k of Object.keys(ACT)) if (!OK.has(k)) ACT[k] = () => toast(real.NOT_YET);
    Object.assign(ACT, {
        acct: (el) => (pop?.anchor === el ? closePop() : openPop(el, realAcctPop(ctx()), 'above')),
        'acct-switch': async (el) => {
            closePop();
            try {
                real.applyAccounts(await real.invoke('set_default_account', { id: el.dataset.id }));
                afterAccounts(el.dataset.id);
            } catch (e) {
                toast(String(e));
            }
        },
        'acct-del': async (el) => {
            const a = real.R.accounts.find((x) => x.id === el.dataset.id);
            if (!a || !(await ask(`移除账户「${a.name}」？${a.kind === 'msa' ? '以后要用得重新登录。' : ''}`))) return;
            real.invoke('remove_account', { id: a.id }).then((list) => afterAccounts(real.applyAccounts(list)));
        },
        'acct-add-msa': () => msaStart(),
        'acct-add-off': () => offStart(),
        'acct-add-ygg': () => yggStart(),
        'ygg-preset': (el) => {
            const f = $('#yggServer');
            if (f) f.value = el.dataset.v;
            $('#yggUser')?.focus();
        },
        'ygg-pick': async (el) => {
            const d = S.drawer;
            try {
                yggDone(await real.invoke('ygg_pick', { ticket: d.ticket, profile: el.dataset.id }));
            } catch (e) {
                S.drawer = { ...d, state: 'form', err: String(e) };
                paintDrawer();
            }
        },
        'dr-close': () => {
            if (S.drawer?.kind === 'msa' && S.drawer.state === 'wait') real.invoke('msa_cancel');
            closeDrawer();
        },
        stop: () => {
            const r = RUN.get(S.inst);
            if (r?.run) real.stop(r.run);
        },
        'crash-fix-real': async () => {
            const i = cur();
            const fix = RUN.get(i?.id)?.crash?.diag?.fix;
            if (!fix) return;
            try {
                toast(await real.invoke('apply_fix', { dir: i.dir, name: i.name, fix }));
            } catch (e) {
                return toast(String(e));
            }
            RUN.delete(i.id);
            i.loaded = false;
            refresh(i.id, true);
            ensureDetails(i);
            realLaunch(i);
        },
        'inst-rename': async () => {
            const i = cur();
            const name = (await askText(`把「${i.name}」改名为：`, i.name))?.trim();
            if (!name || name === i.name) return;
            try {
                const n = await real.invoke('rename_instance', { dir: i.dir, name: i.name, newName: name });
                S.inst = `${i.dir}/${n}`;
                await realRescan(`改好了：${n}`);
            } catch (e) {
                toast(String(e));
            }
        },
        'inst-copy': async () => {
            const i = cur();
            const name = (await askText('复制出来的这份叫什么：', `${i.name} 的副本`))?.trim();
            if (!name) return;
            toast('正在复制…');
            try {
                const n = await real.invoke('copy_instance', { dir: i.dir, name: i.name, newName: name });
                S.inst = `${i.dir}/${n}`;
                await realRescan(`复制好了：${n}`);
            } catch (e) {
                toast(String(e));
            }
        },
        'inst-shortcut': async () => {
            const i = cur();
            try {
                await real.invoke('create_shortcut', { dir: i.dir, name: i.name });
                toast(`桌面上建好了「${i.name}」的快捷方式，双击就直接启动`);
            } catch (e) {
                toast(String(e));
            }
        },
        'lp-after': (el) => saveLaunchPrefs({ afterLaunch: el.dataset.v }),
        'inst-del': async () => {
            const i = cur();
            const shared = i.isolatedDir ? '它自己的存档、Mod 和配置也在里面，会一起放进去。' : '没开版本隔离，存档和 Mod 在游戏目录里，不会动它们。';
            if (!(await ask(`把「${i.name}」放进回收站？${shared}后悔了可以从回收站还原。`))) return;
            try {
                await real.invoke('delete_instance', { dir: i.dir, name: i.name });
                await realRescan(`「${i.name}」放进回收站了`);
            } catch (e) {
                toast(String(e));
            }
        },
        'crash-open': () => {
            const p = RUN.get(S.inst)?.crash?.report;
            if (p) real.invoke('open_path', { path: p }).catch((e) => toast(String(e)));
        },
        folder: (el) => {
            const i = cur();
            if (i) real.invoke('open_instance_dir', { dir: i.dir, name: i.name, sub: el.dataset.v || '' }).catch((e) => toast(String(e)));
        },
        'mg-switch': async (el) => {
            const i = cur();
            const g = i?.groups?.groups.find((x) => x.id === el.dataset.v);
            if (!g || g.active) return;
            if (isRunning(i)) return toast('游戏还开着，关掉再切 Mod 组');
            await mgDo(i, () => real.invoke('mod_group_switch', { dir: i.dir, name: i.name, id: g.id }), `换成「${g.name}」了`);
        },
        'mg-new': async () => {
            const i = cur();
            if (!i?.groups) return;
            const name = await askText('新的 Mod 组叫什么？', `Mod 组 ${i.groups.groups.length + 1}`, { ok: '下一步' });
            if (name == null || !name.trim()) return;
            const copy = await ask(`「${name.trim()}」从哪开始？\n复制一份：现在这组的 Mod 都带过去（不多占硬盘）。\n空的：一个 Mod 都没有，自己往里加。`, { ok: '复制现在这组', cancel: '空的' });
            let id;
            try {
                id = await real.invoke('mod_group_create', { dir: i.dir, name: i.name, group: name.trim(), copy });
            } catch (e) {
                return toast(String(e));
            }
            if (isRunning(i)) {
                await mgDo(i, null, `建好了，关掉游戏后在上面点它切过去`);
                return;
            }
            await mgDo(i, () => real.invoke('mod_group_switch', { dir: i.dir, name: i.name, id }), `建好了，已经换成「${name.trim()}」`);
        },
        'mg-rename': async (el) => {
            const i = cur();
            const g = i?.groups?.groups.find((x) => x.id === el.dataset.v);
            if (!g) return;
            const name = await askText('改成什么名字？', g.name);
            if (name == null || !name.trim() || name.trim() === g.name) return;
            await mgDo(i, () => real.invoke('mod_group_rename', { dir: i.dir, name: i.name, id: g.id, group: name.trim() }), '改好了');
        },
        'mg-del': async (el) => {
            const i = cur();
            const g = i?.groups?.groups.find((x) => x.id === el.dataset.v);
            if (!g || !(await ask(`删掉 Mod 组「${g.name}」？里面的 ${g.count} 个 Mod 文件进回收站。`, { ok: '删掉', danger: true }))) return;
            await mgDo(i, () => real.invoke('mod_group_delete', { dir: i.dir, name: i.name, id: g.id }), '删掉了，文件在回收站');
        },
        'real-rescan': () => realRescan('扫描完了'),
        'tmpl-apply': async () => {
            const i = cur();
            const from = byId(document.getElementById('tmplFrom')?.value);
            const groups = [...document.querySelectorAll('[data-tg]')].filter((x) => x.checked).map((x) => x.dataset.tg);
            if (!from || !groups.length) return toast('选一个实例，再勾上要套哪些');
            if (isRunning(i)) return toast('游戏退出时会覆盖设置，先关掉游戏再套');
            try {
                const r = await real.invoke('apply_options', { dir: i.dir, name: i.name, fromDir: from.dir, fromName: from.name, groups });
                toast(`从「${from.name}」套用了 ${r.count} 项${r.skipped.length ? `，${r.skipped.length} 个键位这个版本没有，跳过了` : ''}。下次启动生效`, { ms: 6000 });
            } catch (e) {
                toast(String(e));
            }
        },
        'bisect-start': async () => {
            const i = cur();
            try {
                const e = await real.invoke('bisect_estimate', { dir: i.dir, name: i.name });
                if (!(await ask(`会自动启动大约 ${e.launches} 次游戏，每次只开 ${e.mods} 个 Mod 里的一部分，看启动时崩不崩。\n\n游戏窗口会自己弹出来又关掉，期间别动它；只查进主菜单之前的崩溃。查完 Mod 的开关会还原。\n\n开始吗？`))) return;
                RUN.delete(i.id);
                real.R.bisect.set(i.id, { phase: 'starting', key: i.id });
                refresh(i.id, true);
                await real.invoke('bisect_start', { dir: i.dir, name: i.name, key: i.id });
            } catch (err) {
                real.R.bisect.delete(i.id);
                refresh(i.id, true);
                toast(String(err));
            }
        },
        'bisect-cancel': () => {
            real.invoke('bisect_cancel', { key: cur().id });
            toast('等这一次启动结束就停下，然后还原 Mod 开关');
        },
        'bisect-dismiss': () => {
            real.R.bisect.delete(cur().id);
            refresh(cur().id, true);
        },
        'bisect-disable': async (el) => {
            const i = cur();
            const m = i.mods.find((x) => x.file === el.dataset.v);
            if (!m) return toast('找不到这个文件了，重新扫描一下');
            try {
                await real.setMod(i, m, false);
                toast(`禁用了「${m.name}」，再启动试试`);
                real.R.bisect.delete(i.id);
                refresh(i.id, true);
            } catch (err) {
                toast(String(err));
            }
        },
        'mod-links': (el) => {
            const m = cur()?.mods.find((x) => x.id === el.dataset.id);
            if (!m) return;
            if (pop?.anchor === el) return closePop();
            openPop(el, modLinksPop(m), 'below');
        },
        'open-url': (el) => {
            closePop();
            real.invoke('open_path', { path: el.dataset.v }).catch((err) => toast(String(err)));
        },
        'world-hist': async (el) => {
            const i = cur();
            const w = i.worlds.find((x) => x.folder === el.dataset.v);
            if (!w) return;
            if (pop?.anchor === el) return closePop();
            openPop(el, worldHistPop(w, null), 'below');
            try {
                const list = await real.invoke('world_history', { dir: i.dir, name: i.name, world: w.folder });
                if (pop?.anchor === el) openPop(el, worldHistPop(w, list), 'below');
            } catch (e) {
                toast(String(e));
            }
        },
        'world-snap': async (el) => {
            const i = cur();
            const w = i.worlds.find((x) => x.folder === el.dataset.v);
            closePop();
            toast(`正在给「${w.name}」存一份…`);
            try {
                await real.invoke('world_snapshot', { dir: i.dir, name: i.name, world: w.folder });
                toast(`「${w.name}」存好了`);
            } catch (e) {
                toast(String(e));
            }
        },
        'world-restore': async (el) => {
            const i = cur();
            const w = i.worlds.find((x) => x.folder === el.dataset.v);
            closePop();
            if (!(await ask(`把「${w.name}」退回到这一份？现在的样子会先存一份，后悔了还能再退回来。`))) return;
            try {
                await real.invoke('world_restore', { dir: i.dir, name: i.name, world: w.folder, id: el.dataset.id });
                toast(`「${w.name}」退回好了`);
                i.loaded = false;
                ensureDetails(i);
            } catch (e) {
                toast(String(e));
            }
        },
        'lan-code': async (el) => {
            try {
                const p = await real.invoke('lan_pair_code');
                real.R.lan = await real.invoke('lan_status');
                openPop(el, lanCodePop(p), 'above');
            } catch (e) {
                toast(String(e));
            }
        },
        'lan-copy': (el) => navigator.clipboard?.writeText(el.dataset.v).then(() => toast('配对码复制好了，到另一台电脑上点「输入配对码」粘贴'), () => toast('复制不了，选中那串字后按 Ctrl C')),
        'lan-join': async () => {
            const code = (await askText('粘贴另一台设备上显示的配对码（LS1. 开头）：'))?.trim();
            if (!code) return;
            toast('正在配对…');
            try {
                const r = await real.invoke('lan_pair', { code });
                real.R.lan = r.status;
                paintView();
                toast(`配好了：${r.name}`);
            } catch (e) {
                toast(String(e), { ms: 8000 });
            }
        },
        'lan-forget': async (el) => {
            if (!(await ask('移除这台设备？以后要再同步得重新配对。'))) return;
            real.R.lan = await real.invoke('lan_forget', { id: el.dataset.id }).catch((e) => (toast(String(e)), real.R.lan));
            paintView();
        },
        'lan-rename': async () => {
            const name = (await askText('这台设备叫什么：', real.R.lan?.me.name || ''))?.trim();
            if (!name) return;
            try {
                real.R.lan = await real.invoke('lan_rename', { name });
                paintView();
            } catch (e) {
                toast(String(e));
            }
        },
        'lan-browse': async (el) => {
            const peer = el.dataset.id;
            const peerName = real.R.lan?.peers.find((p) => p.id === peer)?.name || '';
            S.drawer = { kind: 'lan', state: 'loading', peer, peerName, list: [] };
            paintDrawer();
            try {
                const info = await real.invoke('lan_remote', { peer });
                S.drawer = { ...S.drawer, state: 'ok', peerName: info.name, list: info.instances };
            } catch (e) {
                S.drawer = { ...S.drawer, state: 'err', msg: String(e) };
            }
            paintDrawer();
        },
        'lan-pull': async (el) => {
            const d = S.drawer;
            const x = d.list.find((y) => y.key === el.dataset.v);
            const dir = $('#lanDir')?.value;
            if (!x || !dir) return;
            const name = (await askText('在这台电脑上叫什么：', x.name))?.trim();
            if (!name) return;
            try {
                await real.invoke('lan_pull', { peer: d.peer, remote: x.key, dir, name });
                closeDrawer();
                toast(`开始拿「${x.name}」，进度在「任务」页`);
            } catch (e) {
                toast(String(e));
            }
        },
        'lan-sync': async (el) => {
            const i = cur();
            const w = i.worlds.find((y) => y.folder === el.dataset.v);
            const run = async (prefer) => real.invoke('lan_sync_world', { dir: i.dir, name: i.name, world: w.folder, prefer });
            toast(`正在同步「${w.name}」…`);
            try {
                let r = await run(null);
                if (r.outcome.kind === 'conflict') {
                    const mine = (await ask(`「${w.name}」两边都改过。\n\n点「确定」用这台电脑上的，点「取消」用「${i.link.peerName || '另一台设备'}」上的。\n没选上的那份会先存进历史，不会丢。`));
                    r = await run(mine ? 'mine' : 'theirs');
                }
                const o = r.outcome;
                toast(o.kind === 'same' ? `「${w.name}」两边一样，不用同步` : r.direction === 'push' ? `「${w.name}」传过去了` : `「${w.name}」拿过来了`);
                i.loaded = false;
                ensureDetails(i);
            } catch (e) {
                toast(String(e), { ms: 8000 });
            }
        },
        'res-kind': (el) => {
            const r = real.R.res;
            r.kind = el.dataset.v;
            r.hits = [];
            r.total = 0;
            if (r.kind === 'game') return paintView();
            resSearch();
        },
        'res-src': (el) => {
            real.R.res.source = el.dataset.v;
            resSearch();
        },
        'res-sort': (el) => {
            real.R.res.sort = el.dataset.v;
            resSearch();
        },
        'res-more': () => resSearch(true),
        'res-open': (el) => resOpen(el.dataset.id),
        'res-plan': (el) => resPlan(el.dataset.v),
        'pack-ver': (el) => packInstall(el.dataset.v),
        'pack-file': async () => {
            const path = await real.invoke('pack_pick').catch((e) => (toast(String(e)), null));
            if (!path) return;
            try {
                const p = await real.invoke('pack_read', { path });
                const fmt = { mrpack: 'Modrinth', curseforge: 'CurseForge', multimc: 'MultiMC / Prism', mcbbs: 'MCBBS / HMCL' }[p.format] || p.format;
                const name = (await askText(`「${p.name}」${p.version ? ` ${p.version}` : ''}（${fmt} 整合包）\n游戏 ${p.mc}${p.loader ? ` · ${p.loader[0]} ${p.loader[1]}` : ''} · 要下载 ${p.files} 个文件${p.manual ? `，${p.manual} 个要自己下` : ''}\n\n新实例叫什么：`, p.name))?.trim();
                if (!name) return;
                const dir = $('#packDir')?.value || DIRS.find((x) => x.access === 'direct')?.id;
                await real.invoke('pack_install', { path, dir, name });
                toast(`开始装「${name}」，进度在「任务」页`);
            } catch (e) {
                toast(String(e), { ms: 8000 });
            }
        },
        'pack-export': async () => {
            const i = cur();
            const saves = (await ask('导出的整合包里带上存档吗？\n\n点「确定」带上，点「取消」不带（发给别人时一般不带）。'));
            toast('正在导出…');
            try {
                const r = await real.invoke('pack_export', { dir: i.dir, name: i.name, saves });
                toast(`导出好了，放在桌面：${r.path}。Modrinth 认得的 ${r.linked} 个 Mod 只写了下载地址，另外 ${r.bundled} 个直接打包进去了`, { ms: 9000 });
            } catch (e) {
                toast(String(e));
            }
        },
        'res-go': () => {
            const r = real.R.res;
            r.kind = 'mod';
            r.inst = S.inst;
            goPage('download');
            resSearch();
        },
        'zh-real': () => {
            const i = cur();
            if (real.R.zh.has(i.id)) {
                real.R.zh.delete(i.id);
                return paintBody();
            }
            zhScan(i);
        },
        'zh-run': async () => {
            const i = cur();
            const opts = { community: !!$('#zhCom')?.checked, ai: !!$('#zhAi')?.checked, files: [] };
            if (!opts.community && !opts.ai) return toast('两个都没选');
            try {
                await real.invoke('i18n_run', { dir: i.dir, name: i.name, opts });
                toast('开始补中文，进度在「任务」页');
            } catch (e) {
                toast(String(e));
            }
        },
        'zh-remove': async () => {
            const i = cur();
            if (!(await ask('拿掉「LeafStarter 汉化」资源包？文件夹放进回收站，游戏里的设置也去掉。'))) return;
            await real.invoke('i18n_remove', { dir: i.dir, name: i.name }).catch((e) => toast(String(e)));
            zhScan(i);
        },
        'mod-check': async () => {
            const i = cur();
            real.R.updates.set(i.id, { loading: true });
            paintBody();
            try {
                real.R.updates.set(i.id, await real.invoke('mod_updates', { dir: i.dir, name: i.name }));
            } catch (e) {
                real.R.updates.set(i.id, { error: String(e) });
            }
            if (cur() === i) paintBody();
        },
        'mod-up-one': async (el) => {
            const i = cur();
            const x = real.R.updates.get(i.id)?.updates[Number(el.dataset.k)];
            if (!x) return;
            await real.invoke('content_install', { dir: i.dir, name: i.name, kind: 'mod', items: [x.item] }).catch((e) => toast(String(e)));
            toast(`开始更新「${x.title}」`);
        },
        'mod-up-all': async () => {
            const i = cur();
            const u = real.R.updates.get(i.id);
            if (!u?.updates.length) return;
            await real.invoke('content_install', { dir: i.dir, name: i.name, kind: 'mod', items: u.updates.map((x) => x.item) }).catch((e) => toast(String(e)));
            toast(`开始更新 ${u.updates.length} 个 Mod，进度在「任务」页`);
            real.R.updates.delete(i.id);
        },
        // 概览卡片上的几个按钮
        // 存档「进入」：启动后直接进这个世界
        'world-enter': (el) => realLaunch(cur(), `world:${el.dataset.v}`),
        // 删 Mod：放进回收站，能找回来，所以不再问一遍
        'mod-trash': async (el) => {
            const i = cur();
            const m = i?.mods.find((x) => x.id === el.dataset.id);
            if (!m) return;
            try {
                await real.invoke('mod_trash', { dir: i.dir, name: i.name, file: m.file });
                i.mods.splice(i.mods.indexOf(m), 1);
                paintBody();
                toast(`「${m.name}」放进回收站了`);
            } catch (e) {
                toast(String(e));
            }
        },
        'perf-open': () => {
            setTab('config');
            ACT['perf-load']();
        },
        'gpu-high': () => saveCfg({ dataset: { rcfg: 'gpu' }, checked: true, value: '' }).then(() => paintBody()),
        'mem-auto': (el) => el.getAttribute('aria-pressed') !== 'true' && saveCfg({ dataset: { rcfg: 'memory' }, value: '' }).then(() => paintBody()),
        'zh-look': () => zhScan(cur()),
        'perf-load': async () => {
            const i = cur();
            i.perf = { loading: true };
            paintBody();
            try {
                i.perf = await real.invoke('perf_status', { dir: i.dir, name: i.name });
            } catch (e) {
                i.perf = { error: String(e) };
            }
            if (cur() === i) paintBody();
        },
        'perf-apply': async () => {
            const i = cur();
            const slugs = [...document.querySelectorAll('[data-pf]')].filter((x) => x.checked).map((x) => x.dataset.pf);
            if (!slugs.length) return toast('先勾上要加的');
            try {
                await real.invoke('perf_apply', { dir: i.dir, name: i.name, slugs });
                i.perf = null;
                paintBody();
                toast('开始加了，进度在「任务」页');
            } catch (e) {
                toast(String(e));
            }
        },
        'perf-remove': async () => {
            const i = cur();
            if (!(await ask('把之前加的优化 Mod 放进回收站？别的 Mod 不动。'))) return;
            try {
                const n = await real.invoke('perf_remove', { dir: i.dir, name: i.name });
                toast(`撤掉了 ${n} 个`);
                i.perf = null;
                i.loaded = false;
                ensureDetails(i);
            } catch (e) {
                toast(String(e));
            }
        },
        'seed-copy': (el) => navigator.clipboard?.writeText(el.dataset.v).then(() => toast(`种子 ${el.dataset.v} 复制好了`), () => toast('复制不了，选中按钮上的数字后按 Ctrl C')),
        'real-add-dir': async () => {
            try {
                const path = await real.invoke('pick_folder');
                if (!path) return;
                real.applyTargets(await real.invoke('add_manual_dir', { path }));
                afterTargets();
                toast('加好了');
            } catch (e) {
                toast(String(e));
            }
        },
        'dir-open': (el) => {
            const d = DIRS.find((x) => x.id === el.dataset.id);
            if (d) real.invoke('open_path', { path: d.path }).catch((e) => toast(String(e)));
        },
        'dir-del': async (el) => {
            try {
                real.applyTargets(await real.invoke('remove_target', { id: el.dataset.id }));
                afterTargets();
                toast('移除了（文件夹本身没动）');
            } catch (e) {
                toast(String(e));
            }
        },
        'java-refresh': async () => {
            await real.reloadJava();
            if (S.page === 'settings') paintView();
            toast('检测完了');
        },
        'data-open': () => real.invoke('open_path', { path: real.R.info.dataDir }).catch((e) => toast(String(e))),
        // 从下载页进来默认装原版：老版本、快照多半没有加载器，要的话再自己选
        'dl-mc': (el) => openDrawer({ mc: el.dataset.v, loader: null }),
        dlv: (el) => {
            S.dlv = el.dataset.v;
            S.dlvq = '';
            paintView();
        },
        'dl-retry': () => real.loadManifest(true),
        'tasks-clear': async () => {
            await real.clearTasks().catch((e) => toast(String(e)));
            paintTasks();
            paintTop();
        },
        'rs-mode': (el) => saveNet({ mode: el.dataset.v }),
        'rs-del': (el) => saveNet({ custom: real.R.sources.settings.custom.filter((x) => x.id !== el.dataset.id) }),
        'rs-test': async () => {
            const b = $('#rsTest');
            if (b) {
                b.disabled = true;
                b.lastChild.textContent = '测速中…';
            }
            real.R.sources = await real.invoke('test_sources').catch(() => real.R.sources);
            if (S.page === 'settings') paintView();
        },
        'rs-add': () => {
            const name = $('#rsName').value.trim() || '自定义镜像';
            const base = $('#rsBase').value.trim().replace(/\/+$/, '');
            if (!/^https?:\/\//.test(base)) return toast('镜像地址要以 http:// 或 https:// 开头');
            saveNet({ custom: [...(real.R.sources.settings.custom || []), { id: 'custom-' + Date.now(), name, kind: 'bmclapi', base, builtin: false, enabled: true }] });
            toast('加好了，记得测速');
        },
    });
    // AI 助手和设置里的 AI 源、MCP（M9）；常驻服务器（M10）
    Object.assign(ACT, agent.actions, aiSet.actions, servers.actions, skin.actions, wtool.actions, edition.actions, plug.actions, mpr.actions, upd.actions);
}

// 每个 Mod 的中文情况（M11）
async function zhScan(i) {
    real.R.zh.set(i.id, { loading: true });
    if (cur() === i) paintBody();
    try {
        real.R.zh.set(i.id, await real.invoke('i18n_scan', { dir: i.dir, name: i.name }));
    } catch (e) {
        real.R.zh.set(i.id, { error: String(e) });
    }
    if (cur() === i && (S.tab === 'mods' || S.tab === 'overview')) paintBody();
}

// ---------- 事件 ----------
document.addEventListener('click', (e) => {
    const p = e.target.closest('[data-pal]');
    if (p) return runPal(Number(p.dataset.pal));
    if (e.target.id === 'palette') return closePalette();
    const el = e.target.closest('[data-act], [data-page]');
    if (!el || el.disabled) return;
    if (el.dataset.act) ACT[el.dataset.act]?.(el, e);
    else goPage(el.dataset.page);
});

document.addEventListener('pointerdown', (e) => {
    if (pop && !pop.el.contains(e.target) && !pop.anchor.contains(e.target)) closePop(false);
});

$('#palList').addEventListener('pointermove', (e) => {
    const b = e.target.closest('[data-pal]');
    if (!b || Number(b.dataset.pal) === pal.active) return;
    pal.active = Number(b.dataset.pal);
    markPal();
});

// Mod 图标：原图不大于 32 像素的是像素画，放大时保持方块感
document.addEventListener('load', (e) => {
    const t = e.target;
    if (t.classList?.contains('mic') && t.naturalWidth && t.naturalWidth <= 32) t.classList.add('px');
}, true);

document.addEventListener('change', (e) => {
    const t = e.target;
    if (REAL && (aiSet.change(t) || servers.change(t) || plug.change(t) || upd.change(t))) return;
    if (REAL && t.id === 'rsPref') return saveNet({ preferred: t.value });
    if (REAL && t.id === 'lpDirect') return saveLaunchPrefs({ directLaunch: t.checked });
    if (REAL && t.id === 'resQ') {
        real.R.res.q = t.value;
        return resSearch();
    }
    if (REAL && t.id === 'resInst') {
        real.R.res.inst = t.value;
        return resSearch();
    }
    if (REAL && t.id === 'resFit') {
        real.R.res.fit = t.checked;
        return resSearch();
    }
    if (REAL && t.id === 'lanOn') return real.invoke('lan_enable', { on: t.checked }).then((s) => { real.R.lan = s; paintView(); }, (e) => { toast(String(e)); paintView(); });
    if (REAL && t.dataset.rcfg) return saveCfg(t);
    if (REAL && t.id === 'rsThreads') return saveNet({ threads: Number(t.value) });
    if (t.dataset.hf) return mp.drawerInput(t);
    if (skin.change(t) || tools.change(t) || aisrc.change(t) || plugins.change(t)) return;
    if (t.dataset.wd) return world.wdChange(t);
    if (t.dataset.mod) return toggleMod(t.dataset.mod, t.checked, t);
    if (t.id === 'syncSwitch') return setSync(t.checked);
    if (t.dataset.cfg) return setCfg(t);
    if (t.dataset.g) {
        GLOBAL[t.dataset.g] = t.value;
        return;
    }
    if (t.dataset.set) return setPref(t.dataset.set, t.checked);
    if (t.id === 'drPerf') S.drawer.perf = t.checked;
    else if (t.id === 'drSync') S.drawer.sync = t.checked;
    else if (t.id === 'drDir') S.drawer.dir = t.value;
    else if (t.id === 'dlTarget') {
        S.dl.target = t.value;
        dlSearch();
    }
});

document.addEventListener('input', (e) => {
    const t = e.target;
    if (REAL && aboutInput(t)) return;
    if (REAL && t.id === 'dlVq') {
        S.dlvq = t.value;
        const l = $('#dlList');
        if (l) l.innerHTML = realDlRows(ctx());
        return;
    }
    if (REAL && aiSet.input(t)) return;
    if (REAL && t.id === 'offName' && S.drawer?.kind === 'off') {
        S.drawer.name = t.value;
        return;
    }
    if (REAL && t.id === 'rsThreads') {
        const v = $('#rsThreadsVal');
        if (v) v.textContent = t.value;
        return;
    }
    // 拖内存滑块：数字跟着变，「自动」松开（松手时才存）
    if (REAL && t.id === 'memRange') {
        const v = $('#memVal');
        if (v) v.textContent = gbOf(Number(t.value));
        const box = t.closest('.memctl');
        box?.classList.remove('auto');
        const b = box?.querySelector('[data-act="mem-auto"]');
        b?.classList.remove('on');
        b?.setAttribute('aria-pressed', 'false');
        return;
    }
    if (t.dataset.hf && t.type !== 'checkbox') return mp.drawerInput(t);
    if (t.type === 'range' && t.dataset.ais) {
        const b = document.querySelector(`[data-rv="${t.dataset.ais}:${t.dataset.f.slice(4)}"]`);
        if (b) b.textContent = t.value;
    }
    if (aisrc.input(t) || tools.input(t) || plugins.input(t)) return;
    if (t.id === 'railFilter') {
        S.filter = t.value;
        const ids = visibleIds(ctx());
        if (ids.length && !ids.includes(S.inst)) select(ids[0]);
        paintRail();
    } else if (t.id === 'palInput') {
        pal.active = 0;
        renderPalette();
    } else if (t.id === 'modQ') {
        S.modQ = t.value;
        $('#modRows').innerHTML = modRows(ctx());
    } else if (t.id === 'dlQ') {
        S.dl.q = t.value;
        dlSearch();
    } else if (t.id === 'drName') {
        S.drawer.name = t.value;
        S.drawer.touched = true;
        const b = $('#drCreate');
        if (b) b.disabled = !t.value.trim() || (S.drawer.loader && loaderVersion(S.drawer.loader, S.drawer.mc) === null);
    } else if (t.id === 'drVq') {
        S.drawer.vq = t.value;
        $('#drVlist').innerHTML = vlist(ctx());
    } else if (t.id === 'setThreads') {
        $('#threadsVal').textContent = t.value;
    }
});

$('#palInput').addEventListener('keydown', (e) => {
    const mod = e.ctrlKey || e.metaKey;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        if (!pal.list.length) return;
        pal.active = (pal.active + (e.key === 'ArrowDown' ? 1 : -1) + pal.list.length) % pal.list.length;
        markPal();
    } else if (e.key === 'Enter') {
        e.preventDefault();
        runPal(pal.active);
    } else if (e.key === 'Escape' || (mod && e.key.toLowerCase() === 'k')) {
        e.preventDefault();
        closePalette();
    } else return;
    e.stopPropagation();
});

const typing = (el) => !!el && ((el.tagName === 'INPUT' && !['checkbox', 'radio', 'range', 'button'].includes(el.type)) || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT' || el.isContentEditable);

document.addEventListener('keydown', (e) => {
    if (!$('#palette').hidden) return;
    if (REAL && servers.keydown(e)) return;
    const a = document.activeElement;
    const mod = e.ctrlKey || e.metaKey;
    const key = e.key.toLowerCase();
    if (mod && key === 'k') {
        e.preventDefault();
        return openPalette();
    }
    if (e.key === 'Escape') {
        if (closePop() || cancelDirect() || mp.closeAsk() || closeDrawer() || agent.close()) return e.preventDefault();
        if (a?.id === 'railFilter') {
            e.preventDefault();
            if (S.filter) {
                a.value = '';
                S.filter = '';
                paintRail();
            } else a.blur();
            return;
        }
        return toggleRail(false);
    }
    if ((mod || e.altKey) && /^[1-5]$/.test(e.key)) {
        e.preventDefault();
        return goPage(PAGES[Number(e.key) - 1]);
    }
    if (mod && key === 'z' && !typing(a)) {
        if (skin.undo() || undoLast()) e.preventDefault();
        return;
    }
    if (mod || e.altKey) return;
    const inFilter = a?.id === 'railFilter';
    if (typing(a) && !inFilter) {
        if (e.key !== 'Enter') return;
        if (a.id === 'agInput' && !e.shiftKey) {
            e.preventDefault();
            agent.send();
        } else if (a.id === 'skinDesc' && !e.shiftKey) {
            e.preventDefault();
            ACT['sk-gen']();
        } else if (a.id === 'wcAsk') {
            e.preventDefault();
            ACT['wc-ask']();
        } else if (a.id === 'wcCmd') {
            e.preventDefault();
            const v = a.value.trim();
            if (v) {
                a.value = '';
                world.console.exec([v.startsWith('/') ? v : '/' + v]);
            }
        } else if (a.id === 'srvCmd') {
            e.preventDefault();
            mp.consoleCmd(a);
        } else if (a.tagName === 'INPUT' && a.id !== 'drVq' && a.closest('#drawer')) {
            e.preventDefault();
            drawerPrimary();
        }
        return;
    }
    if (e.key === '/' && !inFilter) {
        e.preventDefault();
        $('#railFilter').focus();
        $('#railFilter').select();
        return;
    }
    const idle = !a || a === document.body;
    if (S.drawer) {
        // 侧边面板开着：Enter 就是「创建并安装」（焦点在按钮上时让按钮自己处理）
        if (e.key === 'Enter' && (idle || !a.closest('button, select, input'))) {
            e.preventDefault();
            drawerPrimary();
        }
        return;
    }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        const onItem = a?.classList?.contains('item');
        if (!(idle || inFilter || onItem)) return;
        e.preventDefault();
        return moveSel(e.key === 'ArrowDown' ? 1 : -1, onItem);
    }
    if (e.key === 'Enter' && (idle || inFilter || a?.classList?.contains('item'))) {
        e.preventDefault();
        primary();
    }
});

// 实时刷新：窗口在前台时每 4 秒看一眼游戏目录和选中的实例有没有变（在资源管理器里放了 Mod、别的启动器装了版本……），
// 变了就重新扫描或重读；从别的窗口切回来时马上看一次
const watch = { targets: null, inst: null, key: '', busy: false };
async function watchTick() {
    if (!REAL || watch.busy || document.visibilityState !== 'visible') return;
    // 有任务在跑时它们自己会通知刷新，别跟着抢
    if (TASKS.some((t) => t.state === 'run')) return;
    const i = cur();
    const key = i?.real ? i.id : '';
    watch.busy = true;
    try {
        const sig = await real.invoke('watch_sig', { dir: i?.real ? i.dir : null, name: i?.real ? i.name : null });
        if (watch.targets !== null && sig.targets !== watch.targets) {
            watch.targets = sig.targets;
            watch.inst = sig.inst;
            watch.key = key;
            await realRescan();
            return;
        }
        watch.targets = sig.targets;
        if (watch.key === key && watch.inst !== null && sig.inst !== watch.inst && i && i.loaded && !i.loading && !RUN.get(i.id)) {
            i.loaded = false;
            ensureDetails(i);
            if (real.R.zh.has(i.id)) zhScan(i);
        }
        watch.inst = sig.inst;
        watch.key = key;
    } catch {
    } finally {
        watch.busy = false;
    }
}
if (REAL) {
    setInterval(watchTick, 4000);
    window.addEventListener('focus', () => watchTick());
    document.addEventListener('visibilitychange', () => watchTick());
}

// 运行中每秒更新一次计时
setInterval(() => {
    const r = RUN.get(S.inst);
    if (r?.phase !== 'running') return;
    paintLaunch();
    const rc = $('#runClock');
    if (rc) rc.textContent = clock(Math.floor((Date.now() - r.started) / 1000));
}, 1000);

let resizeTimer;
window.addEventListener('resize', () => {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
        paintScenes();
        placeInd($('#navInd'), $(`#pages [data-page="${S.page}"]`), false);
        placeInd($('#tabInd'), $('#tabs .on'), false);
        placeRailSel(false);
    }, 120);
});

// ---------- 开始 ----------
hydrate(document);
if (S.theme !== 'system') document.documentElement.dataset.theme = S.theme;
document.documentElement.dataset.motion = S.motion;
$('#app').classList.toggle('simple', !!S.simple);
$('#app').classList.toggle('real', REAL);
if (REAL) {
    S.account = real.R.defaultAccount;
    if (!byId(S.inst)) S.inst = INSTANCES[0]?.id || null;
    if (!['home', 'download', 'tasks', 'settings', 'plugins', 'skin'].includes(S.page)) S.page = 'home';
}
if (REAL) applyEdition(api);
plugins.applyLook();
paintTop();
placeInd($('#navInd'), $(`#pages [data-page="${S.page}"]`), false);
paintRail();
paintAcct();
paintView('page');
paintLaunch(true);
if (S.inst) ensureVisible(S.inst);
// 第一屏画好了才把窗口拿出来（窗口建好时是藏着的），带上界面底色，拿出来那一帧不闪白
performance.mark('first-paint-ready');
if (REAL) real.invoke('ui_ready', { bg: getComputedStyle(document.body).backgroundColor.match(/\d+/g)?.slice(0, 3).map((n) => (+n).toString(16).padStart(2, '0')).join('') || null }).then((m) => { window.__lsStartup = m; }).catch(() => {});

if (REAL) {
    real.hooks.changed = (what, arg) => {
        if (what === 'targets') return afterTargets();
        if ((what === 'loader' || what === 'versions') && S.drawer?.kind === 'new') paintDrawer();
        if (what === 'versions' && S.page === 'download') paintView();
        if ((what === 'sources' || what === 'prefs') && S.page === 'settings') paintView();
        if (what === 'worlds' && S.page === 'home' && S.tab === 'worlds' && cur() === arg) paintBody();
        if (what === 'mods' && S.page === 'home' && (S.tab === 'mods' || S.tab === 'overview') && cur() === arg) paintBody();
        if (what === 'logs' && S.page === 'home' && S.tab === 'logs' && cur() === arg) paintBody();
    };
    real.listen('task-update', (e) => onTask(e.payload));
    real.listen('targets-dirty', () => realRescan());
    real.listen('sources-updated', () => real.loadSources());
    real.listen('launch-state', (e) => onLaunchState(e.payload));
    real.listen('game-log', (e) => onGameLog(e.payload));
    real.listen('msa-result', (e) => onMsaResult(e.payload));
    real.listen('direct-notice', (e) => onDirectNotice(e.payload));
    real.listen('content-installed', (e) => {
        const i = byId(`${e.payload.dir}/${e.payload.name}`);
        if (i) {
            i.loaded = false;
            ensureDetails(i);
        }
    });
    real.listen('i18n-done', (e) => {
        const i = byId(`${e.payload.dir}/${e.payload.name}`);
        if (i) zhScan(i);
    });
    real.listen('lan-changed', async () => {
        real.R.lan = await real.invoke('lan_status').catch(() => real.R.lan);
        if (S.page === 'settings') paintView();
        if (pop && $('.lan-qr')) closePop();
    });
    real.listen('bisect-state', (e) => {
        const p = e.payload;
        real.R.bisect.set(p.key, p);
        const i = byId(p.key);
        if (p.phase !== 'running' && i) {
            // Mod 开关还原了，重新读
            i.loaded = false;
            ensureDetails(i);
            toast(p.msg || '查完了', { ms: 8000 });
        }
        refresh(p.key, true);
    });
    // 界面加载完之前就开始的启动（直接启动、跳转列表）：补上状态
    real.invoke('run_states').then((list) => list.forEach((p) => {
        if (p.phase === 'failed') toast(`没能启动：${p.detail}`, { ms: 8000 });
        else onLaunchState(p);
    }));
    real.invoke('direct_notice').then((p) => p && onDirectNotice(p));
    // 插件：读列表和贡献；后台拉一次市场索引，顺便按吊销名单停用
    plug.load().then(() => real.invoke('plugin_index').then(() => plug.load(), () => {}));
    ensureDetails(cur());
    if (real.R.info?.importedFrom) toast('已经从方块补给搬来下载源设置和手动添加的目录');
}
