// AI 源：可以配好几个，按用途分配；高级参数按模型能力显示。另外是 MCP 一键接入和功能包
import { $, toast, store, esc } from './ui.js';
import { icon } from './icons.js';

// 常见服务商做成预设：地址、模型、能力都填好
export const PRESETS = {
    claude: { name: 'Claude', kind: 'anthropic', base: 'https://api.anthropic.com', model: 'claude-opus-5-5', models: ['claude-opus-5-5', 'claude-sonnet-5-5', 'claude-haiku-5-5'], tools: true, vision: true, params: ['max', 'effort', 'stop', 'headers', 'extra'], note: '这个模型不接受 temperature、top_p（传了会报错），所以不显示；想要更稳或更快，调「推理强度」' },
    deepseek: { name: 'DeepSeek', kind: 'openai', base: 'https://api.deepseek.com', model: 'deepseek-chat', tools: true, vision: false, params: ['temp', 'top_p', 'max', 'stop', 'extra'] },
    qwen: { name: '通义千问', kind: 'openai', base: 'https://dashscope.aliyuncs.com/compatible-mode/v1', model: 'qwen-plus', tools: true, vision: false, params: ['temp', 'top_p', 'max', 'stop', 'seed', 'extra'] },
    kimi: { name: 'Kimi', kind: 'openai', base: 'https://api.moonshot.cn/v1', model: '', tools: true, vision: false, params: ['temp', 'top_p', 'max', 'stop', 'extra'] },
    glm: { name: '智谱', kind: 'openai', base: 'https://open.bigmodel.cn/api/paas/v4', model: '', tools: true, vision: false, params: ['temp', 'top_p', 'max', 'stop', 'extra'] },
    custom: { name: 'OpenAI 兼容接口', kind: 'openai', base: '', model: '', unknown: true },
    ollama: { name: '本机 Ollama', kind: 'ollama', base: 'http://127.0.0.1:11434', model: 'qwen2.5:7b', local: true, tools: true, vision: false, params: ['temp', 'top_p', 'ctx', 'seed', 'extra'] },
    llama: { name: '本机 llama.cpp / LM Studio', kind: 'openai', base: 'http://127.0.0.1:8080/v1', model: '', local: true, unknown: true },
    comfy: { name: '本机 ComfyUI', kind: 'image', base: 'http://127.0.0.1:8188', model: '皮肤工作流（社区模型）', local: true, params: ['steps', 'size', 'seed'] },
    imgcloud: { name: '云端生图', kind: 'image', base: '', model: '', params: ['size', 'steps', 'seed'] },
};
export const GROUPS = [['云端', ['claude', 'deepseek', 'qwen', 'kimi', 'glm', 'custom']], ['本机', ['ollama', 'llama']], ['生图（皮肤用）', ['comfy', 'imgcloud']]];
export const KIND = { anthropic: 'Anthropic', openai: 'OpenAI 兼容', ollama: 'Ollama', image: '生图' };
export const USES = [['chat', '对话和排查', '要能调工具'], ['tr', '翻译', '量大，挑便宜的'], ['img', '生图', '皮肤工作台用'], ['cmd', '写命令', '指令台用，小模型就够']];
export const PARAMS = {
    temp: ['temperature', 'range', 0, 2, 0.1, 0.7],
    top_p: ['top_p', 'range', 0, 1, 0.05, 1],
    max: ['最大输出', 'num', 4096, 'token'],
    ctx: ['上下文长度', 'num', 8192, 'token'],
    effort: ['推理强度', 'seg', ['low', 'medium', 'high'], 'medium'],
    stop: ['停止词', 'text', '', '用逗号隔开'],
    seed: ['随机种子', 'text', '', '留空就随机'],
    headers: ['自定义请求头', 'text', '', '比如 X-Org: 123'],
    extra: ['额外参数', 'text', '', 'JSON，原样透传给接口'],
    steps: ['步数', 'num', 28, ''],
    size: ['尺寸', 'text', '512x512', ''],
};
const ALL = ['temp', 'top_p', 'max', 'ctx', 'effort', 'stop', 'seed', 'headers', 'extra'];

// 这个源该显示哪些高级参数：认得的模型只显示它支持的；认不出的全部放开并提示
export function paramsFor(src) {
    const p = PRESETS[src.p];
    if (p.kind === 'image') return { list: p.params, note: '' };
    if (p.unknown || (p.kind === 'anthropic' && !/^claude-/.test(src.model))) return { list: ALL, note: '认不出这个模型，参数全部放开；有的服务商不认其中几项，传了可能报错，「测试连接」能看出来', warn: true };
    if (p.models && src.model && !p.models.includes(src.model)) return { list: p.params, note: p.note || '' };
    return { list: p.params, note: p.note || '' };
}
export const ready = (s) => !!s && (s.key || PRESETS[s.p].local);
export const isImg = (s) => PRESETS[s.p].kind === 'image';

export function createAiSrc(api) {
    const { S } = api;
    const saved = store.get('ai', null);
    S.ai = saved || { list: [], use: {}, edit: null };
    S.ai.edit = null;
    const save = () => store.set('ai', { list: S.ai.list, use: S.ai.use });
    const byId = (id) => S.ai.list.find((s) => s.id === id);
    // 老版本原型只存了一个服务商：转成一个源
    if (!saved) {
        const old = store.get('aiProvider', 'none');
        if (old && old !== 'none' && PRESETS[old === 'openai' ? 'custom' : old]) add(old === 'openai' ? 'custom' : old, { key: store.get('aiKey', false) }, true);
    }

    function add(p, extra = {}, quiet) {
        const pr = PRESETS[p];
        const n = S.ai.list.filter((s) => s.p === p).length;
        const s = { id: 'a' + Date.now().toString(36) + Math.random().toString(36).slice(2, 5), p, name: pr.name + (n ? ` ${n + 1}` : ''), base: pr.base, model: pr.model, key: false, test: null, prm: {}, ...extra };
        S.ai.list.push(s);
        // 用途还空着的，先分给它
        for (const [u] of USES) {
            if (S.ai.use[u] && byId(S.ai.use[u])) continue;
            if ((u === 'img') === isImg(s)) S.ai.use[u] = s.id;
        }
        if (!quiet) save();
        return s;
    }
    const aiFor = (use) => {
        const s = byId(S.ai.use[use]);
        return ready(s) ? s : null;
    };

    function repaint(focusId) {
        if (S.page === 'settings') api.paintView();
        api.agent?.rerender();
        if (focusId) requestAnimationFrame(() => $(`[data-src="${focusId}"]`)?.scrollIntoView({ block: 'nearest', behavior: 'smooth' }));
    }

    function test(id) {
        const s = byId(id);
        if (!s) return;
        s.test = { state: 'run' };
        repaint();
        setTimeout(() => {
            const p = PRESETS[s.p];
            if (!ready(s)) s.test = { state: 'fail', msg: '没填 Key' };
            else if (!s.model && p.kind !== 'image') s.test = { state: 'fail', msg: '没填模型名' };
            else s.test = { state: 'ok', ms: p.local ? 120 + Math.round(Math.random() * 80) : 480 + Math.round(Math.random() * 500), tools: p.unknown ? null : !!p.tools, vision: p.unknown ? null : !!p.vision, kind: p.kind };
            repaint();
        }, 800);
    }

    // ---------- MCP 一键接入 ----------
    S.mcpw = store.get('mcpw', {});
    const saveMcp = () => store.set('mcpw', S.mcpw);

    // ---------- 功能包 ----------
    S.ext = { mp: true, world: false, tr: false, ...store.get('ext', {}) };
    S.simple = store.get('simple', false);
    S.extBusy = null;
    const saveExt = () => store.set('ext', S.ext);

    function extInstall(id) {
        const P = PACKS[id];
        if (S.ext[id]) return;
        if (S.extBusy) return toast(`「${PACKS[S.extBusy].name}」还在装，等它装完`);
        S.extBusy = id;
        S.extPct = 0;
        S.extStage = P.stages[0][1];
        repaintExt();
        const t = api.addTask(`装功能包 · ${P.name}`, P.from);
        api.runTask(t, P.ms, () => {
            S.ext[id] = true;
            S.extBusy = null;
            saveExt();
            repaintExt();
            toast(`装好了「${P.name}」，不用重启`);
        }, (pct) => {
            S.extPct = pct;
            S.extStage = P.stages.filter(([at]) => pct >= at).at(-1)[1];
            t.stage = S.extStage;
            document.querySelectorAll(`[data-extp="${id}"] i`).forEach((el) => el.style.setProperty('--p', pct + '%'));
            document.querySelectorAll(`[data-extt="${id}"]`).forEach((el) => { el.textContent = `${S.extStage} ${pct}%`; });
        });
    }
    function extRemove(id) {
        const P = PACKS[id];
        if (id === 'mp' && api.INSTANCES.some((i) => i.room || i.srv?.phase === 'setup')) return toast('有房间开着或正在开服，先关掉再移除联机');
        S.ext[id] = false;
        saveExt();
        repaintExt();
        toast(`移除了「${P.name}」，腾出${P.size}`, {
            undo: () => {
                S.ext[id] = true;
                saveExt();
                repaintExt();
            },
        });
    }
    function repaintExt() {
        if (S.page === 'settings') api.paintView();
        else if (S.page === 'home') {
            api.paintTabs();
            api.paintBody();
        }
        if (S.drawer?.kind === 'i18n') api.paintDrawer();
    }

    return {
        aiFor, add, save,
        demo() {
            const s = add('claude', { key: true });
            for (const [u] of USES) if (u !== 'img' && !aiFor(u)) S.ai.use[u] = s.id;
            save();
        },
        extInstall,
        actions: {
            'ais-add': (el) => api.openPop(el, addPop(), 'below'),
            'ais-new': (el) => {
                api.closePop(false);
                const s = add(el.dataset.v);
                S.ai.edit = s.id;
                repaint(s.id);
            },
            'ais-edit': (el) => {
                S.ai.edit = S.ai.edit === el.dataset.id ? null : el.dataset.id;
                repaint();
            },
            'ais-test': (el) => test(el.dataset.id),
            'ais-model': (el) => {
                const s = byId(el.dataset.id);
                s.model = el.dataset.v;
                s.test = null;
                save();
                repaint();
            },
            'ais-effort': (el) => {
                const s = byId(el.dataset.id);
                s.prm.effort = el.dataset.v;
                save();
                repaint();
            },
            'ais-del': (el) => {
                const s = byId(el.dataset.id);
                const k = S.ai.list.indexOf(s);
                const use = { ...S.ai.use };
                S.ai.list.splice(k, 1);
                for (const [u] of USES) if (S.ai.use[u] === s.id) delete S.ai.use[u];
                save();
                repaint();
                toast(`删掉了「${s.name}」${s.key ? '，Key 也从保管库里删了' : ''}`, {
                    undo: () => {
                        S.ai.list.splice(k, 0, s);
                        S.ai.use = use;
                        save();
                        repaint();
                    },
                });
            },
            'mcp-write': (el) => api.openPop(el, mcpPop(el.dataset.v), 'below'),
            'mcp-write-yes': (el) => {
                const c = MCP_CLIENTS.find((x) => x[0] === el.dataset.v);
                api.closePop(false);
                S.mcpw[c[0]] = true;
                saveMcp();
                repaint();
                toast(`写进了 ${c[1]} 的配置（改之前备份了原文件）。重开 ${c[1]} 就能用`, {
                    undo: () => {
                        S.mcpw[c[0]] = false;
                        saveMcp();
                        repaint();
                    },
                });
            },
            'mcp-unwrite': (el) => {
                S.mcpw[el.dataset.v] = false;
                saveMcp();
                repaint();
                toast('从它的配置里删掉了 LeafStarter 那几行，别的没动');
            },
            'ext-add': (el) => extInstall(el.dataset.v),
            'ext-del': (el) => extRemove(el.dataset.v),
            editions: (el) => api.openPop(el, editionsPop(), 'below'),
        },
        input(t) {
            const id = t.dataset.ais;
            if (!id) return false;
            const s = byId(id);
            const f = t.dataset.f;
            if (f === 'key') {
                s.key = t.value.trim().length > 0 || s.key;
                const st = $(`[data-keyst="${id}"]`);
                if (st) {
                    st.textContent = s.key ? '已存进保管库' : '没填';
                    st.classList.toggle('ok', s.key);
                }
            } else if (f.startsWith('prm:')) s.prm[f.slice(4)] = t.value;
            else {
                s[f] = t.value;
                if (f === 'name') {
                    const nm = $(`[data-srcname="${id}"]`);
                    if (nm) nm.textContent = t.value || '（没名字）';
                }
            }
            s.test = f === 'name' || f.startsWith('prm:') ? s.test : null;
            save();
            api.agent?.rerender();
            return true;
        },
        change(t) {
            if (t.dataset.use) {
                S.ai.use[t.dataset.use] = t.value || undefined;
                save();
                repaint();
                return true;
            }
            if (t.dataset.ais && (t.dataset.f === 'model' || t.dataset.f === 'key')) {
                repaint();
                return true;
            }
            if (t.dataset.ais && t.dataset.f?.startsWith('prm:') && t.type === 'range') {
                return true;
            }
            return false;
        },
    };
}

function addPop() {
    return '<div class="pop-h">加一个 AI 源</div>' + GROUPS.map(([g, ks]) => `<div class="pop-sub">${g}</div>` + ks.map((k) => {
        const p = PRESETS[k];
        return `<button type="button" class="pop-item" data-act="ais-new" data-v="${k}">${icon(p.kind === 'image' ? 'image' : p.local ? 'monitor' : 'cloud', 16)}<span class="tx"><span class="nm">${esc(p.name)}</span><span class="sb">${KIND[p.kind]}${p.base ? ' · ' + esc(p.base.replace(/^https?:\/\//, '')) : ' · 自己填地址'}</span></span></button>`;
    }).join('')).join('');
}

// ---------- MCP 客户端 ----------
const SNIP_JSON = ['"mcpServers": {', '  "leafstarter": { "command": "leafstarter", "args": ["--mcp"] }', '}'];
export const MCP_CLIENTS = [
    ['code', 'Claude Code', '~/.claude.json', true, SNIP_JSON],
    ['desktop', 'Claude 桌面版', '%APPDATA%\\Claude\\claude_desktop_config.json', true, SNIP_JSON],
    ['codex', 'Codex', '~/.codex/config.toml', true, ['[mcp_servers.leafstarter]', 'command = "leafstarter"', 'args = ["--mcp"]']],
    ['cursor', 'Cursor', '~/.cursor/mcp.json', false, SNIP_JSON],
    ['trae', 'Trae', '', false, SNIP_JSON],
];
function mcpPop(id) {
    const c = MCP_CLIENTS.find((x) => x[0] === id);
    return `<div class="pop-h">写进 ${esc(c[1])} 的配置？</div>
        <p class="pop-note big">改的是 <span class="mono">${esc(c[2])}</span>，只加下面这几行，别的不动；改之前备份原文件。</p>
        <pre class="diff mono">${c[4].map((l) => `<span class="add">+ ${esc(l)}</span>`).join('\n')}</pre>
        <div class="pop-act"><button type="button" class="btn sm pri" data-act="mcp-write-yes" data-v="${c[0]}">写入</button><button type="button" class="btn sm ghost" data-act="pop-close">先不</button></div>`;
}

// ---------- 功能包 ----------
export const PACKS = {
    mp: { name: '联机', inside: 'P2P 组网程序 EasyTier', size: '约 14 MB', ms: 1600, perms: ['联网', '开端口', '后台运行'], from: '约 14 MB · LeafStarter 的下载源', stages: [[0, '下载'], [70, '校验签名和哈希'], [92, '装进 ext/']] },
    world: { name: '世界工具', inside: '地图渲染、按种子推算结构（cubiomes）、方块颜色表', size: '约 6 MB', ms: 1300, perms: ['读存档'], from: '约 6 MB · LeafStarter 的下载源', stages: [[0, '下载'], [70, '校验签名和哈希'], [92, '装进 ext/']] },
    tr: { name: '离线翻译', inside: '开源翻译模型，在本机跑，不联网', size: '约 180 MB', ms: 3400, perms: ['只在本机'], from: '约 180 MB · 从魔搭社区下载，不走 LeafStarter 的服务器', stages: [[0, '从魔搭社区下载模型'], [82, '按清单校验 SHA-256'], [95, '装好']], ondemand: true },
};

function editionsPop() {
    return `<div class="pop-h">LeafStarter 的几种版本</div>
        <div class="pop-row">${icon('package', 15)}<span class="tx"><span class="nm">安装版（默认推荐）</span><span class="sb wrap">开始菜单、自动更新、注册 leafstarter://（点邀请链接直接打开）。起步是精简的，功能包按需装</span></span><span class="chip ok">你在用</span></div>
        <div class="pop-row">${icon('file', 15)}<span class="tx"><span class="nm">便携版 · 精简</span><span class="sb wrap">一个 exe，数据放在旁边的 data/</span></span></div>
        <div class="pop-row">${icon('folder', 15)}<span class="tx"><span class="nm">便携版 · 全功能</span><span class="sb wrap">一个 zip，带联机和世界工具；不带离线翻译模型（用到再下）</span></span></div>
        <p class="pop-note">下载走 R2 和蓝奏云两个源。不做「单个 exe 自解压出全部功能」：自解压程序最容易被杀毒软件误报。</p>`;
}
