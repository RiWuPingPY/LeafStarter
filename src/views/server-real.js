// 真程序：实例页「联机」标签里的常驻服务器（M10），和几个侧边面板
import { esc } from '../ui.js';
import { icon } from '../icons.js';
import { mpNow } from '../mp-real.js';

const seg = (act, list, cur) => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="${act}" data-v="${v}" class="${String(cur) === v ? 'on' : ''}" aria-pressed="${String(cur) === v}">${t}</button>`).join('')}</div>`;

export const STEPS = [['os', '识别系统'], ['deps', '装基础工具'], ['java', '装 Java'], ['server', '装服务端'], ['mods', '同步 Mod 和配置'], ['config', '同意 EULA、写服务器设置'], ['ctl', '写控制脚本'], ['firewall', '放行端口'], ['service', '开机自启和定时备份'], ['start', '启动服务器']];
const KIND = { ssh: 'SSH', mcsm: 'MCSManager', ptero: '翼龙面板' };
const STATE = { running: ['ok', '在运行'], starting: ['acc', '启动中'], stopping: ['warn', '停止中'], stopped: ['', '停着'], notSetup: ['warn', '还没开服'], unknown: ['', '状态未知'] };

// 按 Mod 数量推荐配置（参考：20 人以内的中型整合包 2 核 4G）
function suggest(i) {
    const n = i.mods?.length || 0;
    if (n === 0) return ['2 核 2G', '原版或少量 Mod，十来个人够用'];
    if (n <= 60) return ['2 核 4G', `${n} 个 Mod，20 人以内够用`];
    return ['4 核 8G', `${n} 个 Mod 算大型整合包`];
}

// 「联机」标签：现在一起玩（临时联机）/ 常驻服务器，两个都是官方插件，关了哪个就只剩另一个
export function realServers(c) {
    const now = !c.plugOff('mp');
    const host = !c.plugOff('server');
    const mode = !now ? 'host' : !host ? 'now' : c.S.mpMode === 'host' ? 'host' : 'now';
    const btn = (v, ic, t, sub) => `<button type="button" role="tab" class="mode${mode === v ? ' on' : ''}" data-act="mp-mode" data-v="${v}" aria-selected="${mode === v}">
        <span class="mode-ic">${icon(ic, 20)}</span><span class="tx"><span class="nm">${t}</span><span class="sb">${sub}</span></span></button>`;
    const tabs = now && host ? `<div class="modes" data-on="${mode}" role="tablist" aria-label="联机方式"><i class="mode-ind" aria-hidden="true"></i>
        ${btn('now', 'users', '现在一起玩', '你开着游戏，朋友进来 · 免费')}${btn('host', 'server', '常驻服务器', '朋友随时能上，不用你开着电脑')}</div>` : '';
    return `${tabs}<div class="mp-body" id="mpBody">${realMpBody(c, mode)}</div>`;
}

export function realMpBody(c, mode) {
    return mode === 'host' ? hostBody(c) : mpNow(c);
}

function hostBody(c) {
    const i = c.inst;
    const R = c.R.srv;
    const list = (R?.list || []).filter((s) => s.inst === i.id);
    if (!R?.loaded) return '<section class="card"><p class="note">正在读服务器列表…</p></section>';
    if (!list.length) {
        const [cfg, why] = suggest(i);
        return `<section class="card"><h3>${icon('server', 18)}开一台常驻服务器</h3>
            <p>推荐 <b>${cfg}</b>（${why}）的轻量服务器，系统选 Ubuntu 或 Debian。填好 IP 和密码，剩下的启动器来做。</p>
            <div class="actions"><button type="button" class="btn pri" data-act="srv-add">${icon('plus')}填服务器 IP 和密码</button>
                <button type="button" class="btn" data-act="open-url" data-v="https://www.aliyun.com/product/swas">${icon('link', 13)}阿里云轻量</button>
                <button type="button" class="btn" data-act="open-url" data-v="https://cloud.tencent.com/product/lighthouse">${icon('link', 13)}腾讯云轻量</button>
                <button type="button" class="btn ghost" data-act="panel-add">已经有面板服</button></div></section>`;
    }
    return `<div class="toolbar"><span class="grow"></span><button type="button" class="btn" data-act="srv-add">${icon('plus')}再加一台</button></div>` + list.map((s) => card(c, s)).join('');
}

function card(c, s) {
    const R = c.R.srv;
    const st = R.status.get(s.id);
    const steps = R.steps.get(s.id) || {};
    const live = Object.values(steps).some((x) => x.state === 'run');
    const ready = s.kind !== 'ssh' || s.setup;
    let status = '';
    if (st?.loading) status = '<span class="chip acc">查状态…</span>';
    else if (st?.error) status = `<span class="chip warn" title="${esc(st.error)}">连不上</span>`;
    else if (st) {
        const [cls, t] = STATE[st.state] || STATE.unknown;
        status = `<span class="chip ${cls}">${t}</span>`;
    }
    const facts = st && !st.error && !st.loading ? [
        st.players != null ? `在线 ${st.players}/${st.maxPlayers}${st.names?.length ? `：${st.names.map(esc).join('、')}` : ''}` : '',
        st.memMb ? `内存 ${st.memMb} MB` : '',
        st.load != null ? `负载 ${st.load}` : '',
        st.cpu != null ? `CPU ${Math.round(st.cpu)}%` : '',
        st.tps ? esc(st.tps) : '',
    ].filter(Boolean).join(' · ') : '';
    // 剧本进度：做完的、正在做的、失败的
    const showSteps = s.kind === 'ssh' && (!s.setup || live || s.failed);
    const stepRows = showSteps ? `<div class="steps srv-steps">${STEPS.map(([id, title]) => {
        const x = steps[id];
        const state = x?.state || (s.failed?.[0] === id ? 'fail' : s.done.includes(id) ? 'done' : '');
        return `<div class="step ${state}"><span class="sic">${state === 'done' ? icon('check', 12) : state === 'fail' ? icon('x', 12) : ''}</span><span class="tx"><span class="nm">${title}</span>${x?.out && state !== 'run' ? `<span class="sb">${esc(x.out.split('\n').slice(-1)[0].slice(0, 120))}</span>` : ''}</span></div>`;
    }).join('')}</div>` : '';
    const fail = s.failed && !live ? `<div class="callout warn">${icon('alert', 14)}<span>「${esc(STEPS.find((x) => x[0] === s.failed[0])?.[1] || s.failed[0])}」这一步失败了。重跑会从头检查，做过的会跳过。</span></div>
        <pre class="cmdbox srv-out">${esc(s.failed[1].split('\n').slice(-14).join('\n'))}</pre>` : '';
    const setupBtn = s.kind === 'ssh' && (!s.setup || s.failed)
        ? `<button type="button" class="btn ${s.failed ? '' : 'pri'}" data-act="srv-setup" data-id="${s.id}"${live ? ' disabled' : ''}>${icon('play', 13)}${live ? '正在开服…' : s.failed ? '重试' : s.done.length ? '接着开服' : '开服'}</button>${s.failed ? `<button type="button" class="btn" data-act="srv-ai" data-id="${s.id}">${icon('sparkles', 13)}交给 AI 看看</button>` : ''}`
        : '';
    const running = st?.state === 'running';
    const ops = ready ? `
        ${running ? `<button type="button" class="btn" data-act="srv-power" data-id="${s.id}" data-v="restart">${icon('refresh', 13)}重启</button><button type="button" class="btn" data-act="srv-power" data-id="${s.id}" data-v="stop">${icon('stop', 13)}停止</button>` : `<button type="button" class="btn pri" data-act="srv-power" data-id="${s.id}" data-v="start">${icon('play', 13)}启动</button>`}
        <button type="button" class="btn" data-act="srv-console" data-id="${s.id}">${icon('terminal', 13)}控制台</button>
        <button type="button" class="btn" data-act="srv-sync" data-id="${s.id}">${icon('upload', 13)}同步 Mod</button>
        ${s.kind === 'ssh' ? `<button type="button" class="btn" data-act="srv-backup" data-id="${s.id}">${icon('package', 13)}备份</button><button type="button" class="btn" data-act="srv-world" data-id="${s.id}">${icon('globe', 13)}世界</button>` : ''}` : '';
    return `<section class="card srv" data-srv="${s.id}">
        <h3>${icon('server', 18)}${esc(s.name)}<span class="chip">${KIND[s.kind] || s.kind}</span>${status}<span class="sp"></span>
            <button type="button" class="iconbtn" data-act="srv-status" data-id="${s.id}" aria-label="刷新状态" title="刷新状态">${icon('refresh')}</button>
            <button type="button" class="iconbtn" data-act="srv-cfg" data-id="${s.id}" aria-label="服务器设置" title="服务器设置">${icon('sliders')}</button></h3>
        <div class="srv-addr"><span class="note">朋友在「多人游戏 → 添加服务器」里填</span><code class="mono">${esc(s.address)}</code><button type="button" class="btn sm ghost" data-act="srv-copy" data-v="${esc(s.address)}">${icon('copy', 13)}复制</button></div>
        ${facts ? `<p class="note">${facts}</p>` : ''}
        ${st?.error ? `<p class="callout warn">${icon('alert', 14)}<span>${esc(st.error)}</span></p>` : ''}
        ${stepRows}${fail}
        <div class="actions">${setupBtn}${ops}</div>
    </section>`;
}

// ---------- 侧边面板 ----------
const shell = (title, body, foot = '') => `<section class="drawer" role="dialog" aria-label="${esc(title)}">
    <header class="dr-head"><h2>${esc(title)}</h2><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
    <div class="dr-body">${body}${foot ? `<div class="actions">${foot}</div>` : ''}</div></section>`;

export function srvAddDrawer(c) {
    const d = c.S.drawer;
    const busy = d.state === 'busy';
    const dis = busy ? ' disabled' : '';
    const body = `<p class="note">填云控制台里看到的公网 IP。密码只用这一次：登录后装上启动器的密钥，以后都用密钥登录，密码不保存。</p>
        <label class="field" for="saHost"><span>服务器 IP</span><input id="saHost" type="text" value="${esc(d.host || '')}" placeholder="比如 47.100.1.2" autocomplete="off"${dis}></label>
        <div class="field-row"><label class="field" for="saUser"><span>用户名</span><input id="saUser" type="text" value="${esc(d.user || 'root')}" autocomplete="off"${dis}></label>
            <label class="field" for="saPort"><span>SSH 端口</span><input id="saPort" type="text" value="${esc(String(d.port || 22))}" inputmode="numeric"${dis}></label></div>
        <div class="field"><span>登录方式</span>${seg('sa-mode', [['pw', '密码'], ['key', '密钥文件（.pem）']], d.mode || 'pw')}</div>
        ${(d.mode || 'pw') === 'pw'
            ? `<label class="field" for="saPass"><span>密码</span><input id="saPass" type="password" autocomplete="off"${dis}></label>`
            : `<label class="field" for="saKey"><span>密钥文件</span><input id="saKey" type="file"${dis}></label>${d.keyName ? `<p class="note">选了 ${esc(d.keyName)}</p>` : ''}`}
        <label class="field" for="saName"><span>起个名字（可以不填）</span><input id="saName" type="text" value="${esc(d.name || '')}" placeholder="比如 生存服"${dis}></label>
        ${d.error ? `<p class="callout warn">${icon('alert', 14)}<span>${esc(d.error)}</span></p>` : ''}`;
    return shell(`给「${c.inst?.name || ''}」加服务器`, body, `<button type="button" class="btn pri" data-act="sa-go"${dis}>${busy ? '正在连…' : '连接'}</button><button type="button" class="btn ghost" data-act="dr-close">取消</button>`);
}

export function panelAddDrawer(c) {
    const d = c.S.drawer;
    const busy = d.state === 'busy';
    const body = `<p class="note">用面板的 API Key 接管已有的服务器：能启停、看控制台、传 Mod。MCSManager 的管理员 Key 权限太大，建议在面板里建一个只管这台实例的子用户，用它的 Key。</p>
        <div class="field"><span>面板</span>${seg('pa-kind', [['mcsm', 'MCSManager'], ['ptero', '翼龙 Pterodactyl']], d.pk || 'mcsm')}</div>
        <label class="field" for="paBase"><span>面板地址</span><input id="paBase" type="text" value="${esc(d.base || '')}" placeholder="${(d.pk || 'mcsm') === 'mcsm' ? 'http://1.2.3.4:23333' : 'https://panel.example.com'}" autocomplete="off"></label>
        <label class="field" for="paKey"><span>API Key</span><input id="paKey" type="password" autocomplete="off" placeholder="${(d.pk || 'mcsm') === 'mcsm' ? '个人资料 → API 密钥' : '账户 → API 凭据（ptlc_ 开头）'}"></label>
        <div class="actions"><button type="button" class="btn" data-act="pa-probe"${busy ? ' disabled' : ''}>${busy ? '正在连…' : '列出能管的实例'}</button></div>
        ${d.list ? `<div class="rows">${d.list.length ? d.list.map((x, k) => `<button type="button" class="row pick${d.pick === k ? ' on' : ''}" data-act="pa-pick" data-v="${k}"><span class="tx"><span class="nm">${esc(x.name || x.id)}</span><span class="sb mono">${esc(x.id)}</span></span>${d.pick === k ? icon('check', 14) : ''}</button>`).join('') : '<div class="row empty">这个 Key 下没有实例</div>'}</div>` : ''}
        ${d.pick != null ? `<label class="field" for="paAddr"><span>朋友连哪个地址（IP 或域名加端口）</span><input id="paAddr" type="text" value="${esc(d.addr || '')}" placeholder="比如 mc.example.com:25565"></label>` : ''}
        ${d.error ? `<p class="callout warn">${icon('alert', 14)}<span>${esc(d.error)}</span></p>` : ''}`;
    return shell('接管面板服', body, d.pick != null ? `<button type="button" class="btn pri" data-act="pa-go">加上</button><button type="button" class="btn ghost" data-act="dr-close">取消</button>` : '');
}

export function srvConsoleDrawer(c) {
    const d = c.S.drawer;
    const s = c.R.srv.list.find((x) => x.id === d.id);
    const body = `<pre class="cmdbox srv-log" id="srvLog">${esc(d.log ?? '正在读…')}</pre>
        ${d.out?.length ? `<div class="srv-outs">${d.out.map(([cmd, r]) => `<p class="mono"><b>&gt; ${esc(cmd)}</b><br>${esc(r)}</p>`).join('')}</div>` : ''}
        <div class="srv-cmd"><input id="srvCmd" type="text" class="mono" placeholder="控制台命令，比如 list、say 你好、whitelist add 名字" autocomplete="off"><button type="button" class="btn pri" data-act="srv-send">发送</button></div>
        <div class="actions"><button type="button" class="btn sm" data-act="srv-log" data-id="${d.id}">${icon('refresh', 13)}刷新日志</button></div>`;
    return shell(`控制台 · ${s?.name || ''}`, body);
}

export function srvCfgDrawer(c) {
    const d = c.S.drawer;
    const s = c.R.srv.list.find((x) => x.id === d.id);
    if (!s) return shell('服务器设置', '<p class="note">找不到这台服务器</p>');
    const mem = [['0', '自动（2G）'], ['2048', '2G'], ['3072', '3G'], ['4096', '4G'], ['6144', '6G'], ['8192', '8G']];
    const ssh = s.kind === 'ssh';
    const body = `<label class="field" for="scName"><span>名字</span><input id="scName" type="text" value="${esc(s.name)}"></label>
        ${ssh ? `<label class="field" for="scMem"><span>内存</span><select id="scMem">${mem.map(([v, t]) => `<option value="${v}"${String(s.memMb) === v ? ' selected' : ''}>${t}</option>`).join('')}</select></label>
        <label class="field" for="scPort"><span>游戏端口</span><input id="scPort" type="text" value="${s.port}" inputmode="numeric"></label>
        <label class="tick"><input type="checkbox" id="scOnline"${s.online ? ' checked' : ''}>正版验证（只有正版账户能进；关掉的话离线账户也能进，按 D6 的规则）</label>
        <label class="field" for="scWl"><span>白名单（一行一个游戏名；空着就不开白名单）</span><textarea id="scWl" rows="4">${esc(s.whitelist.join('\n'))}</textarea></label>
        <label class="field" for="scMotd"><span>服务器简介（MOTD）</span><input id="scMotd" type="text" value="${esc(s.motd)}" placeholder="${esc(s.name)}（LeafStarter）"></label>
        <p class="note">改了内存、端口、正版验证、白名单要「重新跑剧本」才生效，做过的步骤会跳过。</p>` : `<p class="note">面板：${esc(s.panelBase)} · 实例 ${esc(s.panelInst)}</p>`}
        <div class="actions"><button type="button" class="btn pri" data-act="sc-save">保存</button>${ssh && s.setup ? `<button type="button" class="btn" data-act="srv-setup" data-id="${s.id}">${icon('play', 13)}重新跑剧本</button>` : ''}</div>
        ${ssh ? `<h3 class="sub">安全</h3>
        <p class="note">服务器身份（指纹）：<span class="mono">${esc(s.fingerprint)}</span></p>
        <div class="actions"><button type="button" class="btn sm" data-act="srv-harden" data-id="${s.id}">关掉密码登录</button><button type="button" class="btn sm ghost" data-act="srv-retrust" data-id="${s.id}">重新信任（重装过系统时用）</button></div>
        <h3 class="sub">云防火墙</h3>
        <p class="note">轻量服务器默认只放行 22、80、443。可以去控制台手动加一条 TCP ${s.port}，或者填一个只有防火墙权限的子账号 AccessKey，让启动器加（Key 只用这一次，不保存）。</p>
        <div class="field">${seg('fw-prov', [['aliyun', '阿里云'], ['tencent', '腾讯云']], d.prov || 'aliyun')}</div>
        <div class="field-row"><label class="field" for="fwAk"><span>${(d.prov || 'aliyun') === 'aliyun' ? 'AccessKey ID' : 'SecretId'}</span><input id="fwAk" type="text" autocomplete="off"></label>
            <label class="field" for="fwSk"><span>${(d.prov || 'aliyun') === 'aliyun' ? 'AccessKey Secret' : 'SecretKey'}</span><input id="fwSk" type="password" autocomplete="off"></label></div>
        <div class="actions"><button type="button" class="btn sm" data-act="srv-fw" data-id="${s.id}">放行 TCP ${s.port}</button></div>` : ''}
        <h3 class="sub">移除</h3>
        <p class="note">只是从启动器里拿掉，服务器上的东西不动。</p>
        <div class="actions"><button type="button" class="btn sm ghost danger" data-act="srv-del" data-id="${s.id}">${icon('trash', 13)}从启动器移除</button></div>`;
    return shell(`服务器设置 · ${s.name}`, body);
}

// 世界：传本地的上去 / 拉服务器的回来
export function srvWorldPop(c, s) {
    const ws = c.inst?.worlds || [];
    return `<div class="pop-h">「${esc(s.name)}」的世界</div>
        <button type="button" class="pop-item" data-act="srv-down" data-id="${s.id}">${icon('download', 15)}<span class="tx"><span class="nm">拉回这台电脑</span><span class="sb">另存成一个新存档，不覆盖本地的</span></span></button>
        <div class="pop-sub">把本地的世界传上去（服务器上原来的先备份）</div>
        ${ws.length ? ws.map((w) => `<button type="button" class="pop-item" data-act="srv-up" data-id="${s.id}" data-v="${esc(w.folder)}">${icon('upload', 15)}<span class="tx"><span class="nm">${esc(w.name)}</span></span></button>`).join('') : '<p class="pop-note">这个实例还没有存档</p>'}`;
}
