// 插件（Everything is Plugins）：官方插件、插件市场、权限、外观插件换主题和字体、安全模式、吊销名单、定制版预览
import { $, toast, store } from './ui.js';
import { hue, newInstance, CATALOG } from './data.js';

export const LAYER = { look: '外观', decl: '声明式扩展', script: '脚本插件', native: '原生插件' };
export const TIER = {
    official: ['官方', 'check'], verified: ['认证开发者', 'shield'], reviewed: ['已审核', 'check'],
    community: ['社区', 'users'], dev: ['本地开发', 'terminal'], revoked: ['已吊销', 'x'],
};
// 权限：给用户看的大白话 + 危险程度
export const PERM = {
    inst: ['读你的实例列表和设置', 'low'],
    mods: ['读实例里装了哪些 Mod', 'low'],
    crash: ['读崩溃日志（给它之前打过码）', 'low'],
    ui: ['在实例概览里加一张卡片', 'low'],
    ai: ['给 AI 助手和 MCP 加工具（只读级）', 'low'],
    acct: ['读你的游戏名和 UUID（拿不到登录令牌）', 'low'],
    shots: ['读你的截图', 'low'],
    net: ['联网，只能访问 {host}', 'mid'],
    bg: ['在后台运行', 'mid'],
    agent: ['往游戏里加 javaagent（能在游戏进程里运行代码）', 'high'],
};

// 官方插件：三个功能包 + 三个内置功能（能关）
export const OFFICIAL = [
    { id: 'mp', name: '联机', ic: 'users', kind: 'pack', desc: 'P2P 组网（EasyTier）、邀请码、常驻服务器', size: '约 14 MB' },
    { id: 'world', name: '世界工具', ic: 'globe', kind: 'pack', desc: '地图、查找建筑（cubiomes）、方块颜色表', size: '约 6 MB' },
    { id: 'tr', name: '离线翻译', ic: 'cpu', kind: 'pack', ondemand: true, desc: '开源翻译模型，在本机跑，用到再下（从魔搭社区下载）', size: '约 180 MB' },
    { id: 'ai', name: 'AI 助手与 MCP', ic: 'sparkles', kind: 'builtin', desc: '查崩溃、配整合包、让别的 AI 客户端操作启动器' },
    { id: 'zh', name: '补全中文', ic: 'file', kind: 'builtin', desc: '社区汉化 + 机器 / AI 补齐，生成资源包' },
    { id: 'skin', name: '皮肤工作台', ic: 'user', kind: 'builtin', desc: '画皮肤、AI 生成、一键换上' },
];

const mk = (p) => ({ ...p, h: hue(p.name) });
export const MARKET = [
    { id: 'forest', name: '墨绿森林', by: '小叶', layer: 'look', tier: 'reviewed', dl: 12400, size: '18 KB', ver: '1.4.0', perms: [], desc: '整套换成墨绿和米白，长时间看不累眼。装上立刻生效。', short: '主题：墨绿和米白' },
    { id: 'serif', name: '书卷气', by: '墨白', layer: 'look', tier: 'reviewed', dl: 5300, size: '6 KB', ver: '0.9.2', perms: [], desc: '界面文字全换成衬线体。用系统自带的宋体，不另外下载字体。', short: '字体：衬线体' },
    { id: 'crashrules', name: '常见崩溃规则 · 社区版', by: 'CrashHunter', layer: 'decl', tier: 'reviewed', dl: 31800, size: '42 KB', ver: '2026.11', perms: ['crash'], desc: '社区一起维护的 300 多条崩溃规则，包括一批 Mod 之间的已知冲突。装上以后，有些「规则认不出」的崩溃也能直接给修复按钮。', adds: '给崩溃识别加了 312 条规则' },
    { id: 'mirror', name: '国内镜像加速', by: '镜像站联盟（示例）', layer: 'decl', tier: 'reviewed', dl: 22000, size: '3 KB', ver: '1.2.0', perms: ['net:mirror.example.cn'], desc: '给游戏文件和 Modrinth 加一组国内镜像，测速后自动挑最快的。', adds: '下载源多了 3 个镜像' },
    { id: 'srvcard', name: '服务器状态卡', by: 'ServerKit', layer: 'script', tier: 'community', dl: 2100, size: '64 KB', ver: '1.0.0', perms: ['inst', 'net:你实例里填的服务器地址', 'ui'], desc: '在实例概览里加一张卡片：在线人数、延迟、今天谁上过线。', adds: '实例概览多一张卡片' },
    { id: 'packdoc', name: '整合包体检', by: 'Modpacker', layer: 'script', tier: 'reviewed', dl: 8800, size: '120 KB', ver: '0.6.1', perms: ['mods', 'ai'], desc: '检查整合包里过期、重复、可能冲突的 Mod；给 AI 助手和 MCP 加一个「体检」工具。', adds: '给 AI 助手和 MCP 加了工具 pack_doctor（只读）；命令面板里多了「整合包体检」' },
    { id: 'fps', name: '帧数记录器', by: 'PerfLab', layer: 'native', tier: 'verified', dl: 4100, size: '2.3 MB', ver: '1.1.0', perms: ['agent', 'bg'], desc: '游戏里每分钟记一次帧数和卡顿，退出后在启动器里画成曲线。', adds: '启动游戏时加载它的 javaagent' },
    { id: 'skinpack', name: '一键领皮肤', by: 'freeskin123', layer: 'script', tier: 'community', dl: 650, size: '210 KB', ver: '2.0.3', perms: ['acct', 'shots', 'net:freeskin123.example'], desc: '每天送一个热门皮肤。', hidden: true },
].map((p) => mk({ host: 'GitHub Releases（也能从 Gitee 镜像下）', sha: (hue(p.name + 'x') * 7919).toString(16).padStart(6, '0') + 'e1c4b9', ...p }));

const REVOKE = { skinpack: '作者账号被盗后，有人用它推送了会把你的游戏名和截图传到 freeskin123.example 的版本（示例），插件市场把它整个下架了' };

export function distManifest(D, pack) {
    return {
        format: 'leafstarter-dist/1',
        name: D.name,
        author: D.author,
        icon: D.icon,
        theme: { color: D.color, font: D.font },
        instances: [{ pack: pack?.name, source: pack?.src, mc: pack?.mc, loader: pack?.loader, locked: true }],
        servers: [{ name: D.srv, address: D.addr, autoJoin: true }],
        home: { notice: D.notice },
        features: { ...D.keep, minimal: D.minimal },
        plugins: [],
        update: 'https://example.cn/launcher/dist.json',
        basedOn: 'LeafStarter >=0.1',
        signature: 'ed25519:4b1f9c…（原型里是假的）',
    };
}

const DIST0 = {
    name: '星辰服启动器', author: '星辰服管理组', icon: 'diamond', color: '#0E7C86', font: 'sans', pack: 'bmc',
    srv: '星辰服', addr: 'play.xingchen.example', notice: '本周六晚 8 点开新地图！进服前记得让启动器更新一下整合包。',
    keep: { download: false, mp: true, ai: false, skin: true, world: false, sync: false }, minimal: false,
};

export function createPlugins(api) {
    const { S } = api;
    S.plug = { inst: { mirror: { on: true, ver: '1.2.0' }, skinpack: { on: true, ver: '2.0.3' } }, off: {}, dev: false, revoked: {}, ...store.get('plug', {}) };
    S.dist = { ...DIST0, ...store.get('dist', {}), keep: { ...DIST0.keep, ...(store.get('dist', {}).keep || {}) } };
    S.ptab = 'installed';
    S.pq = '';
    S.pcat = 'all';
    S.safe = false;
    S.plugChecking = false;
    S.devp = { n: 1, err: null };
    S.distOn = false;
    const save = () => store.set('plug', S.plug);
    const saveDist = () => store.set('dist', S.dist);
    const P = () => S.plug;
    const on = (id) => !S.safe && !!P().inst[id]?.on && !P().revoked[id];
    const count = () => Object.keys(P().inst).filter(on).length + (P().dev && !S.devp.err && !S.safe ? 1 : 0);
    const off = (id) => !!P().off[id] || (S.distOn && S.dist.keep[id] === false);

    // ---------- 外观插件、定制版：换主题变量和字体 ----------
    function applyLook() {
        let el = document.getElementById('plugLook');
        if (!el) {
            el = document.createElement('style');
            el.id = 'plugLook';
            document.head.append(el);
        }
        const acc = (c, ink, soft, a, b, glow) => `--accent:${c};--accent-ink:${ink};--accent-soft:${soft};--go-a:${a};--go-b:${b};--glow:${glow};`;
        const dark = (body) => `@media (prefers-color-scheme: dark){:root:not([data-theme="light"]){${body}}} :root[data-theme="dark"]{${body}}`;
        const css = [];
        if (on('forest') && !S.distOn) {
            css.push(`:root{${acc('#2F7D5B', '#1F6B4A', 'rgb(47 125 91 / .12)', '#2F7D5B', '#0E7C86', 'rgb(47 125 91 / .4)')}--bg:#F1F4EE;--rail:#E5ECE2;--desk:#D9E2D5;--panel:#FCFEFB;--line:#DCE5D9;--line-2:#C9D5C5;--sel:#FFFFFF;}`);
            css.push(dark(`${acc('#6EE7B7', '#A7F3D0', 'rgb(110 231 183 / .14)', '#2F9E6E', '#0E9AA7', 'rgb(47 158 110 / .5)')}--bg:#121814;--rail:#0F1411;--desk:#090C0A;--panel:#18201B;--line:#26302A;--line-2:#334038;--sel:#1F2A23;`));
        }
        if ((on('serif') && !S.distOn) || (S.distOn && S.dist.font === 'serif')) css.push(':root{--font: var(--serif);}');
        if (S.distOn) {
            const c = S.dist.color;
            const body = acc(c, `color-mix(in srgb, ${c} 80%, black)`, `color-mix(in srgb, ${c} 13%, transparent)`, c, `color-mix(in srgb, ${c} 65%, #1D4ED8)`, `color-mix(in srgb, ${c} 45%, transparent)`);
            css.push(`:root{${body}}`);
            css.push(dark(acc(`color-mix(in srgb, ${c} 70%, white)`, `color-mix(in srgb, ${c} 55%, white)`, `color-mix(in srgb, ${c} 18%, transparent)`, c, `color-mix(in srgb, ${c} 65%, #1D4ED8)`, `color-mix(in srgb, ${c} 55%, transparent)`)));
        }
        el.textContent = css.join('\n');
        const app = $('#app');
        app.classList.toggle('p-noai', off('ai'));
        app.classList.toggle('d-nodl', S.distOn && !S.dist.keep.download);
        app.classList.toggle('distro', S.distOn);
        app.classList.toggle('distro-min', S.distOn && S.dist.minimal);
    }

    function repaint() {
        applyLook();
        api.paintTop();
        if (S.page === 'plugins' || S.page === 'home' || S.page === 'settings') api.paintView();
        api.paintLaunch(true);
    }

    // ---------- 装、卸、开关 ----------
    function install(id) {
        const p = MARKET.find((x) => x.id === id);
        api.closeDrawer();
        const t = api.addTask(`装插件 · ${p.name}`, `${p.size} · ${p.host}`);
        const stages = [[0, '下载'], [55, '核对作者签名和市场签名'], [80, '核对 SHA-256'], [95, '装好']];
        api.runTask(t, 1100, () => {
            P().inst[id] = { on: true, ver: p.ver };
            save();
            repaint();
            const hint = { forest: '主题已经换好了', serif: '字体已经换好了', crashrules: '启动「空岛生存」试试：那个冲突现在规则能认出来了', srvcard: '去「生存一服」的概览看看', packdoc: '命令面板里搜「体检」', fps: '下次启动游戏会加载它' }[id] || '';
            toast(`装好了「${p.name}」${hint ? '。' + hint : ''}`);
        }, (pct) => { t.stage = stages.filter(([at]) => pct >= at).at(-1)[1]; });
    }
    function uninstall(id) {
        const p = MARKET.find((x) => x.id === id);
        const was = P().inst[id];
        leave(`p-${id}`, () => {
            delete P().inst[id];
            delete P().revoked[id];
            save();
            repaint();
            toast(`卸载了「${p.name}」，它的数据也一起删了`, {
                undo: () => {
                    P().inst[id] = was;
                    save();
                    repaint();
                },
            });
        });
    }
    function leave(rowId, fn) {
        const row = document.querySelector(`[data-row="${rowId}"]`);
        if (!row) return fn();
        row.classList.add('leave');
        setTimeout(fn, 170);
    }

    function check() {
        S.plugChecking = true;
        api.paintView();
        setTimeout(() => {
            S.plugChecking = false;
            const msgs = [];
            for (const id of Object.keys(REVOKE)) {
                if (P().inst[id] && !P().revoked[id]) {
                    P().revoked[id] = REVOKE[id];
                    msgs.push(`吊销并停用了 1 个（「${MARKET.find((x) => x.id === id).name}」）`);
                }
            }
            if (P().inst.mirror && P().inst.mirror.ver !== '1.3.0') {
                P().inst.mirror.ver = '1.3.0';
                msgs.push('「国内镜像加速」更新到 1.3.0（权限没变，自动更新）');
            }
            if (P().inst.srvcard) msgs.push('「服务器状态卡」新版本要多一项权限，先不更新，等你同意');
            save();
            repaint();
            toast(msgs.length ? `检查完了：${msgs.join('；')}` : '检查完了，都是最新的，没有被吊销的');
        }, 900);
    }

    // ---------- 定制版预览：整个启动器换成服主做的样子 ----------
    let brandWas = '';
    function enterDistro() {
        const D = S.dist;
        const pack = CATALOG.pack.find((p) => p.id === D.pack) || CATALOG.pack[0];
        S.distPrev = { inst: S.inst, page: S.page, tab: S.tab };
        const i = newInstance({ name: pack.name, mc: pack.mc, loader: pack.loader, perf: false, sync: false, dir: 'own' });
        Object.assign(i, { block: D.icon, distro: true, cfg: { join: D.addr } });
        i.servers = [{ id: 'dv1', name: D.srv || '服务器', addr: D.addr, motd: '定制包里预装的服务器', players: '23/100', ping: 31 }];
        api.INSTANCES.push(i);
        S.distInst = i.id;
        S.distOn = true;
        S.inst = i.id;
        S.page = 'home';
        S.tab = 'overview';
        S.world = null;
        const brand = $('.brand');
        brandWas = brand.innerHTML;
        brand.innerHTML = `<i class="blk blk-${D.icon}" style="--s:18px" aria-hidden="true"></i><span class="word">${D.name.replace(/[<>&]/g, '')}</span><span class="proto">定制版预览</span>`;
        brand.dataset.act = 'about';
        brand.title = '关于';
        const bar = document.createElement('div');
        bar.className = 'distbar';
        bar.id = 'distBar';
        bar.innerHTML = `<span>正在预览定制版：玩家打开定制包看到的就是这样</span><button type="button" class="btn sm" data-act="about">关于</button><button type="button" class="btn sm pri" data-act="dist-exit">退出预览</button>`;
        $('.top .cmd').after(bar);
        applyLook();
        api.paintTop();
        api.paintRail();
        api.paintView('page');
        api.paintLaunch(true);
        api.install(i);
    }
    function exitDistro() {
        if (!S.distOn) return;
        const k = api.INSTANCES.findIndex((x) => x.id === S.distInst);
        if (k >= 0) api.INSTANCES.splice(k, 1);
        S.distOn = false;
        Object.assign(S, S.distPrev || {});
        const brand = $('.brand');
        brand.innerHTML = brandWas;
        brand.dataset.act = 'about';
        $('#distBar')?.remove();
        api.closeDrawer();
        applyLook();
        api.paintTop();
        api.paintRail();
        api.paintView('page');
        api.paintLaunch(true);
        toast('退出了定制版预览');
    }

    const distField = (t) => {
        const map = { dName: 'name', dSrvName: 'srv', dSrvAddr: 'addr', dNotice: 'notice', dAuthor: 'author' };
        if (!(t.id in map)) return false;
        S.dist[map[t.id]] = t.value;
        saveDist();
        api.paintDistMock();
        return true;
    };

    return {
        on, off, count, applyLook,
        launchLines(i) {
            const out = [];
            if (on('mirror')) out.push('[LeafStarter/INFO]: 下载源：插件「国内镜像加速」加的 3 个镜像也参与测速');
            if (on('fps')) out.push('[帧数记录器/INFO]: javaagent 已加载（安装时你同意了这项高危权限）');
            return out;
        },
        primary() {
            const d = S.drawer;
            if (d.kind === 'pinst') {
                const p = MARKET.find((x) => x.id === d.id);
                const high = p.perms.some((x) => (PERM[x.split(':')[0]] || [])[1] === 'high');
                if (high && !d.ack) return;
                install(d.id);
            }
        },
        exitDistro,
        actions: {
            ptab: (el) => {
                S.ptab = el.dataset.v;
                api.paintView();
            },
            pcat: (el) => {
                S.pcat = el.dataset.v;
                api.paintView();
            },
            'plug-get': (el) => api.openDrawer({ kind: 'pinst', id: el.dataset.v, ack: false }),
            'plug-del': (el) => uninstall(el.dataset.v),
            'plug-check': () => check(),
            'safe-on': () => {
                S.safe = true;
                repaint();
                toast('以安全模式重新打开了：第三方插件都没加载');
            },
            'safe-off': () => {
                S.safe = false;
                repaint();
                toast('正常打开了，插件都加载回来了');
            },
            'dev-reload': () => {
                S.devp.n += 1;
                api.paintView();
                toast(`「我的第一个插件」热重载好了，用了 38 ms`);
            },
            'dev-crash': () => {
                S.devp.err = "TypeError: Cannot read properties of undefined (reading 'mods') · main.js:12";
                api.paintView();
                toast('「我的第一个插件」出错了，已经停用它；启动器和别的插件都没受影响');
            },
            'dev-fix': () => {
                S.devp.err = null;
                S.devp.n += 1;
                api.paintView();
                toast('重新启用了「我的第一个插件」');
            },
            'pack-doctor': () => {
                const i = api.cur();
                const upd = i.mods.filter((m) => m.update).length;
                toast(`「整合包体检」看完了「${i.name}」：${upd} 个 Mod 有更新，没有重复的，${i.mods.some((m) => m.name === 'Glimmer Render') ? 'Glimmer Render 和 Sodium 有已知冲突' : '没发现已知冲突'}`);
            },
            'd-icon': (el) => {
                S.dist.icon = el.dataset.v;
                saveDist();
                api.paintView();
            },
            'd-color': (el) => {
                S.dist.color = el.dataset.v;
                saveDist();
                api.paintView();
            },
            'd-font': (el) => {
                S.dist.font = el.dataset.v;
                saveDist();
                api.paintView();
            },
            'dist-try': () => enterDistro(),
            'dist-exit': () => exitDistro(),
            about: () => api.openDrawer({ kind: 'about' }),
        },
        change(t) {
            if (t.dataset.pon) {
                P().inst[t.dataset.pon].on = t.checked;
                save();
                repaint();
                return true;
            }
            if (t.dataset.poff) {
                P().off[t.dataset.poff] = !t.checked;
                save();
                repaint();
                // 名字先在概念版的名单里找，真程序多出来的几个（常驻服务器、设备同步）从开关旁边的文字拿
                const name = OFFICIAL.find((x) => x.id === t.dataset.poff)?.name || (t.getAttribute('aria-label') || '').replace(/^启用/, '') || '这个功能';
                toast(t.checked ? `打开了「${name}」` : `关掉了「${name}」，入口都藏起来了`);
                return true;
            }
            if (t.id === 'devMode') {
                P().dev = t.checked;
                save();
                api.paintView();
                if (t.checked) toast('开发者模式：可以从文件夹加载本地插件，改了代码自动热重载');
                return true;
            }
            if (t.id === 'pAck' && S.drawer?.kind === 'pinst') {
                S.drawer.ack = t.checked;
                api.paintDrawer();
                return true;
            }
            if (t.id === 'dPack') {
                S.dist.pack = t.value;
                saveDist();
                api.paintDistMock();
                return true;
            }
            if (t.dataset.dkeep) {
                S.dist.keep[t.dataset.dkeep] = t.checked;
                saveDist();
                api.paintDistMock();
                return true;
            }
            if (t.id === 'dMin') {
                S.dist.minimal = t.checked;
                saveDist();
                api.paintDistMock();
                return true;
            }
            return false;
        },
        input(t) {
            if (t.id === 'plugQ') {
                S.pq = t.value;
                api.paintPlugGrid();
                return true;
            }
            return distField(t);
        },
    };
}
