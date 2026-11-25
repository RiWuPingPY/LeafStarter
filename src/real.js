// 真程序的数据层。
// 在 LeafStarter 里运行时：用后端扫到的游戏目录、版本、Mod、存档、任务和 Java 换掉 data.js 里的示例数据。
// 在浏览器里直接打开（线上原型、官网演示）时：REAL 是 false，这里什么都不做，界面照旧用示例数据。
import { DIRS, INSTANCES, ACCOUNTS, JAVAS, VERSIONS, TASKS, hue, javaNeedOf, setLoaderSource } from './data.js';

const T = window.__TAURI__;
export const REAL = !!T;
export const invoke = (cmd, args) => T.core.invoke(cmd, args);
export const listen = (ev, fn) => T.event.listen(ev, fn);

// 还没做的功能统一这么说，不假装做成了
export const NOT_YET = '这个功能还在做，按规划一项项接上';

// 后端给的原始数据，设置页、下载页直接用
export const R = { info: null, targets: [], manifest: null, manifestError: '', sources: null, taskOf: new Map(), lv: {}, accounts: [], defaultAccount: 'none', bisect: new Map(), zh: new Map(), res: { kind: 'mod', source: 'modrinth', q: '', sort: 'relevance', fit: true, inst: null, hits: [], total: 0, loading: false, error: '' }, updates: new Map() };
// app.js 注册：后台数据到了要重画哪些地方
export const hooks = { changed: () => {} };

const LOADER_KIND = { Fabric: 'fabric', Quilt: 'quilt', Forge: 'forge', NeoForge: 'neoforge' };
const BLOCK = { Fabric: 'copper', Quilt: 'diamond', Forge: 'brass', NeoForge: 'nether', OptiFine: 'stone' };
export const blockOf = (loader) => BLOCK[loader] || 'grass';
const replace = (arr, items) => arr.splice(0, arr.length, ...items);
const javaNeed = (mc) => (mc && /^1\.\d+/.test(mc) ? javaNeedOf(mc) : 21);

// ---------- 启动时 ----------
export async function boot() {
    replace(VERSIONS, []);
    replace(TASKS, []);
    const [info, targets, javas, tasks, accounts, prefs] = await Promise.all([
        invoke('app_info'),
        invoke('scan_targets').catch(() => []),
        invoke('list_java').catch(() => []),
        invoke('list_tasks').catch(() => []),
        invoke('list_accounts').catch(() => []),
        invoke('get_launch_prefs').catch(() => null),
    ]);
    R.prefs = prefs;
    R.lan = await invoke('lan_status').catch(() => null);
    R.cf = await invoke('cf_available').catch(() => false);
    R.ai = await invoke('ai_get').catch(() => null);
    R.edition = await invoke('edition_get').catch(() => null);
    // 显卡列表：概览上的「显卡」卡片用，不挡启动
    invoke('gpu_list').then((g) => { R.gpus = g; }).catch(() => {});
    applyAccounts(accounts);
    R.info = info;
    applyTargets(targets);
    replace(JAVAS, javas.map((j, k) => ({ id: 'j' + k, label: `Java ${j.major || '?'} · ${j.source}`, major: j.major, path: j.path })));
    tasks.forEach(upsertTask);
    setLoaderSource(loaderPickVersion);
    // 版本清单要联网，不挡第一屏
    loadManifest();
    loadSources();
}

export async function loadManifest(force = false) {
    try {
        R.manifest = await invoke('get_manifest', { force });
        R.manifestError = '';
        replace(VERSIONS, R.manifest.versions.filter((v) => v.type === 'release').map((v) => [v.id, v.releaseTime.slice(0, 10)]));
    } catch (e) {
        R.manifestError = String(e);
    }
    hooks.changed('versions');
}

export async function reloadPrefs() {
    R.prefs = await invoke('get_launch_prefs').catch(() => R.prefs);
    hooks.changed('prefs');
}

export async function loadSources() {
    R.sources = await invoke('get_sources').catch(() => R.sources);
    hooks.changed('sources');
}

// ---------- 游戏目录和实例 ----------
function instFrom(t, v) {
    return {
        id: `${t.id}/${v.name}`, dir: t.id, name: v.name, real: true, loaded: false,
        block: BLOCK[v.loader] || 'grass',
        mc: v.mc || '版本未知', loader: v.loader || null, loaderVer: v.loaderVersion || '', javaNeed: javaNeed(v.mc),
        isolatedDir: v.isolated,
        lastPlayed: 0, played: 0, lastRun: null, perf: false, cfg: {},
        sync: { on: false, state: 'ok', at: 0, device: '' },
        mods: [], worlds: [], servers: [], shots: [], shotCount: 0,
    };
}

// 重新扫描后换掉目录和实例；已经读过的 Mod、存档留着，下次看的时候再刷新
export function applyTargets(targets) {
    R.targets = targets;
    const old = new Map(INSTANCES.filter((i) => i.real).map((i) => [i.id, i]));
    replace(DIRS, targets.map((t) => ({ id: t.id, name: t.launcher, path: t.path, exe: t.exe || null, kind: t.kind, access: t.access, manual: t.manual, count: (t.versions || []).length })));
    const list = [];
    for (const t of targets) {
        for (const v of t.versions || []) {
            const fresh = instFrom(t, v);
            const prev = old.get(fresh.id);
            if (prev) {
                Object.assign(prev, { mc: fresh.mc, loader: fresh.loader, loaderVer: fresh.loaderVer, block: fresh.block, javaNeed: fresh.javaNeed, isolatedDir: fresh.isolatedDir, loaded: false, loadError: '' });
                list.push(prev);
            } else list.push(fresh);
        }
    }
    // 正在装、还没出现在目录里的，先留着
    for (const i of old.values()) if (i.installing && !list.some((x) => x.id === i.id)) list.push(i);
    replace(INSTANCES, list);
}

export async function rescan() {
    applyTargets(await invoke('scan_targets'));
    hooks.changed('targets');
}

// 实例的 Mod、存档、截图：选中时才读
export async function loadDetails(i) {
    const f = await invoke('instance_files', { dir: i.dir, name: i.name });
    i.gameDir = f.gameDir;
    i.isolatedDir = f.isolated;
    i.mods = f.mods.map(modFrom);
    i.worlds = f.worlds.map((w, k) => ({ id: 'w' + k, name: w.name, folder: w.name, mode: '', diff: '', last: w.modified, size: '', history: [], info: null }));
    i.shotCount = f.screenshots;
    i.cfgReal = await invoke('instance_config', { dir: i.dir, name: i.name }).catch(() => ({}));
    i.loaded = true;
    i.loadError = '';
    loadModMeta(i);
    loadWorldInfo(i);
    i.link = await invoke('lan_link', { dir: i.dir, name: i.name }).catch(() => null);
}

// Mod 文件里的元数据：真名字、版本、作者、前置、链接；顺便查缺了哪些前置
async function loadModMeta(i) {
    try {
        const r = await invoke('instance_mods', { dir: i.dir, name: i.name });
        const by = new Map(r.mods.map((m) => [m.file, m]));
        for (const m of i.mods) {
            const meta = by.get(m.file);
            if (!meta) continue;
            m.meta = meta;
            if (meta.name) m.name = meta.name;
            if (meta.version && !meta.version.includes('${')) m.ver = meta.version;
            m.missing = r.missing.filter(([f]) => f === m.file).map(([, d]) => d);
        }
        i.missingDeps = r.missing;
        hooks.changed('mods', i);
    } catch {}
    // 图标：第一次要翻 jar，之后走缓存
    try {
        const icons = await invoke('mod_icons', { dir: i.dir, name: i.name });
        for (const m of i.mods) m.icon = icons[m.file] || icons[m.file.replace(/\.disabled$/, '')] || null;
        hooks.changed('mods', i);
    } catch {}
}

// 存档信息（名字、种子、模式……）一个个读，读到一个就重画一次存档页
export async function loadWorldInfo(i) {
    for (const w of i.worlds) {
        try {
            w.info = await invoke('world_info', { dir: i.dir, name: i.name, world: w.folder });
            w.name = w.info.name;
        } catch (e) {
            w.info = { error: String(e) };
        }
        hooks.changed('worlds', i);
    }
}

// 文件名一般是「名字-版本」：sodium-fabric-0.5.11+mc1.20.1.jar → 名字 sodium-fabric、版本 0.5.11+mc1.20.1
function modFrom(m, k) {
    const base = m.file.replace(/\.disabled$/i, '').replace(/\.(jar|zip)$/i, '');
    const mm = base.match(/^(.+?)[-_ ]v?(\d[\w.+-]*)$/);
    const name = mm ? mm[1] : base;
    return { id: 'm' + k, file: m.file, name, ver: mm ? mm[2] : '', src: sizeText(m.size), on: m.enabled, h: hue(name), client: false, zh: '' };
}

function sizeText(b) {
    if (!b) return '0 KB';
    return b >= 1048576 ? (b / 1048576).toFixed(1) + ' MB' : Math.max(1, Math.round(b / 1024)) + ' KB';
}

export async function reloadJava() {
    const javas = await invoke('list_java').catch(() => []);
    replace(JAVAS, javas.map((j, k) => ({ id: 'j' + k, label: `Java ${j.major || '?'} · ${j.source}`, major: j.major, path: j.path })));
}

export async function setMod(i, m, on) {
    m.file = await invoke('set_mod_enabled', { dir: i.dir, name: i.name, file: m.file, enabled: on });
    m.on = on;
}

// ---------- 加载器版本：新建实例时按选的 MC 版本去查 ----------
// 返回推荐版本号；null = 这个 MC 版本没有这个加载器；undefined = 还在查
function loaderPickVersion(loader, mc) {
    if (!loader) return '';
    const e = R.lv[`${loader}:${mc}`];
    if (!e) {
        fetchLoader(loader, mc);
        return undefined;
    }
    if (e.state === 'loading') return undefined;
    return e.pick ? e.pick.version : null;
}

export const loaderInfo = (loader, mc) => R.lv[`${loader}:${mc}`];

async function fetchLoader(loader, mc) {
    const key = `${loader}:${mc}`;
    R.lv[key] = { state: 'loading' };
    try {
        const list = await invoke('list_loaders', { kind: LOADER_KIND[loader], mc });
        const pick = list.find((v) => v.tag === '推荐') || list.find((v) => v.stable) || list[0] || null;
        R.lv[key] = { state: pick ? 'ok' : 'none', pick, list };
    } catch (e) {
        const msg = String(e);
        R.lv[key] = { state: msg.includes('没有') ? 'none' : 'error', msg };
    }
    hooks.changed('loader');
}

// ---------- 安装 ----------
export async function install(dir, name, mc, loader) {
    const pick = loader ? R.lv[`${loader}:${mc}`]?.pick : null;
    if (loader && !pick) throw new Error(`还没查到 ${loader} 的版本`);
    const id = await invoke('start_install', {
        req: { targetId: dir.id, mcVersion: mc, name, loader: loader ? { kind: LOADER_KIND[loader], id: pick.id, version: pick.version } : null, overwrite: false },
    });
    const i = instFrom({ id: dir.id }, { name, mc, loader: loader || '', loaderVersion: pick?.version || '', isolated: false });
    i.installing = { pct: 0, stage: '准备中' };
    INSTANCES.push(i);
    R.taskOf.set(id, i.id);
    upsertTask({ id, title: loader ? `${mc} · ${LOADER_KIND[loader]} ${pick.version}` : mc, subtitle: name, target: dir.name, state: 'running', stage: '准备中', filesDone: 0, filesTotal: 0, bytesDone: 0, bytesTotal: 0, started: Date.now() });
    return i;
}

// ---------- 任务：后端的任务快照换成界面用的样子 ----------
const STATE = { running: 'run', done: 'done', failed: 'fail', cancelled: 'fail' };

function taskFrom(t) {
    const pct = t.state === 'done' ? 100
        : t.bytesTotal > 0 ? Math.min(99, Math.round((t.bytesDone / t.bytesTotal) * 100))
        : t.filesTotal > 0 ? Math.min(99, Math.round((t.filesDone / t.filesTotal) * 100)) : 0;
    const stage = t.state === 'running' ? t.stage : t.state === 'failed' ? `没装成：${t.error || '原因不明'}` : t.state === 'cancelled' ? '取消了' : '';
    return { id: 'b' + t.id, bid: t.id, title: `安装 ${t.subtitle || t.title}`, sub: `${t.title} · ${t.target}`, state: STATE[t.state] || 'run', pct, at: t.finished || t.started || Date.now(), stage, error: t.error || '', note: t.note || '' };
}

export function upsertTask(snap) {
    const x = taskFrom(snap);
    const k = TASKS.findIndex((t) => t.bid === snap.id);
    if (k >= 0) Object.assign(TASKS[k], x);
    else TASKS.unshift(x);
    return x;
}

export async function clearTasks() {
    const left = await invoke('clear_tasks');
    replace(TASKS, []);
    left.forEach(upsertTask);
}

// ---------- 账户 ----------
// 返回默认账户的 id；一个账户都没有时放一个占位，左下角点它就能添加
export function applyAccounts(list) {
    R.accounts = list;
    replace(ACCOUNTS, list.length
        ? list.map((a) => ({ id: a.id, name: a.name, label: a.kind === 'msa' ? '微软正版' : '离线', face: 'steve', kind: a.kind, def: a.default }))
        : [{ id: 'none', name: '还没有账户', label: '点这里添加', face: 'steve', none: true }]);
    R.defaultAccount = list.find((a) => a.default)?.id || list[0]?.id || 'none';
    return R.defaultAccount;
}

export async function reloadAccounts() {
    return applyAccounts(await invoke('list_accounts').catch(() => R.accounts));
}

// ---------- 启动 ----------
export const launch = (i, account, join) => invoke('launch_instance', { dir: i.dir, name: i.name, key: i.id, account: account && account !== 'none' ? account : null, opts: null, join: join || null });
export const stop = (run) => invoke('stop_instance', { run });

// 崩溃报告里最有用的两行：Description 和第一行异常
export async function crashInfo(path) {
    const text = await invoke('read_crash_report', { path }).catch(() => '');
    const lines = text.split(/\r?\n/);
    const k = lines.findIndex((l) => l.startsWith('Description:'));
    const desc = k >= 0 ? lines[k].slice(12).trim() : '';
    const exc = lines.slice(k + 1).find((l) => /(Exception|Error)\b/.test(l) && !l.startsWith('\t'))?.trim() || '';
    return { desc, exc };
}
