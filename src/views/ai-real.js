// 真程序设置页里的 AI 源和 MCP（M9）
import { esc, ago } from '../ui.js';
import { icon } from '../icons.js';
import { lvChip } from './agentView.js';
import { AI_PRESETS } from '../agent-real.js';

const seg = (act, list, cur) => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="${act}" data-v="${v}" class="${String(cur) === v ? 'on' : ''}" aria-pressed="${String(cur) === v}">${t}</button>`).join('')}</div>`;
const KIND = { anthropic: 'Anthropic', openai: 'OpenAI 兼容', ollama: 'Ollama' };
const EFFORT = [['', '默认'], ['low', '低'], ['medium', '中'], ['high', '高'], ['xhigh', '很高'], ['max', '最高']];

export function aiRealPanel(c) {
    const ai = c.R.ai;
    if (!ai) return '';
    const E = c.R.aiUi || {};
    const d = E.draft;
    const rows = ai.sources.map((s) => {
        const t = E.test?.get(s.id);
        const p = AI_PRESETS[s.preset] || {};
        const st = t?.state === 'run' ? '<span class="chip acc">测试中…</span>'
            : t?.state === 'ok' ? `<span class="chip ok">${icon('check', 12)}${t.ms} ms</span>${t.tools ? '<span class="chip ok">能调工具</span>' : '<span class="chip warn">没调工具，助手可能用不了</span>'}`
            : t?.state === 'fail' ? `<span class="chip bad" title="${esc(t.msg)}">连不上</span>` : '';
        const need = !s.hasKey && !p.local && s.kind !== 'ollama';
        const isChat = ai.uses.chat === s.id;
        return `<div class="row ai-src${d?.id === s.id ? ' on' : ''}">
            <span class="tile" style="--h:${p.local ? 200 : 265}">${icon(p.local ? 'monitor' : 'cloud', 15)}</span>
            <span class="tx"><span class="nm">${esc(s.name)}</span><span class="sb">${KIND[s.kind] || s.kind} · <span class="mono">${esc(s.model || '默认模型')}</span></span></span>
            <span class="tags">${isChat ? '<span class="chip acc">对话用这个</span>' : ''}${need ? '<span class="chip warn">没填 Key</span>' : ''}${st}</span>
            <button type="button" class="btn sm" data-act="ai-test" data-id="${esc(s.id)}"${t?.state === 'run' ? ' disabled' : ''}>${icon('zap', 13)}测试</button>
            <button type="button" class="btn sm ghost" data-act="ai-edit" data-id="${esc(s.id)}">改</button>
            <button type="button" class="btn sm ghost danger" data-act="ai-del" data-id="${esc(s.id)}" aria-label="删掉">${icon('trash', 13)}</button>
        </div>${t?.state === 'fail' ? `<p class="callout warn">${icon('alert', 14)}<span>${esc(t.msg)}</span></p>` : ''}${d?.id === s.id ? editor(d) : ''}`;
    }).join('');
    const caps = [['0', '不限'], ['20000', '2 万'], ['100000', '10 万'], ['500000', '50 万']];
    return `<section class="panel" id="aiPanel"><h2 class="ph">AI 助手</h2>
        <p class="note">用你自己的 Key，加密存在本机</p>
        <div class="rows">${rows || '<div class="row empty">还没有 AI 源</div>'}</div>
        ${d && !d.id ? editor(d) : ''}
        <div class="actions"><button type="button" class="btn" data-act="ai-add">${icon('plus')}加一个 AI 源</button></div>
        ${ai.sources.length > 1 ? `<div class="kv"><div class="k"><b>对话和排查用哪个</b></div>
            <div class="v"><select data-aiuse="chat" aria-label="对话用哪个源">${ai.sources.map((s) => `<option value="${esc(s.id)}"${ai.uses.chat === s.id ? ' selected' : ''}>${esc(s.name)}</option>`).join('')}</select></div></div>` : ''}
        <div class="kv"><div class="k"><b>每次会话最多用</b></div>
            <div class="v">${seg('ai-cap', caps, String(ai.cap || 0))}</div></div>
        <div class="perm">
            <div class="pr">${lvChip('read')}<span>列实例、读日志和崩溃报告、看 Mod、搜 Modrinth</span><b>直接做</b></div>
            <div class="pr">${lvChip('rev')}<span>开关 Mod、改内存和 Java、装 Mod</span><b>先问你，可选「这次会话都允许」；每一步都能撤回</b></div>
            <div class="pr">${lvChip('ext')}<span>启动、结束游戏</span><b>每次都问</b></div>
            <div class="pr"><span class="lv no">${icon('x', 12)}不开放</span><span>读 Key 和令牌、删世界、付款、改密码</span><b>工具里根本没有</b></div>
        </div>
    </section>`;
}

function editor(d) {
    const p = AI_PRESETS[d.preset] || {};
    const f = (k, label, val, ph = '', cls = 'wide') => `<label class="sf"><span>${label}</span><input type="text" class="${cls}" data-aif="${k}" value="${esc(val ?? '')}" placeholder="${esc(ph)}" autocomplete="off" spellcheck="false"></label>`;
    const extra = d.extraText ?? (d.extra ? JSON.stringify(d.extra) : '');
    return `<div class="ai-edit" id="aiEdit">
        <div class="sfs">
            ${f('name', '名字', d.name)}
            ${f('base', '地址', d.base, d.kind === 'anthropic' ? 'https://api.anthropic.com' : 'https://…/v1', 'wide mono')}
            ${p.local || d.kind === 'ollama' ? '' : `<label class="sf"><span>API Key ${d.hasKey ? '<span class="chip ok">已保存</span>' : ''}</span><input type="password" class="wide" data-aif="key" placeholder="${d.hasKey ? '不改就留空' : '粘贴 Key'}" autocomplete="off"></label>`}
            ${f('model', '模型', d.model, d.kind === 'anthropic' ? 'claude-opus-5-5' : '按服务商给的模型名填', 'wide mono')}
        </div>
        ${p.models ? `<div class="mchips">${p.models.map((m) => `<button type="button" class="chip${d.model === m ? ' acc' : ''}" data-act="ai-model" data-v="${m}">${m}</button>`).join('')}</div>` : ''}
        <details class="adv"><summary>${icon('sliders', 13)}高级参数</summary>
            ${d.kind === 'anthropic' ? `<div class="sf"><span>推理强度</span>${seg('ai-effort', EFFORT, d.effort || '')}</div>` : ''}
            <div class="sfs">
                ${f('maxTokens', '最大输出（token）', d.maxTokens ?? '', '留空用默认', 'wide mono')}
                ${f('extra', '额外参数（JSON，原样透传）', extra, '{"temperature": 0.3}', 'wide mono')}
            </div>
        </details>
        <div class="actions"><button type="button" class="btn sm pri" data-act="ai-save">${d.id ? '保存' : '加上'}</button><button type="button" class="btn sm ghost" data-act="ai-cancel">取消</button></div>
    </div>`;
}

export function mcpRealPanel(c) {
    const ai = c.R.ai;
    if (!ai) return '';
    const E = c.R.aiUi || {};
    const lv = ai.mcpLevels || [];
    const exe = ai.exe || 'leafstarter.exe';
    const json = JSON.stringify({ mcpServers: { leafstarter: { command: exe, args: ['--mcp'] } } }, null, 2);
    const cc = `claude mcp add leafstarter -- "${exe}" --mcp`;
    const pv = E.mcpPreview;
    const log = c.R.mcpLog || [];
    return `<section class="panel" id="mcpPanel"><h2 class="ph">MCP</h2>
        <div class="kv"><div class="k"><b>让别的 AI 客户端操作 LeafStarter</b><small>Claude、Cursor 等支持 MCP 的客户端</small></div>
            <div class="v"><input type="checkbox" class="switch" id="mcpOn"${ai.mcp ? ' checked' : ''} aria-label="开放 MCP"></div></div>
        ${ai.mcp ? `
        <div class="kv"><div class="k"><b>开放哪些操作</b><small>${ai.mcpPort ? `127.0.0.1:${ai.mcpPort}` : '没开起来，关掉再打开试试'}</small></div>
            <div class="v mcp-lv"><label>${lvChip('read')}<input type="checkbox" data-mcplv="read"${lv.includes('read') ? ' checked' : ''}></label><label>${lvChip('rev')}<input type="checkbox" data-mcplv="rev"${lv.includes('rev') ? ' checked' : ''}></label></div></div>
        <div class="kv stack"><div class="k"><b>接入</b></div>
            <div class="actions"><button type="button" class="btn sm" data-act="mcp-preview" data-v="claude-desktop">写进 Claude 桌面版</button><button type="button" class="btn sm" data-act="mcp-preview" data-v="cursor">写进 Cursor</button></div>
            ${pv ? `<div class="ai-edit"><p class="note">要写的文件：<span class="mono">${esc(pv.path)}</span></p><pre class="cmdbox mono">${esc(pv.content)}</pre>
                <div class="actions"><button type="button" class="btn sm pri" data-act="mcp-write">写入</button><button type="button" class="btn sm ghost" data-act="mcp-cancel">先不</button></div></div>` : ''}
            <p class="note">Claude Code：</p><pre class="cmdbox mono" id="mcpCc">${esc(cc)}</pre>
            <p class="note">其他客户端（JSON 配置）：</p><pre class="cmdbox mono" id="mcpJson">${esc(json)}</pre>
            <div class="actions"><button type="button" class="btn sm ghost" data-act="mcp-copy" data-v="#mcpCc">复制 Claude Code 命令</button><button type="button" class="btn sm ghost" data-act="mcp-copy" data-v="#mcpJson">复制 JSON</button></div></div>
        ${log.length ? `<div class="kv stack"><div class="k"><b>最近的调用</b></div>
            <div class="rows">${log.map((x) => `<div class="row"><span class="tx"><span class="nm">${esc(x.title)}</span><span class="sb mono">${esc(x.tool)} ${esc(JSON.stringify(x.input || {}))}</span></span><span class="tags"><span class="chip">${ago(x.at)}</span></span></div>`).join('')}</div></div>` : ''}` : ''}
    </section>`;
}
