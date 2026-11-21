import { icon, hydrateIcons } from './icons.js';

// ---------- 与后端通信（浏览器里直接打开时用演示数据）----------
const TAURI = window.__TAURI__;
let invoke, listen;
if (TAURI) {
    invoke = TAURI.core.invoke;
    listen = TAURI.event.listen;
} else {
    const mock = await import('./mock.js');
    invoke = mock.invoke;
    listen = mock.listen;
}

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const store = {
    get(k, d) { try { const v = localStorage.getItem('mcs.' + k); return v == null ? d : JSON.parse(v); } catch { return d; } },
    set(k, v) { try { localStorage.setItem('mcs.' + k, JSON.stringify(v)); } catch { /* 无痕模式等 */ } },
};

function size(b) {
    if (!b) return '0 B';
    const u = ['B', 'KB', 'MB', 'GB'];
    let i = 0;
    while (b >= 1024 && i < u.length - 1) { b /= 1024; i++; }
    return (i ? b.toFixed(b >= 100 ? 0 : 1) : b) + ' ' + u[i];
}
const speed = (bps) => (bps > 0 ? size(bps) + '/s' : '—');
const date = (s) => (s ? s.slice(0, 10) : '');

let toastTimer;
function toast(msg) {
    const t = $('#toast');
    t.textContent = msg;
    t.classList.add('show');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => t.classList.remove('show'), 2600);
}

function modal(html, { wide = false } = {}) {
    const m = document.createElement('div');
    m.className = 'modal';
    m.innerHTML = `<div class="box ${wide ? 'wide' : ''}">${html}</div>`;
    document.body.appendChild(m);
    hydrateIcons(m);
    requestAnimationFrame(() => m.classList.add('in'));
    const close = () => {
        m.classList.add('out');
        setTimeout(() => m.remove(), 220);
    };
    m.addEventListener('click', (e) => { if (e.target === m) close(); });
    m.querySelectorAll('[data-close]').forEach((b) => b.addEventListener('click', close));
    return { el: m, close };
}

// ---------- 状态 ----------
const S = {
    info: { platform: 'windows' },
    manifest: null,
    filter: store.get('filter', 'release'),
    query: '',
    shown: 60,
    targets: [],
    tasks: new Map(),
    sources: null,
    view: 'versions',
};
const isAndroid = () => S.info.platform === 'android';

const LOADERS = [
    { k: 'vanilla', label: '原版', icon: 'box' },
    { k: 'fabric', label: 'Fabric', icon: 'feather' },
    { k: 'quilt', label: 'Quilt', icon: 'grid' },
    { k: 'forge', label: 'Forge', icon: 'hammer' },
    { k: 'neoforge', label: 'NeoForge', icon: 'hammer' },
];
const TYPE_LABEL = { release: '正式版', snapshot: '快照', old_beta: '远古 Beta', old_alpha: '远古 Alpha' };
const SRC_COLORS = ['var(--accent)', 'var(--accent-2)', 'var(--ok)', 'var(--amber)', 'var(--bad)'];

// ---------- 视图切换 ----------
const HERO = {
    versions: ['方块<span class="grad">补给</span>', 'MC SUPPLY', '所有 Minecraft 版本与 Fabric / Quilt / Forge / NeoForge，下载完直接放进你正在用的启动器。'],
    targets: ['找到的<span class="grad">启动器</span>', 'LAUNCHERS', ''],
    tasks: ['下载<span class="grad">任务</span>', 'TASKS', '下载过程中可以切到别的页面，进度会一直在这儿。'],
    sources: ['智能<span class="grad">选源</span>', 'MIRRORS', '官方源和国内镜像都会测速，下载时按每个文件的实际表现随时调整。'],
};

function switchView(name) {
    S.view = name;
    $$('.view').forEach((v) => v.classList.toggle('hidden', v.id !== name));
    const view = $('#' + name);
    view.classList.remove('enter');
    void view.offsetWidth;
    view.classList.add('enter');
    $$('[data-tab]').forEach((b) => b.classList.toggle('on', b.dataset.tab === name));
    const [t, chip, sub] = HERO[name];
    $('#heroTitle').innerHTML = t;
    $('#heroChip').textContent = chip;
    $('#heroSub').textContent = name === 'targets' ? targetHeroText() : sub;
    movePill();
    if (name === 'sources') renderSources();
    if (name === 'targets') renderTargets();
    window.scrollTo({ top: 0, behavior: 'smooth' });
}

function movePill() {
    const on = $('#nav button.on');
    const pill = $('#navPill');
    if (!on) return;
    pill.style.opacity = 1;
    pill.style.transform = `translateY(${on.offsetTop}px)`;
    pill.style.height = on.offsetHeight + 'px';
}

// ---------- 版本 ----------
function versionGroups() {
    const v = S.manifest?.versions || [];
    return {
        release: v.filter((x) => x.type === 'release'),
        snapshot: v.filter((x) => x.type === 'snapshot'),
        old: v.filter((x) => x.type.startsWith('old')),
        all: v,
    };
}

function renderVersionStats() {
    const m = S.manifest;
    const best = S.sources?.sources?.[0];
    const usable = S.targets.filter((t) => t.access === 'direct' || t.access === 'saf');
    const cards = [
        ['star', 'c-violet', m?.latest.release ?? '…', '最新正式版', '正式'],
        ['flask', 'c-amber', m?.latest.snapshot ?? '…', '最新快照', '快照'],
        ['rocket', 'c-emerald', usable.length ? `${usable.length} 个` : '0 个', '可写入的启动器目录', isAndroid() ? '安卓' : 'Windows'],
        ['zap', 'c-sky', best ? best.name : '测速中', best?.liveSpeed || best?.testSpeed ? speed(best.liveSpeed || best.testSpeed) : '当前首选下载源', '智能'],
    ];
    $('#verStats').innerHTML = cards
        .map(([ic, c, v, l, tag]) => `<div class="glass stat lift"><div class="stat-top"><div class="stat-ico ${c}">${icon(ic)}</div><span class="stat-tag ${c}">${tag}</span></div><div class="stat-v">${esc(v)}</div><div class="stat-l">${esc(l)}</div></div>`)
        .join('');
    if (m) $('#ctaText').textContent = `下载 ${m.latest.release}`;
}

function renderVersions(animate = true) {
    const groups = versionGroups();
    $$('#verFilter button').forEach((b) => {
        b.classList.toggle('on', b.dataset.f === S.filter);
        b.querySelector('.n').textContent = S.manifest ? groups[b.dataset.f].length : '';
    });
    const list = $('#verList');
    if (!S.manifest) {
        list.innerHTML = Array.from({ length: 6 }, () => '<div class="skeleton" style="height:66px;border-radius:18px"></div>').join('');
        return;
    }
    const q = S.query.trim().toLowerCase();
    let items = q ? groups.all.filter((v) => v.id.toLowerCase().includes(q)) : groups[S.filter];
    const total = items.length;
    items = items.slice(0, S.shown);
    if (!items.length) {
        list.innerHTML = `<div class="empty">没有找到「${esc(S.query)}」</div>`;
        $('#verMore').classList.add('hidden');
        return;
    }
    const latest = S.manifest.latest;
    list.innerHTML = items
        .map((v, i) => {
            const k = v.type === 'release' ? '' : v.type === 'snapshot' ? 'k-snapshot' : 'k-old';
            const ic = v.type === 'release' ? 'box' : v.type === 'snapshot' ? 'flask' : 'clock';
            const tag = v.id === latest.release || v.id === latest.snapshot ? '<span class="tag2 t-violet">最新</span>' : '';
            return `<div class="item click ${animate && i < 24 ? 'enter' : ''}" style="--d:${Math.min(i, 12) * 22}ms" data-ver="${esc(v.id)}">
                <div class="top">
                    <div class="fi ${k}">${icon(ic)}</div>
                    <div class="meta"><div class="nm">${esc(v.id)}${tag}</div><div class="sm">${TYPE_LABEL[v.type] || v.type} · ${date(v.releaseTime)}</div></div>
                    <div class="acts"><button class="btn small primary" data-ver="${esc(v.id)}">${icon('download')}安装</button></div>
                </div></div>`;
        })
        .join('');
    $('#verMore').classList.toggle('hidden', total <= S.shown);
}

async function loadManifest(force = false) {
    const btn = $('#verRefresh');
    btn.classList.add('spin');
    try {
        S.manifest = await invoke('get_manifest', { force });
        renderVersions();
        renderVersionStats();
    } catch (e) {
        $('#verList').innerHTML = `<div class="empty">版本清单拿不到：${esc(e)}<br><button class="btn small" style="margin-top:10px" id="retryManifest">重试</button></div>`;
        $('#retryManifest')?.addEventListener('click', () => loadManifest(true));
    } finally {
        btn.classList.remove('spin');
    }
}

// ---------- 安装弹窗 ----------
function defaultName(mc, loader, v) {
    switch (loader) {
        case 'fabric': return `${mc}-Fabric ${v}`;
        case 'quilt': return `${mc}-Quilt ${v}`;
        case 'forge': return `${mc}-Forge_${v}`;
        case 'neoforge': return `${mc}-NeoForge_${v}`;
        default: return mc;
    }
}

function openInstall(mc) {
    const usable = S.targets.filter((t) => t.access !== 'missing');
    const st = {
        mc,
        loader: 'vanilla',
        list: [],
        loading: false,
        error: '',
        picked: null,
        target: (usable.find((t) => t.id === store.get('lastTarget')) || usable.find((t) => t.access === 'direct' || t.access === 'saf') || usable[0])?.id,
        name: mc,
        nameTouched: false,
    };
    const vinfo = S.manifest?.versions.find((v) => v.id === mc);
    const { el, close } = modal(
        `<div class="head"><div><span class="chip mono">${esc(TYPE_LABEL[vinfo?.type] || 'VERSION')}</span><h2 class="serif" style="margin-top:8px">安装 ${esc(mc)}</h2><p class="hint">${date(vinfo?.releaseTime)} 发布</p></div><button class="iconbtn" data-close>${icon('x')}</button></div>
        <div class="field"><label>加载器</label><div class="seg" id="ldSeg">${LOADERS.map((l) => `<button data-l="${l.k}" class="${l.k === 'vanilla' ? 'on' : ''}">${icon(l.icon, 'style="width:13px;height:13px"')}${l.label}</button>`).join('')}</div>
            <div id="ldBody"></div></div>
        <div class="field"><label>放到哪个启动器</label><div class="devices" id="tgList" style="margin-top:0;grid-template-columns:repeat(auto-fill,minmax(200px,1fr))"></div></div>
        <div class="field"><label>版本名</label><input type="text" id="vName" maxlength="80"><p class="hint" id="nameHint"></p></div>
        <div id="insNote"></div>
        <div class="row" style="margin-top:18px;justify-content:flex-end"><button class="btn" data-close>取消</button><button class="btn primary" id="goBtn">${icon('download')}开始安装</button></div>`,
        { wide: true },
    );

    const renderLoaderBody = () => {
        const body = $('#ldBody', el);
        if (st.loader === 'vanilla') {
            body.innerHTML = '<p class="hint" style="margin-top:8px">只装原版。之后想加模组，可以重新装一个带加载器的版本。</p>';
            return;
        }
        if (st.loading) {
            body.innerHTML = '<div class="loader-list">' + '<div class="skeleton"></div>'.repeat(4) + '</div>';
            return;
        }
        if (st.error) {
            body.innerHTML = `<div class="empty" style="margin-top:8px;padding:16px">${esc(st.error)}</div>`;
            return;
        }
        body.innerHTML = `<div class="loader-list">${st.list
            .slice(0, 80)
            .map((v) => `<button class="lv ${st.picked?.id === v.id ? 'sel' : ''}" data-id="${esc(v.id)}"><span class="v">${esc(v.version)}</span>${v.tag ? `<span class="tag2 ${v.tag === '推荐' ? 't-ok' : 't-violet'}">${esc(v.tag)}</span>` : ''}${v.stable ? '' : '<span class="tag2 t-warn">测试版</span>'}</button>`)
            .join('')}</div>`;
        $$('.lv', body).forEach((b) =>
            b.addEventListener('click', () => {
                st.picked = st.list.find((v) => v.id === b.dataset.id);
                $$('.lv', body).forEach((x) => x.classList.toggle('sel', x === b));
                syncName();
            }),
        );
    };

    const renderTargetsPick = () => {
        const box = $('#tgList', el);
        if (!usable.length) {
            box.innerHTML = `<div class="empty" style="grid-column:1/-1">还没找到启动器目录。去「启动器」页扫描或手动添加一个。</div>`;
            return;
        }
        box.innerHTML = usable.map((t, i) => targetCard(t, i, { pick: true, sel: t.id === st.target })).join('');
        $$('.dev', box).forEach((d) =>
            d.addEventListener('click', () => {
                st.target = d.dataset.id;
                $$('.dev', box).forEach((x) => x.classList.toggle('sel', x === d));
                syncName();
            }),
        );
    };

    const syncName = () => {
        if (!st.nameTouched) {
            st.name = defaultName(mc, st.loader, st.picked?.version ?? '');
            $('#vName', el).value = st.name;
        }
        const t = S.targets.find((x) => x.id === st.target);
        const clash = t?.versions?.some((v) => v.name === st.name.trim());
        $('#nameHint', el).innerHTML = clash ? `<span style="color:var(--bad)">「${esc(t.launcher)}」里已经有这个名字的版本了，继续会覆盖它的版本信息。</span>` : '启动器里显示的就是这个名字，可以改。';
        const note = $('#insNote', el);
        const notes = [];
        if (isAndroid() && (st.loader === 'forge' || st.loader === 'neoforge')) {
            notes.push('安卓上没法运行 Forge / NeoForge 的安装器：会先装好原版，并把安装器 jar 放进启动器目录的 installers 文件夹（有权限时也存一份到 Download/方块补给），之后在启动器里用「执行 jar」完成安装。');
        }
        if (t && t.access === 'need-saf') notes.push(`还没授权访问「${t.launcher}」的目录，点开始后会先让你授权。`);
        if (t && t.access === 'need-permission') notes.push('需要先开启「所有文件访问」权限。');
        note.innerHTML = notes.map((n) => `<div class="notebox">${esc(n)}</div>`).join('');
    };

    $$('#ldSeg button', el).forEach((b) =>
        b.addEventListener('click', async () => {
            st.loader = b.dataset.l;
            $$('#ldSeg button', el).forEach((x) => x.classList.toggle('on', x === b));
            st.picked = null;
            st.error = '';
            if (st.loader !== 'vanilla') {
                st.loading = true;
                renderLoaderBody();
                const want = st.loader;
                try {
                    const list = await invoke('list_loaders', { kind: want, mc });
                    if (st.loader !== want) return;
                    st.list = list;
                    st.picked = list.find((v) => v.tag === '推荐') || list.find((v) => v.stable) || list[0] || null;
                    if (!list.length) st.error = `Minecraft ${mc} 没有 ${b.textContent.trim()}`;
                } catch (e) {
                    if (st.loader !== want) return;
                    st.error = String(e).includes('没有') ? String(e) : `取不到 ${b.textContent.trim()} 的版本列表：${e}`;
                }
                st.loading = false;
            }
            renderLoaderBody();
            syncName();
        }),
    );
    $('#vName', el).addEventListener('input', (e) => {
        st.nameTouched = true;
        st.name = e.target.value;
        syncName();
    });

    $('#goBtn', el).addEventListener('click', async () => {
        const t = S.targets.find((x) => x.id === st.target);
        if (!t) return toast('先选一个启动器目录');
        if (st.loader !== 'vanilla' && !st.picked) return toast('先选一个加载器版本');
        if (!st.name.trim()) return toast('版本名不能为空');
        if (t.access === 'need-saf') {
            const ok = await grantTarget(t);
            if (!ok) return;
        }
        if (t.access === 'need-permission') {
            await invoke('android_request_all_files');
            return toast('开好权限后回来再点一次');
        }
        const loader = st.loader === 'vanilla' ? null : { kind: st.loader, id: st.picked.id, version: st.picked.version };
        const clash = t.versions?.some((v) => v.name === st.name.trim());
        try {
            const id = await invoke('start_install', { req: { targetId: st.target, mcVersion: mc, name: st.name.trim(), loader, overwrite: !!clash } });
            store.set('lastTarget', st.target);
            S.tasks.set(id, { id, title: loader ? `${mc} · ${loader.kind} ${loader.version}` : mc, subtitle: st.name, target: t.launcher, state: 'running', stage: '准备中', filesDone: 0, filesTotal: 0, bytesDone: 0, bytesTotal: 0, speed: 0, sources: {}, log: [] });
            close();
            renderTasks();
            switchView('tasks');
            toast('开始下载');
        } catch (e) {
            toast(String(e));
        }
    });

    renderLoaderBody();
    renderTargetsPick();
    syncName();
}

// ---------- 启动器 ----------
const KIND_SHORT = { pcl: 'PCL', 'pcl-ce': 'CE', hmcl: 'HM', official: 'MC', bakaxl: 'BX', sjmcl: 'SJ', zl2: 'ZL2', zl: 'ZL', fcl: 'FCL', pojav: 'PJ', hmclpe: 'HP', custom: '' };

function accessTag(t) {
    switch (t.access) {
        case 'direct': return '<span class="tag">可写入</span>';
        case 'saf': return '<span class="tag sky">已授权</span>';
        case 'need-saf': return '<span class="tag warn">待授权</span>';
        case 'need-permission': return '<span class="tag warn">缺权限</span>';
        default: return '<span class="tag warn">不可用</span>';
    }
}

function targetCard(t, i, { pick = false, sel = false, actions = false } = {}) {
    const short = KIND_SHORT[t.kind] || '';
    const loaders = {};
    (t.versions || []).forEach((v) => { if (v.loader) loaders[v.loader] = (loaders[v.loader] || 0) + 1; });
    const loaderText = Object.entries(loaders).map(([k, n]) => `${k} ${n}`).join(' · ');
    const acts = actions
        ? `<div class="acts">
            ${t.access === 'need-saf' ? `<button class="btn small primary" data-act="grant">${icon('lock')}授权访问</button>` : ''}
            ${t.access === 'need-permission' ? `<button class="btn small primary" data-act="perm">${icon('lock')}开启权限</button>` : ''}
            ${!isAndroid() && t.access === 'direct' ? `<button class="btn small" data-act="open">${icon('folder')}打开</button>` : ''}
            ${t.manual || t.access === 'saf' ? `<button class="btn small danger" data-act="remove">${icon('trash')}移除</button>` : ''}
        </div>`
        : '';
    return `<div class="dev ${pick ? 'pick' : ''} ${sel ? 'sel' : ''} ${t.access === 'need-saf' || t.access === 'need-permission' ? 'off' : ''}" data-id="${esc(t.id)}" style="--d:${i * 40}ms">
        ${accessTag(t)}
        <div class="ico k-${esc(t.kind)}">${short ? esc(short) : icon('folder')}</div>
        <div class="n">${esc(t.launcher)}</div>
        <div class="p">${esc(t.path)}</div>
        <div class="s">${icon('layers', 'style="width:13px;height:13px"')}${t.access === 'need-saf' ? '授权后可查看版本' : `${(t.versions || []).length} 个版本`}${loaderText ? ' · ' + esc(loaderText) : ''}</div>
        ${t.hint ? `<div class="s" style="margin-top:2px">${esc(t.hint)}</div>` : ''}
        ${acts}
    </div>`;
}

function targetHeroText() {
    if (!S.targets.length) return isAndroid() ? '支持 Zalith Launcher 2、FCL、PojavLauncher 等。装了启动器的话会自动出现在这里。' : '会自动识别 PCL、HMCL、官方启动器等，也可以手动指定 .minecraft 文件夹。';
    const names = [...new Set(S.targets.map((t) => t.launcher))];
    return `识别到 ${names.join('、')}。下载的版本会直接放进这些目录，打开启动器就能玩。`;
}

function renderTargets() {
    const list = $('#targetList');
    $('#targetHint').textContent = isAndroid()
        ? '启动器自己的私有目录（Android/data）需要你在系统文件选择器里授权一次：选左侧栏里的启动器名字，再点「使用此文件夹」。'
        : '找了常用位置和 PCL 的记录。没找到的话，点「手动添加目录」选 .minecraft 文件夹。';
    if (!S.targets.length) {
        list.innerHTML = `<div class="empty" style="grid-column:1/-1">${S.scanning ? '正在扫描…' : '没有找到启动器目录'}</div>`;
    } else {
        list.innerHTML = S.targets.map((t, i) => targetCard(t, i, { actions: true })).join('');
    }
    $$('.dev', list).forEach((d) => {
        const t = S.targets.find((x) => x.id === d.dataset.id);
        d.querySelectorAll('[data-act]').forEach((b) =>
            b.addEventListener('click', async (e) => {
                e.stopPropagation();
                const act = b.dataset.act;
                if (act === 'open') invoke('open_path', { path: t.path }).catch((err) => toast(String(err)));
                if (act === 'grant') grantTarget(t);
                if (act === 'perm') invoke('android_request_all_files').then(() => toast('开好之后回到这里会自动刷新'));
                if (act === 'remove') {
                    S.targets = await invoke('remove_target', { id: t.id });
                    renderTargets();
                    afterTargets();
                }
            }),
        );
    });
    renderPermBanner();
    if (S.view === 'targets') $('#heroSub').textContent = targetHeroText();
}

async function renderPermBanner() {
    const box = $('#permBanner');
    if (!isAndroid()) return (box.innerHTML = '');
    const st = await invoke('android_status').catch(() => ({ allFiles: true }));
    box.innerHTML = st.allFiles
        ? ''
        : `<div class="banner warn">${icon('lock')}<div><b>开启「所有文件访问」</b><br><span class="hint">FCL、Pojav 这类把游戏放在公共存储的启动器需要这个权限才能写入；Zalith Launcher 2 用授权目录就行，不需要。</span></div><button class="btn small primary" id="permBtn">去开启</button></div>`;
    $('#permBtn')?.addEventListener('click', () => invoke('android_request_all_files'));
}

async function grantTarget(t) {
    try {
        toast('在文件选择器里选启动器，再点「使用此文件夹」');
        S.targets = await invoke('android_grant', { package: t?.package ?? null });
        renderTargets();
        afterTargets();
        toast('授权成功');
        return true;
    } catch (e) {
        toast(String(e));
        return false;
    }
}

async function scan() {
    S.scanning = true;
    $('#scanBtn').classList.add('spin');
    if (S.view === 'targets') renderTargets();
    try {
        S.targets = await invoke('scan_targets');
    } catch (e) {
        toast('扫描失败：' + e);
    }
    S.scanning = false;
    $('#scanBtn').classList.remove('spin');
    renderTargets();
    afterTargets();
}

function afterTargets() {
    const usable = S.targets.filter((t) => t.access === 'direct' || t.access === 'saf');
    $('#targetChip').textContent = usable.length ? `${usable.length} 个启动器目录可用` : S.targets.length ? '启动器目录待授权' : '没找到启动器';
    renderVersionStats();
}

async function renderJava() {
    if (isAndroid()) return;
    $('#javaPanel').classList.remove('hidden');
    const list = await invoke('list_java').catch(() => []);
    $('#javaList').innerHTML = list.length
        ? list
              .map((j, i) => `<div class="item enter" style="--d:${i * 30}ms"><div class="top"><div class="fi k-amber" style="background:var(--amber-soft);color:var(--amber)">${j.major || '?'}</div><div class="meta"><div class="nm">Java ${esc(j.version || j.major)}<span class="tag2">${esc(j.source)}</span></div><div class="sm mono">${esc(j.path)}</div></div></div></div>`)
              .join('')
        : '<div class="empty">本机没找到 Java。装 Forge 时会自动下载官方运行时。</div>';
}

// ---------- 任务 ----------
function taskPercent(t) {
    if (t.state === 'done') return 100;
    if (t.bytesTotal > 0) return Math.min(99, (t.bytesDone / t.bytesTotal) * 100);
    if (t.filesTotal > 0) return Math.min(99, (t.filesDone / t.filesTotal) * 100);
    return 2;
}

function taskHtml(t) {
    const cls = t.state === 'done' ? 'done' : t.state === 'failed' ? 'err' : t.state === 'cancelled' ? 'cancel' : '';
    const ic = t.state === 'done' ? 'check' : t.state === 'failed' ? 'alert' : t.state === 'cancelled' ? 'stop' : 'download';
    const fk = t.state === 'done' ? 'k-ok' : t.state === 'failed' ? 'k-old' : '';
    const srcTotal = Object.values(t.sources || {}).reduce((a, b) => a + b, 0);
    const srcNames = Object.fromEntries((S.sources?.sources || []).map((s) => [s.id, s.name]));
    const mix = srcTotal
        ? `<span class="srcmix" title="${Object.entries(t.sources).map(([k, v]) => `${srcNames[k] || k} ${size(v)}`).join('，')}">${Object.entries(t.sources)
              .map(([, v], i) => `<i style="width:${(v / srcTotal) * 100}%;background:${SRC_COLORS[i % SRC_COLORS.length]}"></i>`)
              .join('')}</span><span>${Object.entries(t.sources).map(([k, v]) => `${esc(srcNames[k] || k)} ${size(v)}`).join(' · ')}</span>`
        : '';
    const running = t.state === 'running';
    return `<div class="top">
            <div class="fi ${fk}">${icon(ic)}</div>
            <div class="meta"><div class="nm">${esc(t.title)}<span class="tag2 ${t.state === 'done' ? 't-ok' : t.state === 'failed' ? 't-bad' : ''}">${esc(t.subtitle)}</span></div><div class="sm">${esc(t.stage)}</div></div>
            <div class="acts">
                ${running ? `<button class="btn small danger" data-cancel="${t.id}">${icon('x')}取消</button>` : ''}
                ${t.state === 'done' && t.resultPath && !isAndroid() ? `<button class="btn small" data-open="${esc(t.resultPath)}">${icon('folder')}打开</button>` : ''}
            </div>
        </div>
        <div class="bar"><i style="width:${taskPercent(t).toFixed(1)}%"></i></div>
        <div class="task-foot">
            ${running ? `<span><b>${speed(t.speed)}</b></span>` : ''}
            <span>${t.filesTotal ? `<b>${t.filesDone}</b> / ${t.filesTotal} 个文件` : ''}</span>
            ${t.bytesTotal ? `<span><b>${size(t.bytesDone)}</b> / ${size(t.bytesTotal)}</span>` : ''}
            <span style="flex:1"></span>
            <span>${esc(t.target)}</span>
        </div>
        ${mix ? `<div class="task-foot">${mix}</div>` : ''}
        ${t.error ? `<div class="errbox">${esc(t.error)}</div>` : ''}
        ${t.note ? `<div class="notebox">${esc(t.note)}</div>` : ''}
        ${t.log?.length ? `<details class="log" ${t.state === 'failed' ? 'open' : ''}><summary>过程记录</summary><div class="logbox">${t.log.map(esc).join('\n')}</div></details>` : ''}`;
}

function renderTasks() {
    const list = $('#taskList');
    const tasks = [...S.tasks.values()].sort((a, b) => b.id - a.id);
    const running = tasks.filter((t) => t.state === 'running').length;
    $$('[data-badge]').forEach((b) => {
        b.textContent = running;
        b.classList.toggle('hidden', !running);
    });
    if (!tasks.length) {
        list.innerHTML = '<div class="empty">还没有下载任务。去「游戏版本」挑一个吧。</div>';
        return;
    }
    if (list.querySelector('.empty')) list.innerHTML = '';
    // 增量更新，展开的记录不会被重置
    for (const t of tasks) {
        let el = list.querySelector(`[data-task="${t.id}"]`);
        const cls = `item ${t.state === 'done' ? 'done' : t.state === 'failed' ? 'err' : t.state === 'cancelled' ? 'cancel' : ''}`;
        if (!el) {
            el = document.createElement('div');
            el.dataset.task = t.id;
            el.className = cls + ' enter';
            list.prepend(el);
        } else {
            el.className = cls;
        }
        const openLog = el.querySelector('details')?.open;
        el.innerHTML = taskHtml(t);
        if (openLog) el.querySelector('details')?.setAttribute('open', '');
    }
    list.querySelectorAll('[data-task]').forEach((el) => { if (!S.tasks.has(Number(el.dataset.task))) el.remove(); });
}

// ---------- 下载源 ----------
function renderSourceChip() {
    const s = S.sources;
    const best = s?.sources?.[0];
    const name = best ? best.name : '测速中';
    const tested = s?.sources?.some((x) => x.testedAt);
    $('#srcName').textContent = best ? `首选 ${name}` : '正在测速…';
    $('#srcWho').textContent = s?.settings?.mode === 'fixed' ? '固定选源' : tested ? `智能选源 · ${speed(best?.liveSpeed || best?.testSpeed)}` : '智能选源 · 测速中';
    $('#srcNameM').textContent = name;
    for (const d of [$('#srcDot'), $('#srcDotM')]) {
        d.className = 'dot' + (tested ? '' : ' pulse warn');
    }
}

function renderSources() {
    const s = S.sources;
    if (!s) return;
    const max = Math.max(1, ...s.sources.map((x) => x.liveSpeed || x.testSpeed || 0));
    $('#srcList').innerHTML = s.sources
        .map((x, i) => {
            const sp = x.liveSpeed || x.testSpeed;
            return `<div class="src ${i === 0 ? 'best' : ''}" style="--d:${i * 50}ms">
                <span class="rank">#${x.rank}${i === 0 ? ' 首选' : ''}</span>
                <h3>${esc(x.name)}${x.cooling ? '<span class="tag2 t-warn">冷却中</span>' : ''}</h3>
                <div class="base">${esc(x.kind === 'official' ? 'Mojang / Fabric / Forge / NeoForge 官方' : x.base)}</div>
                <div class="nums">
                    <div><b>${x.latencyMs != null ? x.latencyMs + ' ms' : '—'}</b><span>延迟</span></div>
                    <div><b>${sp ? size(sp) + '/s' : '—'}</b><span>${x.liveSpeed ? '实际速度' : '测速'}</span></div>
                    <div><b>${x.ok}/${x.ok + x.fail}</b><span>成功</span></div>
                </div>
                <div class="meter"><i style="width:${sp ? Math.max(4, (sp / max) * 100) : 0}%"></i></div>
                ${x.lastError ? `<div class="err">最近错误：${esc(x.lastError)}</div>` : ''}
                ${!x.builtin ? `<div class="row" style="margin-top:10px"><button class="btn small danger" data-del="${esc(x.id)}">${icon('trash')}删除</button></div>` : ''}
            </div>`;
        })
        .join('');
    $$('[data-del]').forEach((b) =>
        b.addEventListener('click', () => {
            const settings = { ...s.settings, custom: s.settings.custom.filter((c) => c.id !== b.dataset.del) };
            saveSettings(settings);
        }),
    );
    $$('#modeSeg button').forEach((b) => b.classList.toggle('on', b.dataset.m === s.settings.mode));
    $('#prefRow').classList.toggle('hidden', s.settings.mode !== 'fixed');
    $('#prefSel').innerHTML = s.sources.map((x) => `<option value="${esc(x.id)}" ${x.id === s.settings.preferred ? 'selected' : ''}>${esc(x.name)}</option>`).join('');
    $('#threads').value = s.settings.threads;
    $('#threadsVal').textContent = s.settings.threads;
    renderSourceChip();
}

async function loadSources() {
    S.sources = await invoke('get_sources');
    renderSourceChip();
    if (S.view === 'sources') renderSources();
    renderVersionStats();
}

async function saveSettings(settings) {
    S.sources = await invoke('save_net_settings', { settings });
    renderSources();
}

// ---------- 启动 ----------
async function init() {
    hydrateIcons();
    S.info = await invoke('app_info').catch(() => S.info);
    document.body.classList.add('p-' + S.info.platform);
    $('#addDirText').textContent = isAndroid() ? '授权其他目录' : '手动添加目录';

    $$('[data-tab]').forEach((b) => b.addEventListener('click', () => switchView(b.dataset.tab)));
    window.addEventListener('resize', movePill);
    $('#verFilter').addEventListener('click', (e) => {
        const b = e.target.closest('button');
        if (!b) return;
        S.filter = b.dataset.f;
        S.shown = 60;
        store.set('filter', S.filter);
        renderVersions();
    });
    $('#verSearch').addEventListener('input', (e) => {
        S.query = e.target.value;
        S.shown = 60;
        renderVersions(false);
    });
    $('#verList').addEventListener('click', (e) => {
        const it = e.target.closest('[data-ver]');
        if (it) openInstall(it.dataset.ver);
    });
    $('#verMore button').addEventListener('click', () => {
        S.shown += 120;
        renderVersions(false);
    });
    $('#verRefresh').addEventListener('click', () => loadManifest(true));
    $('#ctaLatest').addEventListener('click', () => S.manifest && openInstall(S.manifest.latest.release));

    $('#scanBtn').addEventListener('click', scan);
    $('#addDir').addEventListener('click', async () => {
        if (isAndroid()) return grantTarget(null);
        const path = await invoke('pick_folder');
        if (!path) return;
        S.targets = await invoke('add_manual_dir', { path });
        renderTargets();
        afterTargets();
        toast('已添加');
    });
    $('#javaRefresh').addEventListener('click', renderJava);

    $('#taskList').addEventListener('click', (e) => {
        const c = e.target.closest('[data-cancel]');
        if (c) invoke('cancel_task', { id: Number(c.dataset.cancel) });
        const o = e.target.closest('[data-open]');
        if (o) invoke('open_path', { path: o.dataset.open }).catch((err) => toast(String(err)));
    });
    $('#clearTasks').addEventListener('click', async () => {
        const list = await invoke('clear_tasks');
        S.tasks = new Map(list.map((t) => [t.id, t]));
        $('#taskList').innerHTML = '';
        renderTasks();
    });

    $('#testBtn').addEventListener('click', async () => {
        const b = $('#testBtn');
        b.disabled = true;
        b.lastChild.textContent = '测速中…';
        try {
            S.sources = await invoke('test_sources');
            renderSources();
            renderVersionStats();
        } finally {
            b.disabled = false;
            b.lastChild.textContent = '重新测速';
        }
    });
    $('#modeSeg').addEventListener('click', (e) => {
        const b = e.target.closest('button');
        if (b) saveSettings({ ...S.sources.settings, mode: b.dataset.m });
    });
    $('#prefSel').addEventListener('change', (e) => saveSettings({ ...S.sources.settings, preferred: e.target.value }));
    $('#threads').addEventListener('input', (e) => ($('#threadsVal').textContent = e.target.value));
    $('#threads').addEventListener('change', (e) => saveSettings({ ...S.sources.settings, threads: Number(e.target.value) }));
    $('#mirAdd').addEventListener('click', () => {
        const name = $('#mirName').value.trim() || '自定义镜像';
        const base = $('#mirBase').value.trim().replace(/\/+$/, '');
        if (!/^https?:\/\//.test(base)) return toast('镜像地址要以 http(s):// 开头');
        const custom = [...(S.sources.settings.custom || []), { id: 'custom-' + Date.now(), name, kind: 'bmclapi', base, builtin: false, enabled: true }];
        saveSettings({ ...S.sources.settings, custom });
        $('#mirName').value = '';
        $('#mirBase').value = '';
        toast('已添加，记得测速');
    });

    await listen('task-update', (e) => {
        S.tasks.set(e.payload.id, e.payload);
        renderTasks();
        if (e.payload.state === 'done') toast(`${e.payload.subtitle} 装好了`);
        if (e.payload.state === 'failed') toast(`${e.payload.subtitle} 安装失败`);
    });
    await listen('sources-updated', loadSources);
    await listen('targets-dirty', scan);
    document.addEventListener('visibilitychange', () => { if (!document.hidden && isAndroid()) scan(); });

    switchView('versions');
    renderVersions();
    renderVersionStats();
    renderTasks();
    loadManifest();
    loadSources();
    scan();
    renderJava();
    (await invoke('list_tasks').catch(() => [])).forEach((t) => S.tasks.set(t.id, t));
    renderTasks();
}

init();
