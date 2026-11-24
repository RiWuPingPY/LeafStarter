// 联机标签：「现在一起玩」（临时联机）和「常驻服务器」，以及开服、导入两个侧边面板。只出 HTML，事件在 mp.js 里接
import { esc, head } from '../ui.js';
import { icon } from '../icons.js';

const ico = (tone, name) => `<span class="ico ${tone}">${icon(name, 16)}</span>`;
const chip = (cls, ic, text) => `<span class="chip ${cls}">${ic ? icon(ic, 13) : ''}${esc(text)}</span>`;
const segH = (k, list, cur) => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="hf-seg" data-k="${k}" data-v="${v}" class="${cur === v ? 'on' : ''}" aria-pressed="${cur === v}">${t}</button>`).join('')}</div>`;

export const onMods = (i) => i.mods.filter((m) => m.on);
export const serverMods = (i) => onMods(i).filter((m) => !m.client);
export const clientMods = (i) => onMods(i).filter((m) => m.client);
const newest = (i) => [...i.worlds].sort((a, b) => b.last - a.last)[0];
const modsText = (i) => (i.loader ? `同样的 ${onMods(i).length} 个 Mod` : '同样的版本');

export function mpView(c) {
    const mode = c.S.mpMode === 'host' ? 'host' : 'now';
    return `<div class="modes" data-on="${mode}" role="tablist" aria-label="联机方式">
        <i class="mode-ind" aria-hidden="true"></i>
        ${modeBtn(mode, 'now', 'users', '现在一起玩', '你开着游戏，朋友进来 · 免费')}
        ${modeBtn(mode, 'host', 'server', '常驻服务器', '朋友随时能上，不用你开着电脑')}
    </div>
    <div class="mp-body" id="mpBody">${mpBody(c)}</div>`;
}

const modeBtn = (cur, v, ic, t, s) => `<button type="button" role="tab" class="mode${cur === v ? ' on' : ''}" data-act="mp-mode" data-v="${v}" aria-selected="${cur === v}">
    <span class="mode-ic">${icon(ic, 20)}</span><span class="tx"><span class="nm">${t}</span><span class="sb">${s}</span></span></button>`;

export function mpBody(c) {
    return c.S.mpMode === 'host' ? hostView(c) : roomCard(c) + altCard(c);
}

// ---------- 现在一起玩 ----------
export function roomCard(c) {
    const i = c.inst;
    const room = i.room;
    const r = c.RUN.get(i.id);
    if (room?.phase === 'open') return openRoom(i, room);
    if (room?.phase === 'route') {
        return `<section class="card room" id="roomCard">
            <h3>${ico('violet', 'users')}正在开房间${chip('acc', 'zap', '自动选路')}</h3>
            <ol class="route">${room.steps.map((s) => `<li class="${s.done ? 'done' : s.run ? 'run' : ''}"><span class="sic">${s.done ? icon('check', 13) : ''}</span><span class="tx"><span class="nm">${esc(s.t)}</span><span class="sb">${s.done ? esc(s.sub) : '&nbsp;'}</span></span></li>`).join('')}</ol>
        </section>`;
    }
    if (r?.phase === 'running') {
        const body = room?.phase === 'lan'
            ? `<p>检测到你开了局域网（端口 <b class="mono">${room.port}</b>）。开个房间，朋友就能进来。</p>
               <div class="actions"><button type="button" class="btn pri" data-act="mp-open">${icon('users')}开房间</button></div>`
            : room?.phase === 'wait'
                ? `<p class="waitline"><i class="dot busy"></i>等你在游戏的暂停菜单里点「对局域网开放」…… 启动器会从游戏日志里读到端口。</p><p class="note">原型里进游戏 2 秒后会自动模拟这一步。</p>`
                : `<p>在游戏的暂停菜单里点「对局域网开放」，启动器会在角落问你要不要邀请朋友。</p>
                   <div class="actions"><button type="button" class="btn pri" data-act="mp-lan">${icon('users')}我开好了局域网</button></div>`;
        return `<section class="card room" id="roomCard"><h3>${ico('violet', 'users')}邀请朋友一起玩${chip('ok', 'play', '游戏在运行')}</h3>${body}</section>`;
    }
    const busy = !!r || i.installing;
    return `<section class="card room" id="roomCard">
        <h3>${ico('violet', 'users')}邀请朋友一起玩${chip('', '', '没开房间')}</h3>
        <ol class="how">
            <li><b>进游戏</b><span>启动「${esc(i.name)}」，进一个世界</span></li>
            <li><b>对局域网开放</b><span>暂停菜单里点一下；启动器从日志里读到端口，会在角落问你</span></li>
            <li><b>发邀请码</b><span>朋友点开，自动装好${modsText(i)}，直接进你的世界</span></li>
        </ol>
        <div class="actions"><button type="button" class="btn pri" data-act="mp-start" id="mpStart"${busy ? ' disabled' : ''}>${icon('play')}启动游戏并开房间</button><span class="note">原型里进游戏 2 秒后会模拟「对局域网开放」</span></div>
    </section>`;
}

function openRoom(i, room) {
    // 邀请码第一次出现时一个字一个字落下来；之后重画不再播
    const code = [...room.code].map((ch, k) => `<span style="--k:${k}">${ch === '-' ? '<i>-</i>' : esc(ch)}</span>`).join('');
    const ver = `${i.mc} · ${i.loader ? `${i.loader} ${i.loaderVer} · ${onMods(i).length} 个 Mod` : '原版'}`;
    return `<section class="card room open" id="roomCard">
        <h3>${ico('violet', 'users')}房间开着<span class="chip ok" id="roomLive"><i class="dot run"></i>${liveText(room)}</span>
            <span class="push chip acc" title="能直连就直连，打不通走中继">${icon('zap', 13)}P2P · ${esc(room.route)} · ${room.ping} ms</span></h3>
        <div class="invite">
            <div class="code mono${room.fresh ? ' fresh' : ''}" aria-label="邀请码 ${esc(room.code)}">${code}</div>
            <div class="actions">
                <button type="button" class="btn pri" data-act="mp-copy" data-v="${esc(room.code)}">${icon('copy')}复制邀请码</button>
                <button type="button" class="btn" data-act="mp-copy" data-v="leafstarter://join/${esc(room.code)}">${icon('link')}复制链接</button>
                <button type="button" class="btn" data-act="mp-qr">${icon('qr')}手机扫码</button>
            </div>
        </div>
        <p class="carry">${icon('package', 15)}<span>邀请码里带着实例清单：<b>${esc(ver)}</b>。朋友用 LeafStarter 打开，会先装好一模一样的实例再进来，不会因为 Mod 对不上进不去。</span></p>
        <div class="friends" id="friends">${friendRows(room)}</div>
        <div class="actions"><span class="note">局域网端口 ${room.port} · 邀请码 2 小时后失效，关房间立刻失效</span><span class="sp"></span><button type="button" class="btn ghost danger" data-act="mp-close">${icon('x')}关房间</button></div>
    </section>`;
}

export const liveText = (room) => (room.friends.length ? `${room.friends.length} 个朋友在玩` : '等朋友加入');

export function friendRows(room) {
    if (!room.friends.length) return '<div class="friend empty"><i class="dot busy"></i>把邀请码发给朋友。原型里过几秒会有朋友进来。</div>';
    return room.friends.map((f) => `<div class="friend" data-row="${f.id}">${head(f.face, 30)}
        <span class="tx"><span class="nm">${esc(f.name)}</span><span class="sb">${esc(f.via)}</span></span>
        <span class="chip ${f.ping < 60 ? 'ok' : 'warn'}">${esc(f.route)} · ${f.ping} ms</span></div>`).join('');
}

function altCard(c) {
    const i = c.inst;
    const room = i.room?.phase === 'open' ? i.room : null;
    const sak = c.S.sakura;
    const later = '<span class="note nowrap">开了房间才有地址</span>';
    const row = (tone, ic, name, tag, addr, note, extra) => `<div class="addr${addr ? '' : ' dim'}">
        <span class="ico ${tone}">${icon(ic, 16)}</span>
        <span class="tx"><span class="nm">${name}${tag}</span><span class="sb">${note}</span></span>
        ${addr ? `<span class="av mono">${esc(addr)}</span><button type="button" class="iconbtn sm" data-act="mp-copy" data-v="${esc(addr)}" aria-label="复制地址" title="复制">${icon('copy', 14)}</button>` : `<span class="av">${extra}</span>`}
    </div>`;
    const sakExtra = sak === true ? later : sak === 'busy' ? '<button type="button" class="btn sm" disabled>正在连…</button>' : '<button type="button" class="btn sm" data-act="mp-sakura">连接樱花账号</button>';
    return `<section class="card alt">
        <h3>${ico('sky', 'globe')}朋友没装 LeafStarter？</h3>
        <p>给他一个地址就行，什么都不用装，手机启动器也能进。只是 Mod 得他自己装对，可以把下面的整合包一起发过去。</p>
        <div class="addrs">
            ${row('ok', 'zap', 'IPv6 直连', chip('ok', '', '最快 · 零成本'), room ? `[2408:8256:3a81:1c0::4c1a]:${room.port}` : '', '不经过任何中转。对方也要有公网 IPv6；你这边检测到了', later)}
            ${row('amber', 'cloud', '樱花穿透', chip('', '', '免费 · 要实名'), room && sak === true ? 'frp-hz.example.net:35612' : '', sak === true ? '免费隧道：有流量时 10 Mbps，用完限 4 Mbps，24 小时后恢复（以樱花官网为准）' : '用你自己的樱花账号自动建隧道，几个人玩够用', sakExtra)}
            ${row('violet', 'link', 'Radmin VPN', chip('', '', '已安装'), room ? `26.18.40.7:${room.port}` : '', '检测到你装了 Radmin VPN 并加入了网络。LeafStarter 不打包、不替你装它', later)}
        </div>
        <div class="actions"><button type="button" class="btn" data-act="export">${icon('package')}导出整合包 .mrpack</button><span class="note">别的启动器导入它，就是同样的 Mod</span></div>
    </section>`;
}

// ---------- 常驻服务器 ----------
function hostView(c) {
    const i = c.inst;
    const top = i.srv ? (i.srv.phase === 'setup' ? setupCard(c, i) : srvCard(i)) : `<div class="grid2">${cloudCard(i)}${importCard()}</div>`;
    return `${top}
    <div class="toolbar"><h2 class="sh">服务器列表</h2><span class="note">点「进入」会启动游戏并直接连上</span><span class="sp"></span><button type="button" class="btn" data-act="servers-ping">${icon('refresh')}刷新</button></div>
    <div class="rows" id="srvRows">${serverRows(c)}</div>`;
}

function cloudCard(i) {
    const n = onMods(i).length;
    const spec = !i.loader ? '2 核 2G' : n > 60 ? '4 核 8G' : '2 核 4G';
    return `<section class="card">
        <h3>${ico('violet', 'cloud')}云服务器一键开服</h3>
        <p>按这个实例（${i.loader ? `${n} 个 Mod` : '原版'}，按 2–8 人算）推荐 <b>${spec}</b>，地域选离朋友们近的。你自己下单付款；买好后填 IP 和密码，剩下的启动器按固定剧本做完。</p>
        <p class="note">参考：阿里云的教程建议 20 人以内的中型整合包用 2 核 4G。</p>
        <div class="actions">
            <button type="button" class="btn pri" data-act="host-wizard">${icon('server')}已经买好了</button>
            <button type="button" class="btn" data-act="host-buy" data-v="阿里云">去阿里云买</button>
            <button type="button" class="btn" data-act="host-buy" data-v="腾讯云">去腾讯云买</button>
        </div>
    </section>`;
}

function importCard() {
    return `<section class="card">
        <h3>${ico('sky', 'upload')}导入已有服务器</h3>
        <p>已经有服务器了？接进来就能在这里启停、看控制台、备份、同步 Mod。已有的配置不动，要改会先问你。</p>
        <div class="actions">
            <button type="button" class="btn" data-act="host-import" data-v="addr">只有地址</button>
            <button type="button" class="btn" data-act="host-import" data-v="ssh">${icon('terminal')}SSH 登录</button>
            <button type="button" class="btn" data-act="host-import" data-v="panel">MCSManager / 翼龙面板</button>
        </div>
    </section>`;
}

function setupCard(c, i) {
    const s = i.srv;
    const done = s.steps.filter((x) => x.state === 'done').length;
    const failed = s.steps.find((x) => x.state === 'fail');
    const pct = Math.round((done / s.steps.length) * 100);
    return `<section class="card setup${failed ? ' failed' : ''}" id="setupCard">
        <h3>${ico(failed ? 'amber' : 'violet', 'terminal')}开服剧本${chip(failed ? 'warn' : 'acc', failed ? 'alert' : 'zap', failed ? `第 ${s.steps.indexOf(failed) + 1} 步卡住了` : `${done} / ${s.steps.length}`)}<span class="push mono sb">${esc(s.ip)} · ${esc(s.cloud)}</span></h3>
        <span class="bar${failed ? '' : ' live'}"><i style="--p:${pct}%"></i></span>
        <ol class="steps">${s.steps.map((x, k) => stepRow(x, k, s.open === k)).join('')}</ol>
        ${failed ? `<div class="stuck">
            <p><b>${esc(failed.err)}</b>${esc(failed.why)}</p>
            <div class="actions">
                <button type="button" class="btn pri" data-act="host-ai">${icon('sparkles')}交给 AI 看看${c.aiReady() ? '' : '（要先填 Key）'}</button>
                <button type="button" class="btn" data-act="host-retry">${icon('refresh')}重试这一步</button>
                <button type="button" class="btn ghost" data-act="host-manual" aria-expanded="${!!s.manual}">怎么手动处理</button>
            </div>
            ${s.manual ? '<p class="note">在服务器上把 <span class="mono">/etc/apt/sources.list</span> 里的软件源换成国内镜像站（比如清华、阿里云的），再点「重试这一步」。每一步都能重复执行，已经装好的不会重装。</p>' : ''}
        </div>` : '<p class="note">每一步都能重复执行：中途断网、关掉启动器，回来接着做，不会重复装。点一步能看到执行的命令和输出。</p>'}
    </section>`;
}

function stepRow(x, k, open) {
    const sic = x.state === 'done' ? icon('check', 13) : x.state === 'fail' ? icon('x', 13) : x.state === 'run' ? '' : k + 1;
    const cmd = (x.cmd || []).map((l) => `<span class="p">$</span> ${esc(l)}`).join('\n');
    const out = (x.out || []).map((l) => `<span class="o">${esc(l)}</span>`).join('\n');
    return `<li class="step ${x.state}${open ? ' open' : ''}">
        <button type="button" class="step-h" data-act="step-tog" data-k="${k}" aria-expanded="${open}">
            <span class="sic">${sic}</span>
            <span class="tx"><span class="nm">${esc(x.t)}</span>${x.sub ? `<span class="sb">${esc(x.sub)}</span>` : ''}</span>
            ${x.dur ? `<span class="sb mono">${x.dur}</span>` : ''}
        </button>
        ${open ? `<pre class="cmdbox">${cmd}${cmd && out ? '\n' : ''}${out}</pre>` : ''}
    </li>`;
}

const stat = (k, v, p) => `<div class="stat"><span class="k">${k}</span><b class="mono">${esc(v)}</b>${p != null ? `<span class="bar"><i style="--p:${p}%"></i></span>` : ''}</div>`;

function srvCard(i) {
    const s = i.srv;
    return `<section class="card srv" id="srvCard">
        <h3>${ico('ok', 'server')}${esc(s.title)}${chip('ok', '', '运行中')}<span class="push mono sb">${esc(s.ip)}:25565</span></h3>
        <div class="stats">${stat('在线', `${s.online} / 20`)}${stat('TPS', '20.0')}${stat('内存', '1.9 / 4 GB', 47)}${stat('下次备份', s.backup ? '今天 04:00' : '没设')}</div>
        <dl class="facts">
            <dt>机器</dt><dd>${esc(s.machine)}</dd>
            <dt>服务端</dt><dd>${esc(s.soft)}</dd>
            <dt>登录</dt><dd>${esc(s.login)}</dd>
        </dl>
        <div class="actions">
            <button type="button" class="btn pri" data-act="server-play" data-v="${esc(s.ip)}">${icon('play')}进服</button>
            <button type="button" class="btn" data-act="srv-invite">${icon('users')}邀请朋友</button>
            <button type="button" class="btn${s.showConsole ? ' on' : ''}" data-act="srv-console" aria-pressed="${!!s.showConsole}">${icon('terminal')}控制台</button>
            <button type="button" class="btn" data-act="srv-backup">${icon('history')}立刻备份</button>
            <button type="button" class="btn" data-act="srv-restart">${icon('refresh')}重启</button>
            <button type="button" class="btn ghost" data-act="srv-pull">${icon('download')}把世界拉回本地</button>
        </div>
        ${s.invite ? `<p class="carry">${icon('link', 15)}<span>邀请码 <b class="mono">${esc(s.invite)}</b>（已复制）：带着服务器地址和实例清单，朋友点开会自动装好同样的实例，启动后直接进服。</span></p>` : ''}
        ${s.showConsole ? `<div class="console"><pre id="srvLog" tabindex="0">${s.console.map((l) => esc(l)).join('\n')}</pre><label class="cin" for="srvCmd"><span class="mono">&gt;</span><input id="srvCmd" type="text" placeholder="输入命令，比如 list、say 大家好、whitelist add 小红" autocomplete="off"></label></div>` : ''}
    </section>`;
}

export function serverRows(c) {
    const list = c.inst.servers;
    if (!list.length) return '<div class="row empty">还没有服务器。游戏「多人游戏」里加过的、在这里开好或导入的服务器，都会出现在这里。</div>';
    return list.map((s) => {
        const lvl = s.ping < 60 ? 'ok' : s.ping < 150 ? 'warn' : 'bad';
        return `<div class="row server" data-row="${s.id}">
            <span class="tile" style="--h:${(s.name.length * 53) % 360}">${icon('server', 16)}</span>
            <span class="tx"><span class="nm">${esc(s.name)} <span class="mono sb">${esc(s.addr)}</span></span><span class="sb">${esc(s.motd)}</span></span>
            <span class="sb mono">${esc(s.players)}</span>
            <span class="ping ${lvl}"><i></i><i></i><i></i><span class="mono">${s.ping} ms</span></span>
            <button type="button" class="btn sm" data-act="server-play" data-v="${esc(s.addr)}">${icon('play', 13)}进入</button>
        </div>`;
    }).join('');
}

// ---------- 云服务器开服（侧边面板） ----------
export function hostDrawer(c) {
    const d = c.S.drawer;
    const i = c.inst;
    const sm = serverMods(i);
    const cm = clientMods(i);
    const w = newest(i);
    const api = d.cloud === '腾讯云' ? '腾讯云 Lighthouse' : '阿里云 SWAS';
    const login = d.login === 'pw'
        ? `<label class="field" for="hfPw"><span>root 密码</span><input id="hfPw" type="password" data-hf="pw" placeholder="只用一次：登录后换成密钥，不保存密码" autocomplete="off"></label>
           <label class="check" for="hfLock"><input type="checkbox" id="hfLock" data-hf="lock"${d.lock ? ' checked' : ''}><span><b>换成密钥后关闭密码登录</b><small>先确认密钥能登录再关，免得把自己锁在外面</small></span></label>`
        : d.login === 'pem'
            ? `<div class="field"><span>密钥文件</span><button type="button" class="btn" data-act="hf-pem">${icon('key')}${d.pem ? esc(d.pem) : '选择 .pem 文件'}</button><small class="note">云控制台创建密钥对时下载的那个文件</small></div>`
            : '<p class="note">用本机 <span class="mono">~/.ssh</span> 里已经配好的密钥直接登录。</p>';
    const fw = d.fw === 'ak'
        ? `<label class="field" for="hfAk"><span>AccessKey ID</span><input id="hfAk" type="text" class="mono" data-hf="ak" placeholder="只给「防火墙」权限的子账号" autocomplete="off"></label>
           <label class="field" for="hfSk"><span>AccessKey Secret</span><input id="hfSk" type="password" data-hf="sk" autocomplete="off"></label>
           <p class="note">启动器调 ${api} 的 <span class="mono">CreateFirewallRules</span>，加一条 TCP 25565。用完可以在云控制台删掉这个 Key。</p>`
        : '<p class="note">控制台 › 这台服务器 › 防火墙 › 添加规则：TCP 25565。加好后剧本会自己检测到。</p>';
    return `<section class="drawer" role="dialog" aria-label="云服务器开服">
        <header class="dr-head"><h2>云服务器开服</h2><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
        <div class="dr-body">
            <div class="wstep"><span class="wn">1</span><div class="wb">
                <h4>连上服务器</h4>
                <label class="field" for="hfIp"><span>公网 IPv4</span><input id="hfIp" type="text" class="mono" data-hf="ip" value="${esc(d.ip)}" placeholder="比如 203.0.113.24" autocomplete="off"></label>
                <div class="field"><span>哪家的</span>${segH('cloud', [['阿里云', '阿里云'], ['腾讯云', '腾讯云'], ['其他', '其他']], d.cloud)}</div>
                <div class="field"><span>怎么登录</span>${segH('login', [['pw', 'root 密码'], ['pem', '密钥文件'], ['nopw', '已配好免密']], d.login)}</div>
                ${login}
            </div></div>
            <div class="wstep"><span class="wn">2</span><div class="wb">
                <h4>开端口 25565</h4>
                <p class="note">轻量服务器默认只放行 22、80、443，要给 MC 开一个口子。</p>
                ${d.cloud === '其他' ? '<p class="note">其他云厂商：去它的控制台（安全组 / 防火墙）放行 TCP 25565。</p>' : segH('fw', [['ak', '用子账号 AccessKey 自动加'], ['console', '我去控制台加']], d.fw) + fw}
            </div></div>
            <div class="wstep"><span class="wn">3</span><div class="wb">
                <h4>装什么</h4>
                <dl class="facts">
                    <dt>服务端</dt><dd>${i.loader ? `${esc(i.loader)} ${esc(i.loaderVer)}` : '原版'} · ${esc(i.mc)} · Java ${i.javaNeed}</dd>
                    ${i.loader ? `<dt>Mod</dt><dd>搬上 ${sm.length} 个；剔除 ${cm.length} 个只在客户端用的${cm.length ? `（${esc(cm.slice(0, 3).map((m) => m.name).join('、'))}${cm.length > 3 ? ' 等' : ''}）` : ''}</dd>` : ''}
                </dl>
                ${w ? `<label class="check" for="hfWorld"><input type="checkbox" id="hfWorld" data-hf="world"${d.world ? ' checked' : ''}><span><b>把「${esc(w.name)}」搬上去</b><small>${esc(w.size)}，当作服务器的世界；以后也能拉回电脑或手机</small></span></label>` : ''}
                <label class="check" for="hfWl"><input type="checkbox" id="hfWl" data-hf="wl"${d.wl ? ' checked' : ''}><span><b>白名单</b><small>只让 PingYi、小明、阿哲进来，以后在控制台加人</small></span></label>
                <label class="check" for="hfBk"><input type="checkbox" id="hfBk" data-hf="bk"${d.bk ? ' checked' : ''}><span><b>每天 04:00 备份</b><small>留最近 7 份，坏了能回滚</small></span></label>
                <p class="note inline-ic">${icon('shield', 13)}开机自启、崩了自动重启，这两项一直开着。</p>
            </div></div>
        </div>
        <footer class="dr-foot">
            <span class="hint">大约 5 分钟（原型里十几秒）</span>
            <button type="button" class="go" id="drCreate" data-act="dr-create"${d.ip.trim() ? '' : ' disabled'}>${icon('server', 22)}<span class="txt"><span class="lbl">开始开服</span><span class="sub" id="drSub">${hostSub(d)}</span></span><kbd>Enter</kbd></button>
        </footer>
    </section>`;
}

export const hostSub = (d) => esc(`${d.cloud} · ${d.ip.trim() || '先填 IP'}`);

// ---------- 导入已有服务器（侧边面板） ----------
export const importOk = (d) => !!(d.via === 'addr' ? d.addr.trim() : d.via === 'ssh' ? d.ip.trim() : d.url.trim());
export const importSub = (d) => esc(d.via === 'addr' ? d.addr.trim() || '先填地址' : d.via === 'ssh' ? d.ip.trim() || '先填 IP' : d.url.trim() || '先填面板地址');

export function importDrawer(c) {
    const d = c.S.drawer;
    let body;
    if (d.via === 'addr') {
        body = `<label class="field" for="hfAddr"><span>服务器地址</span><input id="hfAddr" type="text" class="mono" data-hf="addr" value="${esc(d.addr)}" placeholder="比如 mc.example.com 或 203.0.113.9:25565" autocomplete="off"></label>
            <p class="note">加进服务器列表；还能生成邀请码，把地址和整合包一起发给朋友。</p>`;
    } else if (d.via === 'ssh') {
        body = `<label class="field" for="hfIp"><span>服务器 IP</span><input id="hfIp" type="text" class="mono" data-hf="ip" value="${esc(d.ip)}" placeholder="比如 203.0.113.9" autocomplete="off"></label>
            <div class="field"><span>怎么登录</span>${segH('login', [['pw', 'root 密码'], ['pem', '密钥文件'], ['nopw', '已配好免密']], d.login)}</div>
            ${d.login === 'pw' ? '<label class="field" for="hfPw"><span>root 密码</span><input id="hfPw" type="password" data-hf="pw" placeholder="只用一次：登录后换成密钥" autocomplete="off"></label>' : ''}
            <p class="note">登录后找已有的服务端目录（比如 <span class="mono">/opt/minecraft</span>），接管启停、控制台、备份。已有配置不动，要改先问你。</p>`;
    } else {
        body = `<div class="field"><span>面板</span>${segH('panel', [['mcsm', 'MCSManager'], ['ptero', '翼龙 Pterodactyl']], d.panel)}</div>
            <label class="field" for="hfUrl"><span>面板地址</span><input id="hfUrl" type="text" class="mono" data-hf="url" value="${esc(d.url)}" placeholder="${d.panel === 'mcsm' ? '比如 http://203.0.113.9:23333' : '比如 https://panel.example.com'}" autocomplete="off"></label>
            <label class="field" for="hfKey"><span>${d.panel === 'mcsm' ? 'API Key' : '客户端 API Key'}</span><input id="hfKey" type="password" data-hf="key" autocomplete="off"></label>
            <p class="note">${d.panel === 'mcsm' ? 'MCSManager 的 Key 在「用户设置」里。管理员账号的 Key 带管理员权限，建议新建一个只管这台实例的子用户。' : '翼龙面板用「客户端 API Key」（Account › API Credentials），只能管你自己的服务器。'}</p>`;
    }
    return `<section class="drawer" role="dialog" aria-label="导入已有服务器">
        <header class="dr-head"><h2>导入已有服务器</h2><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
        <div class="dr-body">
            <div class="field"><span>怎么接进来</span>${segH('via', [['addr', '只有地址'], ['ssh', 'SSH 登录'], ['panel', '面板服']], d.via)}</div>
            ${body}
        </div>
        <footer class="dr-foot">
            <span class="hint">Key 和密码只进本机的加密保管库</span>
            <button type="button" class="go" id="drCreate" data-act="dr-create"${importOk(d) ? '' : ' disabled'}>${icon('upload', 22)}<span class="txt"><span class="lbl">导入</span><span class="sub" id="drSub">${importSub(d)}</span></span><kbd>Enter</kbd></button>
        </footer>
    </section>`;
}
