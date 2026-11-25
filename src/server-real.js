// 真程序：常驻服务器（M10）的动作和事件。界面在 views/server-real.js，后端在 src-tauri/src/server
import { $, toast, ask, askText } from './ui.js';
import { invoke, listen, R } from './real.js';
import { STEPS, srvWorldPop } from './views/server-real.js';

export function createServers(api) {
    const { S } = api;
    R.srv = { list: [], loaded: false, status: new Map(), steps: new Map() };
    const onTab = () => S.page === 'home' && S.tab === 'servers';
    const repaint = () => onTab() && api.paintBody();
    const byId = (id) => R.srv.list.find((x) => x.id === id);
    const val = (sel) => $(sel)?.value ?? '';
    const fail = (e) => toast(String(e), { ms: 7000 });

    async function load() {
        R.srv.list = await invoke('server_list').catch(() => R.srv.list);
        R.srv.loaded = true;
        repaint();
    }

    async function status(id) {
        R.srv.status.set(id, { loading: true });
        repaint();
        try {
            R.srv.status.set(id, await invoke('server_status', { id }));
        } catch (e) {
            R.srv.status.set(id, { error: String(e) });
        }
        repaint();
    }

    // 进到「联机」标签：查一下这个实例的服务器状态
    function enter() {
        const i = api.cur();
        if (!i) return;
        for (const s of R.srv.list.filter((x) => x.inst === i.id)) {
            if ((s.kind !== 'ssh' || s.setup) && !R.srv.status.get(s.id)) status(s.id);
        }
    }

    listen('server-step', (e) => {
        const p = e.payload;
        const m = R.srv.steps.get(p.id) || {};
        m[p.step] = { state: p.state, out: p.out };
        R.srv.steps.set(p.id, m);
        if (p.state === 'fail' || (p.state === 'done' && p.step === 'start')) load().then(() => p.state === 'done' && status(p.id));
        else repaint();
    });
    listen('servers-changed', () => load());
    load().then(() => onTab() && enter());

    // 加服务器面板：重画之前把已经填的记下来
    const keepAdd = () => {
        const d = S.drawer;
        if (!d) return;
        d.host = val('#saHost') || d.host;
        d.user = val('#saUser') || d.user;
        d.port = val('#saPort') || d.port;
        d.name = val('#saName') || d.name;
    };

    const actions = {
        'srv-add': () => api.openDrawer({ kind: 'srvadd', mode: 'pw' }),
        'sa-mode': (el) => {
            keepAdd();
            S.drawer.mode = el.dataset.v;
            api.paintDrawer();
        },
        'sa-go': async () => {
            keepAdd();
            const d = S.drawer;
            const i = api.cur();
            const form = { name: d.name || '', host: (d.host || '').trim(), sshPort: Number(d.port) || 22, user: d.user || 'root', password: d.mode === 'pw' ? val('#saPass') : null, keyText: d.mode === 'key' ? d.keyText || null : null, inst: i.id };
            if (!form.host) return toast('填一下服务器 IP');
            d.state = 'busy';
            d.error = '';
            api.paintDrawer();
            try {
                const v = await invoke('server_add_ssh', { form });
                api.closeDrawer();
                await load();
                toast(`连上了「${v.name}」，启动器的密钥已经装好，密码没有保存。点「开服」就开始`, { ms: 7000 });
            } catch (e) {
                d.state = '';
                d.error = String(e);
                api.paintDrawer();
            }
        },
        'panel-add': () => api.openDrawer({ kind: 'paneladd', pk: 'mcsm' }),
        'pa-kind': (el) => {
            S.drawer.base = val('#paBase');
            S.drawer.pk = el.dataset.v;
            S.drawer.list = null;
            S.drawer.pick = null;
            api.paintDrawer();
        },
        'pa-probe': async () => {
            const d = S.drawer;
            d.base = val('#paBase').trim();
            d.key = val('#paKey').trim() || d.key;
            if (!d.base || !d.key) return toast('填面板地址和 API Key');
            d.state = 'busy';
            d.error = '';
            api.paintDrawer();
            try {
                d.list = await invoke('panel_probe', { kind: d.pk, base: d.base, key: d.key });
                d.pick = d.list.length === 1 ? 0 : null;
            } catch (e) {
                d.error = String(e);
            }
            d.state = '';
            api.paintDrawer();
        },
        'pa-pick': (el) => {
            S.drawer.addr = val('#paAddr') || S.drawer.addr;
            S.drawer.pick = Number(el.dataset.v);
            api.paintDrawer();
        },
        'pa-go': async () => {
            const d = S.drawer;
            const x = d.list?.[d.pick];
            if (!x) return;
            const addr = val('#paAddr').trim();
            if (!addr) return toast('填一下朋友连的地址');
            try {
                await invoke('panel_add', { form: { kind: d.pk, base: d.base, key: d.key, panelInst: x.id, panelDaemon: x.daemon, name: x.name || '面板服', inst: api.cur().id, address: addr } });
                api.closeDrawer();
                await load();
                enter();
                toast('加好了');
            } catch (e) {
                d.error = String(e);
                api.paintDrawer();
            }
        },
        'srv-setup': async (el) => {
            const id = el.dataset.id;
            R.srv.steps.set(id, {});
            try {
                await invoke('server_setup', { id, only: null });
                if (S.drawer?.kind === 'srvcfg') api.closeDrawer();
                toast('开始开服了，每一步都显示在这里；装 Forge 这类服务端要几分钟');
            } catch (e) {
                fail(e);
            }
            repaint();
        },
        'srv-ai': (el) => {
            const s = byId(el.dataset.id);
            if (!s?.failed) return;
            const title = STEPS.find((x) => x[0] === s.failed[0])?.[1] || s.failed[0];
            api.agent?.ask?.(`我在用 LeafStarter 的开服剧本给服务器「${s.name}」（${s.mc} ${s.loader || '原版'}）开服，「${title}」这一步失败了。输出的最后几行：\n\n${s.failed[1].split('\n').slice(-30).join('\n')}\n\n帮我看看是什么原因、该怎么处理。`);
        },
        'srv-power': async (el) => {
            const { id, v } = el.dataset;
            if (v === 'stop' && !(await ask('停掉服务器？在线的玩家会被踢出去。'))) return;
            el.disabled = true;
            try {
                toast(await invoke('server_power', { id, action: v }));
            } catch (e) {
                fail(e);
            }
            setTimeout(() => status(id), v === 'stop' ? 500 : 4000);
        },
        'srv-status': (el) => status(el.dataset.id),
        'srv-console': async (el) => {
            api.openDrawer({ kind: 'srvcon', id: el.dataset.id, out: [] });
            actions['srv-log'](el);
        },
        'srv-log': async (el) => {
            const d = S.drawer;
            try {
                d.log = (await invoke('server_log', { id: el.dataset.id, lines: 300 })) || '（还没有日志）';
            } catch (e) {
                d.log = String(e);
            }
            if (S.drawer === d) {
                api.paintDrawer();
                const l = $('#srvLog');
                if (l) l.scrollTop = l.scrollHeight;
            }
        },
        'srv-send': async () => {
            const d = S.drawer;
            const cmd = val('#srvCmd').trim();
            if (!cmd) return;
            let r;
            try {
                r = await invoke('server_cmd', { id: d.id, command: cmd });
            } catch (e) {
                r = String(e);
            }
            d.out.push([cmd, r]);
            api.paintDrawer();
            $('#srvCmd')?.focus();
        },
        'srv-sync': async (el) => {
            const s = byId(el.dataset.id);
            try {
                if (s.kind === 'ssh') await invoke('server_setup', { id: s.id, only: ['mods'] });
                else await invoke('panel_sync_mods', { id: s.id });
                toast('开始同步 Mod（只传服务端要的），进度在「任务」页；完了重启服务器生效');
            } catch (e) {
                fail(e);
            }
        },
        'srv-backup': async (el) => {
            el.disabled = true;
            try {
                toast(await invoke('server_backup', { id: el.dataset.id }));
            } catch (e) {
                fail(e);
            }
            el.disabled = false;
        },
        'srv-world': (el) => {
            const s = byId(el.dataset.id);
            if (s) api.openPop(el, srvWorldPop(api.ctx(), s), 'above');
        },
        'srv-down': async (el) => {
            api.closePop();
            try {
                await invoke('server_world_down', { id: el.dataset.id });
                toast('开始拉世界，进度在「任务」页；拉完在「存档」里');
            } catch (e) {
                fail(e);
            }
        },
        'srv-up': async (el) => {
            api.closePop();
            if (!(await ask('把这个世界传上去当服务器的世界？服务器上原来的会先备份，服务器会重启一下。'))) return;
            try {
                await invoke('server_world_up', { id: el.dataset.id, world: el.dataset.v });
                toast('开始上传，进度在「任务」页');
            } catch (e) {
                fail(e);
            }
        },
        'srv-cfg': (el) => api.openDrawer({ kind: 'srvcfg', id: el.dataset.id, prov: 'aliyun' }),
        'sc-save': async () => {
            const d = S.drawer;
            const s = byId(d.id);
            const patch = { name: val('#scName') };
            if (s.kind === 'ssh') {
                Object.assign(patch, { memMb: Number(val('#scMem')) || 0, port: Number(val('#scPort')) || 25565, online: !!$('#scOnline')?.checked, motd: val('#scMotd'), whitelist: val('#scWl').split('\n') });
            }
            try {
                await invoke('server_set', { id: d.id, patch });
                await load();
                api.paintDrawer();
                toast(s.setup ? '存好了。重新跑一遍剧本才会用到服务器上' : '存好了');
            } catch (e) {
                fail(e);
            }
        },
        'fw-prov': (el) => {
            S.drawer.prov = el.dataset.v;
            api.paintDrawer();
        },
        'srv-fw': async (el) => {
            const ak = val('#fwAk').trim();
            const sk = val('#fwSk').trim();
            if (!ak || !sk) return toast('两个都要填');
            el.disabled = true;
            try {
                toast(await invoke('server_firewall', { id: el.dataset.id, provider: S.drawer.prov || 'aliyun', ak, sk }), { ms: 7000 });
                const a = $('#fwAk');
                const b = $('#fwSk');
                if (a) a.value = '';
                if (b) b.value = '';
            } catch (e) {
                fail(e);
            }
            el.disabled = false;
        },
        'srv-harden': async (el) => {
            if (!(await ask('关掉这台服务器的密码登录？以后只能用密钥登录（启动器已经确认过密钥能用）。如果你自己还要用密码登录，先别关。'))) return;
            try {
                toast(await invoke('server_harden', { id: el.dataset.id }));
            } catch (e) {
                fail(e);
            }
        },
        'srv-retrust': async (el) => {
            if (!(await ask('只有在你重装过服务器系统时才需要重新信任。如果没重装过，指纹变了可能是有人在冒充这台服务器。确定重新信任？'))) return;
            try {
                const fp = await invoke('server_retrust', { id: el.dataset.id });
                await load();
                api.paintDrawer();
                toast(`重新记下了服务器身份：${fp}`);
            } catch (e) {
                fail(e);
            }
        },
        'srv-del': async (el) => {
            const s = byId(el.dataset.id);
            if (!s || !(await ask(`从启动器里移除「${s.name}」？服务器上的东西不动。`))) return;
            await invoke('server_remove', { id: s.id }).catch(fail);
            api.closeDrawer();
            await load();
        },
        'srv-copy': (el) => navigator.clipboard.writeText(el.dataset.v).then(() => toast('复制好了'), () => toast('复制不了')),
    };

    return {
        enter,
        load,
        actions,
        // 选密钥文件：读成文本放在面板状态里（不落盘）
        change(t) {
            if (t.id === 'saKey' && t.files?.[0]) {
                const f = t.files[0];
                if (f.size > 64 * 1024) {
                    toast('这个文件太大了，不像密钥');
                    return true;
                }
                f.text().then((txt) => {
                    keepAdd();
                    S.drawer.keyText = txt;
                    S.drawer.keyName = f.name;
                    api.paintDrawer();
                });
                return true;
            }
            return false;
        },
        keydown(e) {
            if (e.key === 'Enter' && e.target.id === 'srvCmd') {
                e.preventDefault();
                actions['srv-send']();
                return true;
            }
            return false;
        },
    };
}
