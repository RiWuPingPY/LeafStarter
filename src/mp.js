// 联机的模拟：开房间、朋友进来、樱花、云服务器开服剧本、导入、服务器控制台
import { $, esc, toast } from './ui.js';
import { icon } from './icons.js';
import { burst } from './fx.js';
import { mpBody, roomCard, friendRows, liveText, serverMods, clientMods, importOk, hostSub, importSub } from './views/multiplayer.js';

const CHARS = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
const randCode = () => 'LS-' + [0, 1].map(() => Array.from({ length: 4 }, () => CHARS[Math.floor(Math.random() * CHARS.length)]).join('')).join('-');

// 示意用的二维码图案（不是真的二维码）
function qrData(seed) {
    try {
        const n = 25;
        const cell = 6;
        const pad = 12;
        const size = n * cell + pad * 2;
        const cv = document.createElement('canvas');
        cv.width = cv.height = size;
        const g = cv.getContext('2d');
        g.fillStyle = '#FFFFFF';
        g.fillRect(0, 0, size, size);
        g.fillStyle = '#1C1915';
        let s = [...seed].reduce((a, ch) => (Math.imul(a, 31) + ch.charCodeAt(0)) >>> 0, 7);
        const rnd = () => (s = (Math.imul(s, 1103515245) + 12345) >>> 0) / 4294967296;
        const eyeZone = (x, y) => (x < 8 && y < 8) || (x >= n - 8 && y < 8) || (x < 8 && y >= n - 8);
        for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) if (!eyeZone(x, y) && rnd() < 0.5) g.fillRect(pad + x * cell, pad + y * cell, cell, cell);
        const eye = (x, y) => {
            g.fillRect(pad + x * cell, pad + y * cell, 7 * cell, 7 * cell);
            g.fillStyle = '#FFFFFF';
            g.fillRect(pad + (x + 1) * cell, pad + (y + 1) * cell, 5 * cell, 5 * cell);
            g.fillStyle = '#1C1915';
            g.fillRect(pad + (x + 2) * cell, pad + (y + 2) * cell, 3 * cell, 3 * cell);
        };
        eye(0, 0);
        eye(n - 7, 0);
        eye(0, n - 7);
        return cv.toDataURL();
    } catch {
        return '';
    }
}

export function createMp(api) {
    const { S } = api;
    const timers = new Map();
    const later = (k, ms, fn) => {
        const list = timers.get(k) || [];
        list.push(setTimeout(fn, ms));
        timers.set(k, list);
    };
    const clear = (k) => {
        (timers.get(k) || []).forEach(clearTimeout);
        timers.delete(k);
    };
    const here = (i) => !!i && S.page === 'home' && S.inst === i.id && S.tab === 'servers';
    const paint = (i) => {
        if (here(i)) api.paintBody();
        api.paintTabs();
    };

    function copy(text) {
        const fail = () => toast(`浏览器不让复制，手动抄一下：${text}`);
        if (!navigator.clipboard?.writeText) return fail();
        navigator.clipboard.writeText(text).then(() => toast(`复制好了：${text}`), fail);
    }

    // ---------- 现在一起玩 ----------
    function start(i) {
        if (!i || api.RUN.get(i.id) || i.installing) return;
        i.room = { phase: 'wait', friends: [] };
        api.launch(i.id);
        paint(i);
    }

    // 游戏进到世界了：要是在等开房间，2 秒后模拟「对局域网开放」
    function onRunning(i) {
        if (i.room?.phase === 'wait') later(`${i.id}:room`, 2200, () => lan(i));
    }

    function lan(i) {
        if (!i || api.RUN.get(i.id)?.phase !== 'running') return;
        const port = 51234;
        i.room = { phase: 'lan', port, friends: [] };
        api.logTo(i.id, `[Server thread/INFO]: Started serving on ${port}`);
        api.logTo(i.id, `[Render thread/INFO]: [System] [CHAT] 本地游戏已在端口 ${port} 上开启`);
        ask(i, port);
        paint(i);
    }

    // 角落里问一句：游戏里刚开了局域网，要不要邀请朋友
    let askEl = null;
    function ask(i, port) {
        closeAsk();
        const el = document.createElement('div');
        el.className = 'mini ask';
        el.setAttribute('role', 'status');
        el.innerHTML = `<span class="ico violet">${icon('users', 18)}</span>
            <span class="tx"><b>「${esc(i.name)}」开了局域网</b><small>端口 ${port}。要邀请朋友一起玩吗？朋友会自动装好${i.loader ? '同样的 Mod' : '同样的版本'}</small></span>
            <span class="ask-act"><button type="button" class="btn sm pri" data-act="mp-ask-yes" data-id="${i.id}">邀请朋友</button><button type="button" class="btn sm ghost" data-act="mp-ask-no">不用</button></span>`;
        $('#app').append(el);
        askEl = el;
    }
    function closeAsk() {
        if (!askEl) return false;
        askEl.remove();
        askEl = null;
        return true;
    }

    function paintRoom(i) {
        const el = $('#roomCard');
        if (el && here(i) && S.mpMode !== 'host') el.outerHTML = roomCard(api.ctx());
    }

    function openRoom(i) {
        if (!i) return;
        closeAsk();
        clear(`${i.id}:room`);
        const port = i.room?.port || 51234;
        i.room = {
            phase: 'route', port, friends: [],
            steps: [
                { t: `读到局域网端口 ${port}`, sub: '从游戏日志里读的，不用你抄' },
                { t: '检查 IPv6', sub: '你有公网 IPv6，朋友也有的话可以直连' },
                { t: '探测网络类型', sub: '锥形 NAT，能打洞直连' },
                { t: '连上 EasyTier 节点', sub: '社区公共节点 · 只转发 MC 这一个端口' },
            ],
        };
        S.mpMode = 'now';
        api.save();
        if (S.inst !== i.id) api.selectGo(i.id);
        if (here(i)) api.paintBody();
        else api.setTab('servers');
        const st = i.room.steps;
        st.forEach((s, k) => {
            later(`${i.id}:room`, 150 + k * 480, () => { s.run = true; paintRoom(i); });
            later(`${i.id}:room`, 150 + k * 480 + 360, () => { s.run = false; s.done = true; paintRoom(i); });
        });
        later(`${i.id}:room`, 150 + st.length * 480 + 250, () => {
            Object.assign(i.room, { phase: 'open', code: randCode(), route: '直连', ping: 24, fresh: true });
            paint(i);
            i.room.fresh = false;
            toast('房间开好了，把邀请码发给朋友');
            const got = i.loader ? `自动装好了 ${i.mods.filter((m) => m.on).length} 个 Mod` : '版本自动对上了';
            later(`${i.id}:room`, 3600, () => join(i, { id: 'f1', name: '小明', face: 'xiaoming', via: `LeafStarter · 点邀请码进来，${got}`, route: '直连', ping: 24 }));
            later(`${i.id}:room`, 8800, () => join(i, { id: 'f2', name: '阿哲', face: 'azhe', via: '手机 · Zalith Launcher 2 · 扫码装好实例，走 IPv6 地址', route: 'IPv6', ping: 41 }));
        });
    }

    function join(i, f) {
        if (i.room?.phase !== 'open') return;
        i.room.friends.push(f);
        api.logTo(i.id, `[Server thread/INFO]: ${f.name} joined the game`);
        toast(`${f.name} 进来了`);
        api.paintTabs();
        if (!here(i) || S.mpMode === 'host') return;
        const box = $('#friends');
        if (box) {
            box.innerHTML = friendRows(i.room);
            box.querySelector(`[data-row="${f.id}"]`)?.classList.add('arrive');
        }
        const live = $('#roomLive');
        if (live) live.innerHTML = `<i class="dot run"></i>${esc(liveText(i.room))}`;
    }

    function closeRoom(i, why) {
        if (!i?.room) return;
        clear(`${i.id}:room`);
        closeAsk();
        const had = i.room.phase === 'open';
        i.room = null;
        paint(i);
        if (had || why) toast(why || '房间关了，邀请码失效');
    }

    function qr(anchor) {
        const room = api.cur()?.room;
        if (!room?.code) return;
        api.openPop(anchor, `<div class="pop-h">手机扫码加入</div>
            <div class="qrbox"><img class="qr" src="${qrData(room.code)}" width="168" height="168" alt="邀请码二维码（示意图案）"></div>
            <p class="pop-note">用 LeafStarter 安卓端扫：把同样的实例写进 Zalith Launcher 2 / FCL 的目录，再把地址加进它的服务器列表。手机走 P2P 要等安卓端支持，现在给的是 IPv6 地址。</p>`, 'below');
    }

    function repaintAny() {
        if (S.page === 'settings') api.paintView();
        else if (here(api.cur())) api.paintBody();
    }

    function sakura() {
        if (S.sakura) return;
        S.sakura = 'busy';
        repaintAny();
        setTimeout(() => {
            S.sakura = true;
            api.save();
            toast('连上了你的樱花账号（原型里是假的）。开房间时会自动建隧道');
            repaintAny();
        }, 900);
    }

    function modeSwitch() {
        const m = $('.modes');
        if (!m) return api.paintBody();
        m.dataset.on = S.mpMode;
        m.querySelectorAll('.mode').forEach((b) => {
            const on = b.dataset.v === S.mpMode;
            b.classList.toggle('on', on);
            b.setAttribute('aria-selected', on);
        });
        const body = $('#mpBody');
        body.innerHTML = mpBody(api.ctx());
        body.classList.remove('swap');
        void body.offsetWidth;
        body.classList.add('swap');
    }

    // ---------- 常驻服务器：开服 ----------
    function wizard() {
        api.openDrawer({ kind: 'host', ip: '', cloud: '阿里云', login: 'pw', lock: true, fw: 'ak', world: true, wl: true, bk: true, pem: '' });
    }

    function importer(via) {
        api.openDrawer({ kind: 'import', via, addr: '', ip: '', login: 'pw', panel: 'mcsm', url: '', key: '' });
    }

    function drawerInput(t) {
        const d = S.drawer;
        if (!d) return;
        d[t.dataset.hf] = t.type === 'checkbox' ? t.checked : t.value;
        const go = $('#drCreate');
        const sub = $('#drSub');
        if (d.kind === 'host') {
            if (go) go.disabled = !d.ip.trim();
            if (sub) sub.innerHTML = hostSub(d);
        } else if (d.kind === 'import') {
            if (go) go.disabled = !importOk(d);
            if (sub) sub.innerHTML = importSub(d);
        }
    }

    function drawerPrimary() {
        const d = S.drawer;
        if (d?.kind === 'host' && d.ip.trim()) startSetup(d);
        else if (d?.kind === 'import' && importOk(d)) doImport(d);
    }

    function startSetup(d) {
        const i = api.cur();
        const ip = d.ip.trim();
        const sm = serverMods(i);
        const cm = clientMods(i);
        const w = [...i.worlds].sort((a, b) => b.last - a.last)[0];
        const cloudApi = d.cloud === '腾讯云' ? '腾讯云 Lighthouse' : '阿里云 SWAS';
        const viaApi = d.fw === 'ak' && d.cloud !== '其他';
        const loaderCmd = i.loader === 'Fabric' ? `java -jar fabric-installer.jar server -mcversion ${i.mc} -loader ${i.loaderVer} -downloadMinecraft`
            : i.loader === 'Forge' ? `java -jar forge-${i.mc}-${i.loaderVer}-installer.jar --installServer`
                : i.loader === 'NeoForge' ? `java -jar neoforge-${i.loaderVer}-installer.jar --installServer`
                    : `下载官方 server.jar ${i.mc}，校验 SHA1`;
        const steps = [
            { t: d.login === 'pw' ? '用 root 密码登录' : '用密钥登录', sub: d.login === 'pw' ? '密码只用这一次' : '', cmd: [`ssh root@${ip}`], out: ['Welcome to Ubuntu 22.04.4 LTS (GNU/Linux 5.15.0-105-generic x86_64)'] },
            d.login === 'pw' && {
                t: '装上密钥，改用密钥登录', sub: d.lock ? '确认密钥能登录后，关掉了密码登录' : '密钥存进了本机的加密保管库',
                cmd: ['mkdir -p ~/.ssh && echo "ssh-ed25519 AAAAC3Nz…leafstarter" >> ~/.ssh/authorized_keys', ...(d.lock ? ["sed -i 's/^#\\?PasswordAuthentication .*/PasswordAuthentication no/' /etc/ssh/sshd_config && systemctl reload ssh"] : [])],
                out: ['用密钥重新登录：成功'],
            },
            {
                t: '开放端口 25565', sub: viaApi ? `调 ${cloudApi} 的 CreateFirewallRules，加了一条 TCP 25565` : '检测到你在控制台放行了',
                cmd: [viaApi ? 'CreateFirewallRules  Protocol=TCP  Port=25565    # 调云接口，不在服务器上执行' : '# 每 5 秒试一次能不能从外面连上 25565，等你在控制台加好规则'], out: ['端口已放行'],
            },
            { t: '识别系统', sub: 'Ubuntu 22.04 · 2 核 · 3.8 GB 内存 · 40 GB 硬盘', cmd: ['cat /etc/os-release; nproc; free -h; df -h /'], out: ['Ubuntu 22.04.4 LTS', '2', 'Mem: 3.8Gi', '/dev/vda3  40G'] },
            {
                t: `安装 Java ${i.javaNeed}`, sub: '', cmd: [`apt-get install -y openjdk-${i.javaNeed}-jre-headless`], out: [`openjdk version "${i.javaNeed}.0.13"`], ms: 1100,
                failOnce: true, err: '软件源连不上', why: '：这台机器默认的软件源连续超时了 3 次。机器本身是好的，前面几步都正常。',
                failOut: [`Err:1 http://mirrors.cloud.example/ubuntu jammy-updates/main amd64 openjdk-${i.javaNeed}-jre-headless`, '  Could not connect to mirrors.cloud.example:80, connection timed out', 'E: Unable to fetch some archives, maybe run apt-get update or try with --fix-missing?'],
            },
            { t: `安装${i.loader ? ` ${i.loader} ` : '原版'}服务端`, sub: `${i.mc}${i.loader ? ` · ${i.loaderVer}` : ''}`, cmd: [loaderCmd], out: ['Done'], ms: 900 },
            i.loader && {
                t: `搬上 ${sm.length} 个 Mod 和配置`, sub: cm.length ? `剔除了 ${cm.length} 个只在客户端用的：${cm.slice(0, 3).map((m) => m.name).join('、')}${cm.length > 3 ? ' 等' : ''}` : '',
                cmd: ['rsync -a mods/ config/ root@服务器:/opt/minecraft/'], out: [...sm.slice(0, 4).map((m) => `  ${m.name} ${m.ver}`), ...(sm.length > 4 ? [`  …共 ${sm.length} 个`] : [])],
            },
            d.world && w && { t: `上传世界「${w.name}」`, sub: w.size, cmd: [`rsync -a "saves/${w.name}/" root@服务器:/opt/minecraft/world/`], out: ['传完了，校验一致'], ms: 1500 },
            { t: '写配置', sub: `server.properties${d.wl ? '、白名单（3 人）' : ''}、RCON（只听本机）`, cmd: [`online-mode=true  white-list=${d.wl}  enable-rcon=true`, ...(d.wl ? ['whitelist: PingYi 小明 阿哲'] : [])], out: [] },
            { t: '注册成系统服务', sub: '开机自启，崩了 10 秒后自动重启', cmd: ['systemctl enable --now minecraft.service'], out: ['Created symlink /etc/systemd/system/multi-user.target.wants/minecraft.service'] },
            d.bk && { t: '设好每天 04:00 备份', sub: '留最近 7 份', cmd: ['systemctl enable --now minecraft-backup.timer'], out: [] },
            { t: '启动服务器，等它就绪', sub: '', cmd: ['journalctl -u minecraft -f'], out: ['[Server thread/INFO]: Done (6.214s)! For help, type "help"'], ms: 1300 },
        ].filter(Boolean).map((s) => ({ ...s, state: 'todo' }));
        api.closeDrawer();
        i.srv = {
            phase: 'setup', ip, cloud: d.cloud, steps, k: 0, open: null, fixed: false, manual: false,
            opts: { login: d.login, lock: d.lock, bk: d.bk },
            task: api.addTask(`开服 ${i.name}`, `${d.cloud} · ${ip}`),
        };
        S.mpMode = 'host';
        api.save();
        if (here(i)) api.paintBody();
        else api.setTab('servers');
        step(i);
    }

    function progress(i) {
        const s = i.srv;
        const done = s.steps.filter((x) => x.state === 'done').length;
        s.task.pct = Math.round((done / s.steps.length) * 100);
        const x = s.steps[s.k];
        s.task.stage = x?.state === 'fail' ? `卡在「${x.t}」：${x.err}` : x?.t || '';
        api.paintTasks();
        if (here(i) && S.mpMode === 'host') api.paintBody();
    }

    function step(i) {
        const s = i.srv;
        if (!s || s.phase !== 'setup') return;
        const x = s.steps[s.k];
        if (!x) return finish(i);
        x.state = 'run';
        progress(i);
        const ms = x.ms || 650;
        later(`${i.id}:srv`, ms, () => {
            if (x.failOnce && !s.fixed) {
                x.state = 'fail';
                x.out = x.failOut;
                s.open = s.k;
                progress(i);
                if (!here(i)) toast(`「${i.name}」开服卡在「${x.t}」了，去「联机」看看`);
                return;
            }
            x.state = 'done';
            x.dur = `${(ms / 1000).toFixed(1)} 秒`;
            if (s.open === s.k) s.open = null;
            s.k += 1;
            step(i);
        });
    }

    // 重试：没换软件源就还是连不上；看过「怎么手动处理」就当已经换好了
    function retry(i) {
        const s = i?.srv;
        const x = s?.steps[s.k];
        if (!x || x.state !== 'fail') return;
        if (!s.manual) {
            x.state = 'run';
            progress(i);
            later(`${i.id}:srv`, 900, () => {
                x.state = 'fail';
                progress(i);
                toast('还是连不上。看看「怎么手动处理」，或者交给 AI');
            });
            return;
        }
        s.fixed = true;
        x.out = [];
        step(i);
    }

    // AI 换好软件源、装好了 Java：这一步算完成，剧本接着往下走
    function resumeSetup(i) {
        const s = i?.srv;
        const x = s?.steps[s.k];
        if (!x || x.state !== 'fail') return;
        s.fixed = true;
        x.state = 'done';
        x.sub = 'AI 换成清华镜像源后装好的';
        x.out = [`Get:1 https://mirrors.tuna.tsinghua.edu.cn/ubuntu jammy-updates/main amd64 openjdk-${i.javaNeed}-jre-headless`, `openjdk version "${i.javaNeed}.0.13"`];
        x.dur = 'AI';
        s.open = null;
        s.k += 1;
        step(i);
    }

    function finish(i) {
        const s = i.srv;
        Object.assign(s.task, { state: 'done', pct: 100, stage: '', at: Date.now() });
        Object.assign(s, {
            phase: 'up', title: `${i.name} 的服务器`,
            machine: `${s.cloud === '其他' ? '云服务器' : `${s.cloud}轻量应用服务器`} · 2 核 4G · Ubuntu 22.04`,
            soft: `${i.loader ? `${i.loader} ${i.loaderVer}` : '原版'} · ${i.mc}${i.loader ? ` · ${serverMods(i).length} 个 Mod` : ''}`,
            login: s.opts.login === 'pw' && s.opts.lock ? '密钥登录（在保管库里），密码登录已关闭' : '密钥登录（在保管库里）',
            backup: s.opts.bk, online: 0, showConsole: false,
            console: [`[Server thread/INFO]: Starting minecraft server version ${i.mc}`, '[Server thread/INFO]: Preparing level "world"', '[Server thread/INFO]: Done (6.214s)! For help, type "help"'],
        });
        i.servers.unshift({ id: 'v' + Date.now(), name: s.title, addr: s.ip, motd: `由 LeafStarter 管理 · ${i.mc}`, players: '0/20', ping: 32 });
        api.paintTop();
        api.paintTasks();
        api.paintTabs();
        if (here(i)) {
            api.paintBody();
            const ic = $('#srvCard .ico');
            if (ic) burst(ic, $('#app'));
        }
        toast(`服务器开好了：${s.ip}`);
    }

    // ---------- 常驻服务器：导入 ----------
    function doImport(d) {
        const i = api.cur();
        api.closeDrawer();
        S.mpMode = 'host';
        api.save();
        const row = (name, addr, motd, players) => ({ id: 'v' + Date.now(), name, addr, motd, players, ping: 46 });
        const ver = `${i.loader || 'Paper'} ${i.mc}`;
        if (d.via === 'addr') {
            const addr = d.addr.trim();
            i.servers.push(row(addr.split(':')[0], addr, '刚导入的服务器', '?/?'));
            toast(`加进了服务器列表：${addr}`);
        } else if (d.via === 'ssh') {
            const ip = d.ip.trim();
            i.srv = {
                phase: 'up', ip, title: `${ip} 上的服务器`, machine: '识别到：Ubuntu 22.04 · 4 核 8G',
                soft: `已有服务端：${ver}（/opt/minecraft），配置没动`, login: '密钥登录（在保管库里）',
                backup: false, online: 2, showConsole: false,
                console: ['[Server thread/INFO]: 小红 joined the game', '[Server thread/INFO]: 小蓝 joined the game'],
            };
            i.servers.unshift(row(i.srv.title, ip, '已有服务端 · 由 LeafStarter 接管', '2/20'));
            toast(`接管了 ${ip} 上的服务端，原来的配置没动`);
        } else {
            const url = d.url.trim();
            const host = url.replace(/^https?:\/\//, '').split(/[/:]/)[0] || url;
            const name = d.panel === 'mcsm' ? 'MCSManager' : '翼龙面板';
            i.srv = {
                phase: 'up', ip: host, title: `${name}里的「生存服」`, machine: `${name} · ${url}`,
                soft: `${ver}（从面板里读到的）`, login: 'API Key（在保管库里）',
                backup: true, online: 0, showConsole: false,
                console: ['[面板] 已连接，实例状态：运行中'],
            };
            i.servers.unshift(row(i.srv.title, host, `${name} · 由 LeafStarter 接管`, '0/20'));
            toast(`连上了${name}，导入了实例「生存服」`);
        }
        if (here(i)) api.paintBody();
        else api.setTab('servers');
    }

    // ---------- 服务器控制台 ----------
    function consoleCmd(input) {
        const s = api.cur()?.srv;
        if (!s) return;
        const cmd = input.value.trim().replace(/^\//, '');
        if (!cmd) return;
        input.value = '';
        const [w, ...rest] = cmd.split(/\s+/);
        const arg = rest.join(' ');
        const out = w === 'list' ? `There are ${s.online} of a max of 20 players online`
            : w === 'say' ? `[Server] ${arg}`
                : w === 'whitelist' && rest[0] === 'add' ? `Added ${rest[1] || '?'} to the whitelist`
                    : w === 'tps' ? 'TPS：20.0（1 分钟）· 20.0（5 分钟）· 20.0（15 分钟）'
                        : w === 'stop' ? '原型里不真的停服；正式版会先问你，再存档、停服'
                            : w === 'help' ? '原型里能试：list、say、whitelist add、tps、stop'
                                : 'Unknown or incomplete command, see below for error';
        s.console.push(`> ${cmd}`, out);
        const log = $('#srvLog');
        if (log) {
            log.textContent = s.console.join('\n');
            log.scrollTop = log.scrollHeight;
        }
    }

    const srv = () => api.cur()?.srv;
    const actions = {
        'mp-mode': (el) => {
            S.mpMode = el.dataset.v;
            api.save();
            modeSwitch();
        },
        'mp-start': () => start(api.cur()),
        'mp-lan': () => lan(api.cur()),
        'mp-open': () => openRoom(api.cur()),
        'mp-ask-yes': (el) => openRoom(api.byId(el.dataset.id)),
        'mp-ask-no': () => {
            closeAsk();
            toast('好。想开房间时，去这个实例的「联机」里点「开房间」');
        },
        'mp-close': () => closeRoom(api.cur()),
        'mp-copy': (el) => copy(el.dataset.v),
        'mp-qr': (el) => (api.popAnchor() === el ? api.closePop() : qr(el)),
        'mp-sakura': () => sakura(),
        'mp-sakura-off': () => {
            S.sakura = false;
            api.save();
            repaintAny();
        },
        'host-wizard': () => wizard(),
        'host-buy': (el) => toast(`正式版会打开${el.dataset.v}轻量应用服务器的购买页，由你自己下单付款`),
        'host-import': (el) => importer(el.dataset.v),
        'host-retry': () => retry(api.cur()),
        'host-manual': () => {
            srv().manual = !srv().manual;
            api.paintBody();
        },
        'host-ai': () => api.agent.open('server', { id: S.inst }),
        'step-tog': (el) => {
            const k = Number(el.dataset.k);
            srv().open = srv().open === k ? null : k;
            api.paintBody();
        },
        'hf-seg': (el) => {
            S.drawer[el.dataset.k] = el.dataset.v;
            api.paintDrawer();
        },
        'hf-pem': () => {
            S.drawer.pem = 'leafstarter-hz.pem';
            api.paintDrawer();
        },
        'srv-invite': () => {
            srv().invite = randCode();
            copy(srv().invite);
            api.paintBody();
        },
        'srv-console': () => {
            srv().showConsole = !srv().showConsole;
            api.paintBody();
            const log = $('#srvLog');
            if (log) log.scrollTop = log.scrollHeight;
            $('#srvCmd')?.focus({ preventScroll: true });
        },
        'srv-backup': () => {
            srv().console.push('[备份] world-2026-11-22-2140.tar.zst · 418 MB · 已校验');
            toast('备份好了，留在服务器的 backups 里（示例）');
            if (srv().showConsole) api.paintBody();
        },
        'srv-restart': () => {
            const s = srv();
            toast('正在重启服务器…');
            s.console.push('[Server thread/INFO]: Stopping server', '[Server thread/INFO]: Saving worlds');
            setTimeout(() => {
                s.console.push('[Server thread/INFO]: Done (5.873s)! For help, type "help"');
                toast('服务器重启好了');
                if (s.showConsole && api.cur()?.srv === s) api.paintBody();
            }, 1500);
        },
        'srv-pull': () => toast('正式版会把服务器上的世界下载成一个新存档「…（服务器）」，不会覆盖本地的'),
    };

    return {
        actions,
        onRunning,
        onStop: (i) => i.room && closeRoom(i, i.room.phase === 'open' ? '游戏结束了，房间跟着关了，邀请码失效' : ''),
        closeAsk,
        drawerInput,
        drawerPrimary,
        consoleCmd,
        resumeSetup,
        failedSetup: () => api.INSTANCES.find((i) => i.srv?.phase === 'setup' && i.srv.steps.some((x) => x.state === 'fail')),
    };
}
