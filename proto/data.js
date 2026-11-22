// 示例数据：界面是真的，这些实例、Mod、存档、服务器都是编的
const now = Date.now();
const MIN = 60e3;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

export const DIRS = [
    { id: 'own', name: 'LeafStarter', path: 'D:\\Games\\.minecraft' },
    { id: 'pcl', name: 'PCL', path: 'D:\\PCL2\\.minecraft' },
    { id: 'hmcl', name: 'HMCL', path: 'D:\\HMCL\\.minecraft' },
];

export const ACCOUNTS = [
    { id: 'msa', name: 'PingYi', label: '微软正版', face: 'pingyi' },
    { id: 'off', name: 'Steve', label: '离线', face: 'steve' },
];

export const JAVAS = [
    { id: 'j21', label: 'Java 21 · 官方运行时', major: 21 },
    { id: 'j17', label: 'Java 17 · Eclipse Adoptium', major: 17 },
    { id: 'j8', label: 'Java 8 · PCL 下载的运行时', major: 8 },
];

// 全局默认；实例里没单独设的项都跟这里走
export const GLOBAL = { java: 'auto', mem: 'auto', win: '854x480', account: 'default', join: 'none', jvm: '', pre: '' };

export function hue(s) {
    let h = 0;
    for (const ch of s) h = (h * 31 + ch.charCodeAt(0)) % 360;
    return h;
}
let seq = 0;
const mod = (name, ver, src, extra = {}) => ({ id: 'm' + ++seq, name, ver, src, on: true, h: hue(name), ...extra });

// 「流畅优先」会加的那组优化 Mod
export function perfMods(loader) {
    const forge = loader === 'Forge' || loader === 'NeoForge';
    return (forge
        ? [['Embeddium', '0.3.31'], ['FerriteCore', '6.0.1'], ['ModernFix', '5.19.4'], ['ImmediatelyFast', '1.2.18'], ['Entity Culling', '1.6.6']]
        : [['Sodium', '0.5.11'], ['Lithium', '0.11.2'], ['FerriteCore', '6.0.1'], ['ImmediatelyFast', '1.2.18'], ['Entity Culling', '1.6.6'], ['ModernFix', '5.19.4']]
    ).map(([n, v]) => mod(n, v, 'Modrinth', { perf: true }));
}

const shots = (n, from) => Array.from({ length: n }, (_, k) => ({ id: 's' + ++seq, at: from - k * 17 * HOUR - k * k * 41 * MIN, seed: (k * 47 + n * 13) % 360 }));

export const INSTANCES = [
    {
        id: 'survival', dir: 'own', name: '生存一服', block: 'grass',
        mc: '1.20.1', loader: 'Fabric', loaderVer: '0.16.9', javaNeed: 17,
        lastPlayed: now - 2 * HOUR - 4 * MIN, played: 2770,
        lastRun: { ok: true, at: now - 2 * HOUR - 4 * MIN, min: 83 },
        sync: { on: true, state: 'ok', at: now - 2 * HOUR - 3 * MIN, device: 'Zalith Launcher 2 · 小米 14' },
        perf: true, cfg: {},
        mods: [
            mod('Fabric API', '0.92.2', 'Modrinth'),
            ...perfMods('Fabric').map((m) => (m.name === 'Sodium' ? { ...m, ver: '0.5.8', update: '0.5.11' } : m)),
            mod('Iris Shaders', '1.7.2', 'Modrinth', { update: '1.7.6' }),
            mod('Mod Menu', '7.2.2', 'Modrinth'),
            mod("Xaero's Minimap", '24.6.1', 'CurseForge'),
            mod('Jade', '11.12.3', 'CurseForge', { update: '11.13.1' }),
            mod('AppleSkin', '2.5.1', 'Modrinth'),
            mod('EMI', '1.1.18', 'Modrinth'),
            mod('Waystones', '14.1.6', 'CurseForge', { on: false }),
            mod('Balm', '7.3.9', 'CurseForge', { on: false, note: 'Waystones 的前置' }),
        ],
        worlds: [
            {
                id: 'w1', name: '主世界生存', mode: '生存', diff: '困难', last: now - 2 * HOUR - 4 * MIN, size: '412 MB',
                history: [
                    { dev: '电脑', at: now - 2 * HOUR - 3 * MIN, note: '退出游戏后自动上传' },
                    { dev: '手机 · Zalith Launcher 2', at: now - 9 * HOUR, note: '在学校玩了 40 分钟' },
                    { dev: '电脑', at: now - DAY - HOUR, note: '' },
                    { dev: '手机 · Zalith Launcher 2', at: now - 2 * DAY - 5 * HOUR, note: '' },
                    { dev: '电脑', at: now - 3 * DAY, note: '' },
                ],
            },
            { id: 'w2', name: '和小明的联机图', mode: '生存', diff: '普通', last: now - 3 * DAY, size: '96 MB', history: [{ dev: '电脑', at: now - 3 * DAY, note: '' }] },
            { id: 'w3', name: '建筑测试', mode: '创造', diff: '和平', last: now - 12 * DAY, size: '38 MB', history: [] },
        ],
        servers: [
            { id: 'v1', name: 'LeafHub 服务器', addr: 'mc.pingyi.me', motd: '生存服 · 1.20.1', players: '7/50', ping: 38 },
            { id: 'v2', name: '宿舍局域网', addr: '192.168.1.20:25565', motd: '小明开的局域网世界', players: '2/8', ping: 3 },
        ],
        shots: shots(9, now - 2 * HOUR - 30 * MIN),
    },
    {
        id: 'create', dir: 'own', name: '机械动力整合包', block: 'brass',
        mc: '1.20.1', loader: 'Forge', loaderVer: '47.3.0', javaNeed: 17,
        lastPlayed: now - 3 * DAY, played: 640,
        lastRun: { ok: false, at: now - 3 * DAY, min: 0 },
        sync: { on: false, state: 'ok', at: 0, device: 'Zalith Launcher 2 · 小米 14' },
        perf: false, cfg: { java: 'j8' }, crashDemo: true,
        diag: {
            title: '游戏崩溃了：Java 版本不对',
            detail: '这个整合包需要 Java 17，可实例设置里指定了 Java 8（从 PCL 导入时带过来的）。',
            fix: '改回自动选择（Java 17）并重新启动',
        },
        mods: [
            mod('Create', '0.5.1.j', 'CurseForge'),
            mod('Flywheel', '0.6.11', 'CurseForge', { note: 'Create 的前置' }),
            mod("Create: Steam 'n' Rails", '1.6.4', 'CurseForge'),
            mod('Create Crafts & Additions', '1.2.4', 'CurseForge'),
            mod('Create Deco', '2.0.2', 'CurseForge'),
            mod("Farmer's Delight", '1.2.4', 'CurseForge'),
            mod('Just Enough Items', '15.20.0.105', 'CurseForge'),
            mod('Jade', '11.12.3', 'CurseForge'),
            mod("Xaero's Minimap", '24.6.1', 'CurseForge'),
            mod('Supplementaries', '2.8.17', 'CurseForge'),
            mod('Moonlight Lib', '2.13.32', 'CurseForge', { note: 'Supplementaries 的前置' }),
            mod('Waystones', '14.1.6', 'CurseForge'),
        ],
        worlds: [{ id: 'w4', name: '齿轮工厂', mode: '生存', diff: '普通', last: now - 3 * DAY, size: '188 MB', history: [] }],
        servers: [],
        shots: shots(4, now - 3 * DAY),
    },
    {
        id: 'vanilla', dir: 'own', name: '原版 1.21.4', block: 'stone',
        mc: '1.21.4', loader: null, loaderVer: '', javaNeed: 21,
        lastPlayed: now - 9 * DAY, played: 312,
        lastRun: { ok: true, at: now - 9 * DAY, min: 47 },
        sync: { on: false, state: 'ok', at: 0, device: 'Zalith Launcher 2 · 小米 14' },
        perf: false, cfg: {},
        mods: [],
        worlds: [{ id: 'w5', name: '新的世界', mode: '生存', diff: '普通', last: now - 9 * DAY, size: '54 MB', history: [] }],
        servers: [{ id: 'v3', name: 'LeafHub 服务器', addr: 'mc.pingyi.me', motd: '生存服 · 1.20.1（版本不同进不去）', players: '7/50', ping: 41 }],
        shots: shots(2, now - 9 * DAY),
    },
    {
        id: 'pcl1122', dir: 'pcl', name: '1.12.2-Forge', block: 'cobble',
        mc: '1.12.2', loader: 'Forge', loaderVer: '14.23.5.2860', javaNeed: 8,
        lastPlayed: now - 41 * DAY, played: 5230,
        lastRun: { ok: true, at: now - 41 * DAY, min: 126 },
        sync: { on: false, state: 'ok', at: 0, device: 'Zalith Launcher 2 · 小米 14' },
        perf: false, cfg: {},
        mods: [
            mod('Just Enough Items', '4.16.1.1012', 'CurseForge'),
            mod('Thermal Expansion', '5.5.7', 'CurseForge'),
            mod('Applied Energistics 2', 'rv6-stable-7', 'CurseForge'),
            mod('Journeymap', '5.7.1', 'CurseForge'),
        ],
        worlds: [{ id: 'w6', name: '科技存档', mode: '生存', diff: '普通', last: now - 41 * DAY, size: '640 MB', history: [] }],
        servers: [],
        shots: shots(6, now - 41 * DAY),
    },
    {
        id: 'hmcl189', dir: 'hmcl', name: '1.8.9 PvP', block: 'nether',
        mc: '1.8.9', loader: null, loaderVer: '', javaNeed: 8,
        lastPlayed: now - 20 * DAY, played: 905,
        lastRun: { ok: true, at: now - 20 * DAY, min: 35 },
        sync: { on: false, state: 'ok', at: 0, device: 'Zalith Launcher 2 · 小米 14' },
        perf: false, cfg: {},
        mods: [],
        worlds: [],
        servers: [{ id: 'v4', name: '起床战争练习服', addr: 'bw.example.cn', motd: '示例服务器', players: '213/1000', ping: 62 }],
        shots: shots(3, now - 20 * DAY),
    },
];

export const TASKS = [
    { id: 't1', title: '更新 2 个 Mod', sub: '生存一服 · Lithium、FerriteCore', state: 'done', pct: 100, at: now - 26 * MIN },
    { id: 't2', title: '同步 生存一服', sub: '上传 4 个区块文件 · 1.2 MB', state: 'done', pct: 100, at: now - 2 * HOUR - 3 * MIN },
    { id: 't3', title: '安装 原版 1.21.4', sub: '1,912 个文件 · 486 MB', state: 'done', pct: 100, at: now - 9 * DAY },
];

// 新建实例时能选的版本（示例列表，正式版读官方版本清单）
export const VERSIONS = [
    ['1.21.4', '2024-12-03'], ['1.21.1', '2024-08-08'], ['1.20.6', '2024-04-29'], ['1.20.4', '2023-12-07'],
    ['1.20.1', '2023-06-12'], ['1.19.4', '2023-03-14'], ['1.18.2', '2022-02-28'], ['1.16.5', '2021-01-15'],
    ['1.12.2', '2017-09-18'], ['1.8.9', '2015-12-09'], ['1.7.10', '2014-06-26'],
];

const mcNum = (mc) => {
    const [a, b, c = 0] = mc.split('.').map(Number);
    return a + b / 100 + c / 10000;
};
export const javaNeedOf = (mc) => (mcNum(mc) >= 1.2005 ? 21 : mcNum(mc) >= 1.18 ? 17 : mcNum(mc) >= 1.17 ? 16 : 8);

const FORGE = { '1.20.1': '47.3.0', '1.16.5': '36.2.39', '1.12.2': '14.23.5.2860', '1.8.9': '11.15.1.2318', '1.7.10': '10.13.4.1614' };
const NEO = { '1.20.1': '47.1.106', '1.20.4': '20.4.237', '1.20.6': '20.6.119', '1.21.1': '21.1.77', '1.21.4': '21.4.124' };
// 加载器支不支持这个版本、推荐哪个版本；null 表示不支持
export function loaderVersion(loader, mc) {
    if (!loader) return '';
    if (loader === 'Fabric') return mcNum(mc) >= 1.14 ? '0.16.9' : null;
    if (loader === 'Quilt') return mcNum(mc) >= 1.14 ? '0.27.1' : null;
    if (loader === 'Forge') return FORGE[mc] || '最新稳定版';
    if (loader === 'NeoForge') return NEO[mc] || null;
    return null;
}

export function newInstance({ name, mc, loader, perf, sync, dir }) {
    const withPerf = !!(perf && loader);
    return {
        id: 'i' + Date.now().toString(36), dir, name, block: 'copper',
        mc, loader: loader || null, loaderVer: loaderVersion(loader, mc), javaNeed: javaNeedOf(mc),
        lastPlayed: 0, played: 0, lastRun: null,
        sync: { on: !!sync, state: 'ok', at: Date.now(), device: 'Zalith Launcher 2 · 小米 14' },
        perf: withPerf, cfg: {},
        mods: [...(loader === 'Fabric' ? [mod('Fabric API', '0.92.2', 'Modrinth')] : []), ...(withPerf ? perfMods(loader) : [])],
        worlds: [], servers: [], shots: [],
    };
}

// 下载页的示例结果
export const CATALOG = {
    mod: [
        { id: 'xaero-wm', name: "Xaero's World Map", by: 'xaero96', sum: '全屏世界地图，记下走过的地方', src: 'CurseForge', loaders: ['Fabric', 'Forge', 'NeoForge', 'Quilt'] },
        { id: 'waystones', name: 'Waystones', by: 'BlayTheNinth', sum: '在传送石碑之间来回传送', src: 'CurseForge', loaders: ['Fabric', 'Forge', 'NeoForge'] },
        { id: 'appleskin', name: 'AppleSkin', by: 'squeek502', sum: '显示每样食物能补多少饱食度', src: 'Modrinth', loaders: ['Fabric', 'Forge', 'NeoForge', 'Quilt'] },
        { id: 'jei', name: 'Just Enough Items', by: 'mezz', sum: '查物品和合成配方', src: 'CurseForge', loaders: ['Forge', 'NeoForge', 'Fabric'] },
        { id: 'jade', name: 'Jade', by: 'Snownee', sum: '准星指着方块时，显示它是什么、属于哪个 Mod', src: 'Modrinth', loaders: ['Fabric', 'Forge', 'NeoForge', 'Quilt'] },
        { id: 'iris', name: 'Iris Shaders', by: 'IMS', sum: '加载光影，和 Sodium 一起用', src: 'Modrinth', loaders: ['Fabric', 'Quilt', 'NeoForge'] },
        { id: 'sodium', name: 'Sodium', by: 'CaffeineMC', sum: '重写渲染，帧数明显更高', src: 'Modrinth', loaders: ['Fabric', 'Quilt', 'NeoForge'] },
        { id: 'modmenu', name: 'Mod Menu', by: 'TerraformersMC', sum: '在游戏里列出装了哪些 Mod', src: 'Modrinth', loaders: ['Fabric', 'Quilt'] },
        { id: 'emi', name: 'EMI', by: 'emilyploszaj', sum: '查物品和配方，界面更新', src: 'Modrinth', loaders: ['Fabric', 'Forge', 'NeoForge', 'Quilt'] },
        { id: 'create', name: 'Create', by: 'simibubi', sum: '齿轮、传送带和各种机械装置', src: 'CurseForge', loaders: ['Forge', 'NeoForge'] },
    ],
    pack: [
        { id: 'fo', name: 'Fabulously Optimized', by: 'robotkoer', sum: '只装优化 Mod 的整合包，装上就流畅', src: 'Modrinth', mc: '1.21.1', loader: 'Fabric' },
        { id: 'atm9', name: 'All the Mods 9', by: 'ATMTeam', sum: '四百多个 Mod 的大型整合包', src: 'CurseForge', mc: '1.20.1', loader: 'Forge' },
        { id: 'bmc', name: 'Better MC', by: 'LunaPixelStudios', sum: '在原版基础上加强探索和生物群系', src: 'CurseForge', mc: '1.20.1', loader: 'Fabric' },
    ],
    rp: [
        { id: 'faithful', name: 'Faithful 32x', by: 'Faithful', sum: '原版风格的 32 倍清晰材质', src: 'CurseForge' },
        { id: 'fresh', name: 'Fresh Animations', by: 'FreshLX', sum: '生物动作更生动（需要 EMF 和 ETF）', src: 'Modrinth' },
    ],
    shader: [
        { id: 'compl', name: 'Complementary Shaders - Reimagined', by: 'EminGT', sum: '画面均衡，低配也能开', src: 'Modrinth' },
        { id: 'bsl', name: 'BSL Shaders', by: 'Capt Tatsu', sum: '经典光影，色彩明亮', src: 'CurseForge' },
    ],
};
for (const list of Object.values(CATALOG)) for (const it of list) it.h = hue(it.name);

// 日志：启动过程逐行冒出来，之后偶尔一两行
export function bootLog(inst, user, java) {
    const mods = inst.mods.filter((m) => m.on).length;
    return [
        `[main/INFO]: Loading Minecraft ${inst.mc}${inst.loader ? ` with ${inst.loader} ${inst.loaderVer}` : ''}`,
        `[main/INFO]: Java is OpenJDK 64-Bit Server VM, version ${java === 8 ? '1.8.0_51' : java + '.0.5'}`,
        inst.loader ? `[main/INFO]: Loading ${mods} mods` : '[main/INFO]: No mods',
        `[Render thread/INFO]: Setting user: ${user}`,
        '[Render thread/INFO]: Backend library: LWJGL version 3.3.1 SNAPSHOT',
        '[Render thread/INFO]: Reloading ResourceManager: vanilla' + (inst.loader ? ', mods' : ''),
        '[Render thread/WARN]: Missing sound for event: minecraft:item.goat_horn.play',
        '[Render thread/INFO]: Sound engine started',
        '[Render thread/INFO]: Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas',
        `[Server thread/INFO]: Starting integrated minecraft server version ${inst.mc}`,
        '[Server thread/INFO]: Preparing spawn area: 84%',
        `[Server thread/INFO]: ${user} joined the game`,
        '[Server thread/INFO]: Changing view distance to 12, from 10',
        '[Render thread/WARN]: Received passengers for unknown entity',
        '[Server thread/INFO]: Saving chunks for level \'ServerLevel[world]\'/minecraft:overworld',
    ];
}

export function crashLog(inst) {
    return [
        `[main/INFO]: Loading Minecraft ${inst.mc} with ${inst.loader} ${inst.loaderVer}`,
        '[main/INFO]: Java is OpenJDK 64-Bit Server VM, version 1.8.0_51',
        '[main/ERROR]: java.lang.UnsupportedClassVersionError: cpw/mods/bootstraplauncher/BootstrapLauncher has been compiled by a more recent version of the Java Runtime (class file version 61.0), this version of the Java Runtime only recognizes class file versions up to 52.0',
        '[main/ERROR]: 游戏进程退出，退出码 1',
    ];
}
for (const i of INSTANCES) i.lastLog = (i.crashDemo ? crashLog(i) : bootLog(i, 'PingYi', i.javaNeed)).map((l, k) => `[21:40:${String(k * 2).padStart(2, '0')}] ${l}`);
