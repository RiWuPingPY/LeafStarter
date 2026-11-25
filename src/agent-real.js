// 真程序里的 AI 助手（M9）：循环跑在本机后端（src-tauri/src/agent），这里只画时间线、回答确认、撤回。
// 后端每一步发一个 agent-event：think / say / tool / confirm / note / error / done，外部 MCP 客户端调用是 mcp。
// 另外管设置页里的 AI 源、用途、上限、MCP。
import { $, toast, esc, ask, askText } from './ui.js';
import { icon } from './icons.js';
import { agentPanel, agItem, agMeta } from './views/agentView.js';
import { invoke, listen, R } from './real.js';

// 预设：地址、模型、接口格式填好
export const AI_PRESETS = {
    claude: { name: 'Claude', kind: 'anthropic', base: 'https://api.anthropic.com', model: 'claude-opus-5-5', models: ['claude-opus-5-5', 'claude-sonnet-5-5', 'claude-haiku-5-5'], effort: true },
    deepseek: { name: 'DeepSeek', kind: 'openai', base: 'https://api.deepseek.com', model: 'deepseek-chat' },
    qwen: { name: '通义千问', kind: 'openai', base: 'https://dashscope.aliyuncs.com/compatible-mode/v1', model: 'qwen-plus' },
    kimi: { name: 'Kimi', kind: 'openai', base: 'https://api.moonshot.cn/v1', model: '' },
    glm: { name: '智谱', kind: 'openai', base: 'https://open.bigmodel.cn/api/paas/v4', model: '' },
    custom: { name: 'OpenAI 兼容接口', kind: 'openai', base: '', model: '' },
    ollama: { name: '本机 Ollama', kind: 'ollama', base: 'http://127.0.0.1:11434', model: 'qwen2.5:7b', local: true },
    llama: { name: '本机 llama.cpp / LM Studio', kind: 'openai', base: 'http://127.0.0.1:8080/v1', model: '', local: true },
};

// 模型说的话：转义后认几样简单格式（段落、列表、**粗体**、`代码`）
export function sayHtml(text) {
    const inline = (s) => esc(s).replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>').replace(/`([^`]+)`/g, '<code>$1</code>');
    return text.trim().split(/\n{2,}/).map((block) => {
        const lines = block.split('\n');
        if (lines.every((l) => /^\s*([-*]|\d+[.)])\s+/.test(l))) return `<ul>${lines.map((l) => `<li>${inline(l.replace(/^\s*([-*]|\d+[.)])\s+/, ''))}</li>`).join('')}</ul>`;
        return `<p>${lines.map(inline).join('<br>')}</p>`;
    }).join('');
}

// 确认卡片上那一行：参数说人话
function inputText(input) {
    if (!input || typeof input !== 'object') return '';
    return Object.entries(input).map(([k, v]) => {
        const val = typeof v === 'string' ? (k === 'instance' ? v.split('/').pop() : v) : JSON.stringify(v);
        return `${k}: ${val}`;
    }).join(' · ');
}

export function createAgentReal(api) {
    const A = { open: false, items: [], busy: false, tokens: 0, seq: 0, session: 'ui' + Date.now(), think: null, steps: new Map(), mcpLog: [] };
    const motion = () => (document.documentElement.dataset.motion === 'reduced' ? 'auto' : 'smooth');

    // ---------- 面板 ----------
    function render() {
        const box = $('#agent');
        if (!box) return;
        box.innerHTML = agentPanel(api.ctx(), A);
        const b = $('#agBody');
        if (b) b.scrollTop = b.scrollHeight;
        foot();
    }
    function markBtn() {
        const b = $('#aiBtn');
        if (!b) return;
        b.classList.toggle('on', A.open);
        b.setAttribute('aria-expanded', A.open);
    }
    function open(text) {
        api.closeDrawer();
        api.closePop(false);
        if (!A.open) {
            A.open = true;
            render();
            markBtn();
        }
        if (text) send(text);
        else setTimeout(() => $('#agInput')?.focus(), 60);
    }
    function close() {
        if (!A.open) return false;
        A.open = false;
        markBtn();
        const el = $('#agent .drawer');
        if (!el) return true;
        el.classList.add('out');
        setTimeout(() => {
            if (!A.open) $('#agent').innerHTML = '';
        }, 170);
        return true;
    }
    function foot() {
        const m = $('#agMeta');
        if (m) m.innerHTML = agMeta(api.ctx(), A);
        const s = $('#agSend');
        if (s) s.disabled = A.busy || !api.aiReady();
    }
    function scrollEnd() {
        const b = $('#agBody');
        if (b) b.scrollTo({ top: b.scrollHeight, behavior: motion() });
    }

    // ---------- 时间线 ----------
    function add(it) {
        it.id = 'ag' + ++A.seq;
        A.items.push(it);
        const list = $('#agList');
        if (list) {
            if (A.items.length === 1) list.innerHTML = '';
            list.insertAdjacentHTML('beforeend', agItem(it));
            list.lastElementChild?.classList.add('in');
            scrollEnd();
        }
        return it;
    }
    function drop(it) {
        A.items = A.items.filter((x) => x !== it);
        $('#' + it.id)?.remove();
    }
    function update(it) {
        const el = $('#' + it.id);
        if (el) el.outerHTML = agItem(it);
    }
    function stopThink() {
        if (A.think) drop(A.think);
        A.think = null;
    }

    // ---------- 后端发来的每一步 ----------
    function onEvent(p) {
        if (p.kind === 'mcp') {
            A.mcpLog.unshift({ at: Date.now(), tool: p.tool, title: p.title, input: p.input });
            A.mcpLog.length = Math.min(A.mcpLog.length, 20);
            if (api.S.page === 'settings') api.paintView();
            return;
        }
        // 外部客户端要做「影响外部」的事：在这里弹确认
        if (p.kind === 'confirm' && p.session === 'mcp') {
            if (!A.open) open();
            add({ kind: 'confirm', state: 'wait', cid: p.id, level: p.level, title: p.title, detail: inputText(p.input), from: '外部 AI 客户端（MCP）' });
            toast('外部 AI 客户端要做一件事，在 AI 助手面板里等你确认', { ms: 6000 });
            return;
        }
        if (p.session !== A.session) return;
        if (p.kind !== 'think') stopThink();
        switch (p.kind) {
            case 'think':
                stopThink();
                A.think = add({ kind: 'think' });
                break;
            case 'say':
                add({ kind: 'say', html: sayHtml(p.text) });
                if (p.tokens) A.tokens = p.tokens;
                break;
            case 'tool': {
                let it = A.steps.get(p.id);
                const o = { level: p.level, title: p.title, detail: p.detail || '', state: p.state, preview: p.preview, undo: !!p.undo, step: p.id };
                if (it) {
                    Object.assign(it, o);
                    update(it);
                } else {
                    it = add({ kind: 'tool', ...o });
                    A.steps.set(p.id, it);
                }
                break;
            }
            case 'confirm':
                add({ kind: 'confirm', state: 'wait', cid: p.id, level: p.level, title: p.title, detail: inputText(p.input) });
                break;
            case 'note':
                add({ kind: 'note', text: p.text });
                break;
            case 'error':
                add({ kind: 'note', text: p.text, err: true });
                break;
            case 'done':
                A.busy = false;
                if (p.tokens != null) A.tokens = p.tokens;
                break;
        }
        foot();
    }

    function send(text) {
        const ta = $('#agInput');
        text = text ?? ta?.value.trim();
        if (!text) return;
        if (A.busy) return toast('上一句还没回完');
        if (!api.aiReady()) return toast('先到设置里加一个 AI 源');
        if (ta && text === ta.value.trim()) ta.value = '';
        add({ kind: 'user', text });
        // 当前选中的实例告诉模型，免得它先去猜
        const i = api.cur();
        const ctxLine = i?.real ? `\n\n（当前选中的实例：「${i.name}」，id 是 ${i.id}）` : '';
        A.busy = true;
        foot();
        invoke('agent_send', { session: A.session, text: text + ctxLine }).catch((e) => {
            A.busy = false;
            add({ kind: 'note', text: String(e), err: true });
            foot();
        });
    }

    function answer(id, ans) {
        const it = A.items.find((x) => x.id === id);
        if (!it || it.state !== 'wait') return;
        it.state = ans;
        update(it);
        invoke('agent_answer', { id: it.cid, answer: ans });
    }

    async function undo(id) {
        const it = A.items.find((x) => x.id === id);
        if (!it || it.undone) return;
        const sess = A.session;
        try {
            const out = await invoke('agent_undo', { session: sess, step: it.step });
            it.undone = true;
            update(it);
            toast(`撤回了：${out}`);
            // 改了 Mod、内存这些：实例要重新读
            const i = api.cur();
            if (i) {
                i.loaded = false;
                api.refresh?.(i.id, true);
            }
        } catch (e) {
            toast(String(e));
        }
    }

    function reset() {
        if (A.busy) return toast('等这一句回完再开新会话');
        invoke('agent_reset', { session: A.session });
        A.session = 'ui' + Date.now();
        A.items = [];
        A.tokens = 0;
        A.steps.clear();
        render();
    }

    listen('agent-event', (e) => onEvent(e.payload));

    const SUG = {
        crash: (n) => `「${n}」为什么崩了？看看日志和崩溃报告，说说原因和改法`,
        mods: (n) => `看看「${n}」的 Mod 有没有缺前置或者版本对不上的`,
        perf: (n) => `「${n}」的内存和 Java 设置合适吗？`,
    };

    return {
        A,
        open: () => open(),
        // 别的地方（开服失败）带着一段话打开助手
        ask: (text) => (api.aiReady() ? open(text) : (open(), toast('先到设置里加一个 AI 源'))),
        close,
        rerender: () => A.open && render(),
        send: () => send(),
        actions: {
            'ai-open': () => (A.open ? close() : open()),
            'ai-close': () => close(),
            'ai-crash': () => {
                const i = api.cur();
                if (!api.aiReady()) {
                    open();
                    return;
                }
                open(SUG.crash(i?.name || '这个实例'));
            },
            'ai-settings': () => {
                close();
                api.goPage('settings');
                requestAnimationFrame(() => $('#aiPanel')?.scrollIntoView({ block: 'start', behavior: motion() }));
            },
            'ag-sug': (el) => send(SUG[el.dataset.v]?.(api.cur()?.name || '这个实例')),
            'ag-send': () => send(),
            'ag-yes': (el) => answer(el.dataset.id, 'yes'),
            'ag-all': (el) => answer(el.dataset.id, 'all'),
            'ag-no': (el) => answer(el.dataset.id, 'no'),
            'ag-undo': (el) => undo(el.dataset.id),
            'ag-reset': () => reset(),
        },
    };
}

// ---------- 设置页：AI 源、MCP ----------
export function createAiSettings(api) {
    // 正在编辑的源（草稿）：id 为空就是新加的
    const E = { draft: null, test: new Map(), mcpPreview: null };
    const paint = () => api.S.page === 'settings' && api.paintView();
    const apply = (v) => {
        R.ai = v;
        paint();
        api.agent?.rerender();
    };

    function edit(id, preset) {
        if (id) {
            const s = R.ai.sources.find((x) => x.id === id);
            if (!s) return;
            E.draft = { ...s, key: '', keyTouched: false };
        } else {
            const p = AI_PRESETS[preset];
            const n = R.ai.sources.filter((x) => x.preset === preset).length;
            E.draft = { id: '', preset, name: p.name + (n ? ` ${n + 1}` : ''), kind: p.kind, base: p.base, model: p.model, effort: '', maxTokens: null, extra: null, key: '', keyTouched: false, hasKey: false };
        }
        paint();
        requestAnimationFrame(() => $('#aiEdit')?.scrollIntoView({ block: 'nearest', behavior: 'smooth' }));
    }

    async function save() {
        const d = E.draft;
        if (!d) return;
        let extra = null;
        const ex = (d.extraText ?? (d.extra ? JSON.stringify(d.extra) : '')).trim();
        if (ex) {
            try {
                extra = JSON.parse(ex);
                if (!extra || typeof extra !== 'object' || Array.isArray(extra)) throw new Error();
            } catch {
                return toast('额外参数要是一个 JSON 对象，比如 {"temperature": 0.3}');
            }
        }
        if (d.kind !== 'anthropic' && !d.base.trim()) return toast('填一下地址');
        if (!d.model.trim() && d.kind !== 'anthropic') return toast('填一下模型名');
        const source = { id: d.id, preset: d.preset, name: d.name.trim() || AI_PRESETS[d.preset]?.name || 'AI', kind: d.kind, base: d.base.trim(), model: d.model.trim(), effort: d.effort || '', maxTokens: d.maxTokens ? Number(d.maxTokens) : null, extra };
        try {
            const v = await invoke('ai_save_source', { source, key: d.keyTouched ? d.key : null });
            E.draft = null;
            apply(v);
            toast(d.id ? '存好了' : '加好了，可以「测试连接」看看');
        } catch (e) {
            toast(String(e));
        }
    }

    async function test(id) {
        E.test.set(id, { state: 'run' });
        paint();
        try {
            const r = await invoke('ai_test', { id });
            E.test.set(id, { state: 'ok', ...r });
        } catch (e) {
            E.test.set(id, { state: 'fail', msg: String(e) });
        }
        paint();
    }

    async function del(id) {
        const s = R.ai.sources.find((x) => x.id === id);
        if (!s || !(await ask(`删掉 AI 源「${s.name}」？保存的 Key 也一起删掉。`))) return;
        apply(await invoke('ai_delete_source', { id }));
        if (E.draft?.id === id) E.draft = null;
    }

    const set = (patch) => invoke('ai_set', { patch }).then(apply, (e) => {
        toast(String(e));
        paint();
    });

    async function mcpPreview(client) {
        try {
            E.mcpPreview = { client, ...(await invoke('mcp_install', { client, write: false })) };
        } catch (e) {
            E.mcpPreview = null;
            toast(String(e));
        }
        paint();
    }
    async function mcpWrite() {
        const p = E.mcpPreview;
        if (!p) return;
        try {
            await invoke('mcp_install', { client: p.client, write: true });
            toast(`写好了（原来的文件备份成 .leafstarter-bak）。重开 ${p.client === 'cursor' ? 'Cursor' : 'Claude 桌面版'} 就能用`, { ms: 7000 });
            E.mcpPreview = null;
            paint();
        } catch (e) {
            toast(String(e));
        }
    }

    return {
        E,
        actions: {
            'ai-add': (el) => api.openPop(el, '<div class="pop-h">加一个 AI 源</div>' + [['云端', ['claude', 'deepseek', 'qwen', 'kimi', 'glm', 'custom']], ['本机', ['ollama', 'llama']]].map(([g, ks]) => `<div class="pop-sub">${g}</div>` + ks.map((k) => `<button type="button" class="pop-item" data-act="ai-new" data-v="${k}">${icon(g === '本机' ? 'monitor' : 'cloud', 16)}<span class="tx"><span class="nm">${esc(AI_PRESETS[k].name)}</span></span></button>`).join('')).join(''), 'below'),
            'ai-new': (el) => {
                api.closePop(false);
                edit(null, el.dataset.v);
            },
            'ai-edit': (el) => (E.draft?.id === el.dataset.id ? ((E.draft = null), paint()) : edit(el.dataset.id)),
            'ai-cancel': () => {
                E.draft = null;
                paint();
            },
            'ai-save': () => save(),
            'ai-test': (el) => test(el.dataset.id),
            'ai-del': (el) => del(el.dataset.id),
            'ai-model': (el) => {
                if (!E.draft) return;
                E.draft.model = el.dataset.v;
                paint();
            },
            'ai-effort': (el) => {
                if (!E.draft) return;
                E.draft.effort = el.dataset.v;
                paint();
            },
            'ai-cap': (el) => set({ cap: Number(el.dataset.v) }),
            'mcp-preview': (el) => mcpPreview(el.dataset.v),
            'mcp-write': () => mcpWrite(),
            'mcp-cancel': () => {
                E.mcpPreview = null;
                paint();
            },
            'mcp-copy': (el) => {
                const t = $(el.dataset.v)?.textContent || '';
                navigator.clipboard.writeText(t).then(() => toast('复制好了'), () => toast('复制不了，手动选中复制吧'));
            },
        },
        // 输入框：草稿字段
        input(t) {
            const f = t.dataset.aif;
            if (!f || !E.draft) return false;
            if (f === 'key') {
                E.draft.key = t.value;
                E.draft.keyTouched = true;
            } else if (f === 'extra') E.draft.extraText = t.value;
            else E.draft[f] = t.value;
            return true;
        },
        change(t) {
            if (t.dataset.aiuse) {
                set({ uses: { [t.dataset.aiuse]: t.value } });
                return true;
            }
            if (t.id === 'mcpOn') {
                set({ mcp: t.checked });
                return true;
            }
            if (t.dataset.mcplv) {
                const lv = [...document.querySelectorAll('[data-mcplv]')].filter((x) => x.checked).map((x) => x.dataset.mcplv);
                set({ mcpLevels: lv });
                return true;
            }
            return !!t.dataset.aif;
        },
    };
}
