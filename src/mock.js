// 在普通浏览器里直接打开界面时用的演示数据（打包进应用后不会加载）
const listeners = {};
const emit = (ev, payload) => (listeners[ev] || []).forEach((f) => f({ payload }));
export const listen = async (ev, f) => {
    (listeners[ev] ||= []).push(f);
    return () => {};
};

const releases = ['26.3', '26.2', '26.1.2', '26.1', '1.21.11', '1.21.10', '1.21.8', '1.21.4', '1.21.1', '1.20.6', '1.20.4', '1.20.1', '1.19.4', '1.19.2', '1.18.2', '1.17.1', '1.16.5', '1.12.2', '1.8.9', '1.7.10'];
const snapshots = ['26.4-snapshot-5', '26.4-snapshot-4', '26.3-rc-3', '26.3-pre-2'];
const manifest = {
    latest: { release: '26.3', snapshot: '26.4-snapshot-5' },
    versions: [
        ...snapshots.map((id, i) => ({ id, type: 'snapshot', url: '', releaseTime: `2026-10-${String(20 - i * 3).padStart(2, '0')}T10:00:00+00:00` })),
        ...releases.map((id, i) => ({ id, type: 'release', url: '', releaseTime: `${2026 - Math.floor(i / 3)}-0${(i % 9) + 1}-12T10:00:00+00:00` })),
        { id: 'b1.7.3', type: 'old_beta', url: '', releaseTime: '2011-07-08T00:00:00+00:00' },
        { id: 'a1.2.6', type: 'old_alpha', url: '', releaseTime: '2010-12-03T00:00:00+00:00' },
    ],
};
let targets = [
    { id: 'pcl1', kind: 'pcl', launcher: 'PCL2', path: 'E:\\PCL\\.minecraft', access: 'direct', exe: 'E:\\PCL\\Plain Craft Launcher 2.exe', versions: [{ name: '1.21.1-Fabric 0.19.3', loader: 'Fabric' }], manual: false },
    { id: 'hm1', kind: 'hmcl', launcher: 'HMCL', path: 'D:\\Games\\HMCL\\.minecraft', access: 'direct', exe: 'D:\\Games\\HMCL\\HMCL-3.6.12.exe', versions: [{ name: '1.20.1-Forge_47.3.0', loader: 'Forge' }, { name: '1.20.1', loader: '' }], manual: false },
    { id: 'off1', kind: 'official', launcher: '官方启动器', path: 'C:\\Users\\Steve\\AppData\\Roaming\\.minecraft', access: 'direct', versions: Array.from({ length: 13 }, (_, i) => ({ name: 'v' + i, loader: '' })), manual: false },
];
const sources = {
    settings: { mode: 'auto', preferred: 'bmclapi', threads: 24, custom: [] },
    best: 'bmclapi',
    sources: [
        { id: 'bmclapi', name: 'BMCLAPI 镜像', kind: 'bmclapi', base: 'https://bmclapi2.bangbang93.com', builtin: true, enabled: true, latencyMs: 46, testSpeed: 9.6e6, liveSpeed: null, ok: 12, fail: 0, cooling: false, testedAt: 1, score: 1, rank: 1 },
        { id: 'official', name: '官方源', kind: 'official', base: '', builtin: true, enabled: true, latencyMs: 380, testSpeed: 2.1e6, liveSpeed: null, ok: 4, fail: 1, lastError: '传输停滞', cooling: false, testedAt: 1, score: 0.5, rank: 2 },
    ],
};
const loaderLists = {
    fabric: ['0.17.3', '0.17.2', '0.16.14', '0.16.10'],
    quilt: ['0.29.1', '0.28.4-beta.2'],
    forge: ['52.1.0', '52.0.40', '52.0.22'],
    neoforge: ['26.3.0.95', '26.3.0.94-beta'],
};
let nextId = 1;
const tasks = new Map();

function simulate(task) {
    const total = 4200;
    const bytes = 682e6;
    let t = 0;
    const iv = setInterval(() => {
        t += 1;
        const f = Math.min(1, t / 40);
        Object.assign(task, {
            stage: f < 0.1 ? '获取资源索引' : f < 1 ? `下载 ${total} 个文件（约 650.4 MB）` : '完成',
            filesDone: Math.round(total * f),
            filesTotal: total,
            bytesDone: Math.round(bytes * f),
            bytesTotal: bytes,
            speed: f < 1 ? 7.5e6 + Math.random() * 3e6 : 0,
            sources: { bmclapi: Math.round(bytes * f * 0.82), official: Math.round(bytes * f * 0.18) },
            log: ['获取版本清单', '读取版本信息', '获取资源索引', '下载文件'],
        });
        if (f >= 1) {
            task.state = 'done';
            task.resultPath = 'E:\\PCL\\.minecraft\\versions\\' + task.subtitle;
            clearInterval(iv);
        }
        emit('task-update', { ...task });
    }, 300);
}

export async function invoke(cmd, args = {}) {
    await new Promise((r) => setTimeout(r, 120));
    switch (cmd) {
        case 'app_info': return { platform: matchMedia('(max-width: 899px)').matches ? 'android' : 'windows', version: '0.1.0', dataDir: '' };
        case 'get_manifest': return manifest;
        case 'list_loaders': return (loaderLists[args.kind] || []).map((v, i) => ({ id: v, version: v, stable: !v.includes('beta'), tag: i === 0 ? '推荐' : null }));
        case 'scan_targets': return targets;
        case 'remove_target': targets = targets.filter((t) => t.id !== args.id); return targets;
        case 'add_manual_dir': return targets;
        case 'pick_folder': return null;
        case 'android_status': return { allFiles: true };
        case 'get_sources': case 'test_sources': return sources;
        case 'save_net_settings': sources.settings = args.settings; return sources;
        case 'list_java': return [{ path: 'C:\\Program Files\\Java\\jdk-21\\bin\\java.exe', major: 21, version: '21.0.5', source: 'JAVA_HOME' }];
        case 'list_tasks': return [...tasks.values()];
        case 'clear_tasks': [...tasks].forEach(([k, v]) => v.state !== 'running' && tasks.delete(k)); return [...tasks.values()];
        case 'start_install': {
            const r = args.req;
            const task = { id: nextId++, title: r.loader ? `${r.mcVersion} · ${r.loader.kind} ${r.loader.version}` : r.mcVersion, subtitle: r.name, target: 'PCL2 · E:\\PCL\\.minecraft', state: 'running', stage: '准备中', filesDone: 0, filesTotal: 0, bytesDone: 0, bytesTotal: 0, speed: 0, sources: {}, log: [] };
            tasks.set(task.id, task);
            simulate(task);
            return task.id;
        }
        case 'cancel_task': { const t = tasks.get(args.id); if (t) { t.state = 'cancelled'; t.stage = '已取消'; emit('task-update', { ...t }); } return null; }
        default: return null;
    }
}
