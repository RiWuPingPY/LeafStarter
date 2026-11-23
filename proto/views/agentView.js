// AI 助手面板：一条时间线。读东西直接读；动手之前先问；能撤回的先建快照
import { esc } from '../ui.js';
import { icon } from '../icons.js';

export const LV = { read: ['只读', 'eye'], rev: ['可撤回', 'undo'], ext: ['影响外部', 'globe'], irr: ['不可撤回', 'alert'] };
export const lvChip = (k) => `<span class="lv ${k}">${icon(LV[k][1], 12)}${LV[k][0]}</span>`;

// 对话用的那个源：名字 · 模型
export const modelChip = (c) => {
    const s = c.aiFor('chat');
    return s ? `${s.name} · ${s.model || '没填模型'}` : '';
};
const CAP = { '2w': '2 万', '10w': '10 万', none: '不限' };

export function agentPanel(c, A) {
    const ready = c.aiReady();
    return `<section class="drawer agent" role="dialog" aria-label="AI 助手">
        <header class="dr-head"><h2>${icon('sparkles', 18)}AI 助手</h2>${ready ? `<span class="chip acc ag-model">${esc(modelChip(c))}</span>` : ''}<span class="sp"></span>
            <button type="button" class="iconbtn" data-act="ai-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
        <div class="ag-body" id="agBody">${ready ? `<div class="ag-list" id="agList">${A.items.length ? A.items.map(agItem).join('') : welcome()}</div>` : empty()}</div>
        <footer class="ag-foot" id="agFoot">${agFoot(c, A)}</footer>
    </section>`;
}

function empty() {
    return `<div class="ag-empty">
        <span class="ag-hero">${icon('sparkles', 26)}</span>
        <h3>还没填 AI 的 Key</h3>
        <p>不填也没关系：常见崩溃靠规则、Mod 冲突靠二分、开服靠固定剧本，都不用 AI。规则认不出来的时候，AI 才派上用场。</p>
        <div class="actions"><button type="button" class="btn pri" data-act="ai-demo">${icon('sparkles')}用示例 Key 试试（原型）</button><button type="button" class="btn" data-act="ai-settings">去设置填 Key</button></div>
        <ul class="ag-points">
            <li>${icon('cpu', 14)}<span>Claude、DeepSeek、通义千问、Kimi、智谱，或者本机的 Ollama、llama.cpp、LM Studio，用你自己的 Key；可以配好几个，按用途分</span></li>
            <li>${icon('shield', 14)}<span>发给模型之前先打码：用户名路径、令牌、IP、密码；发出去的内容可以先看</span></li>
            <li>${icon('undo', 14)}<span>会改东西的操作先问你；能撤回的先建快照</span></li>
        </ul>
    </div>`;
}

function welcome() {
    return `<div class="ag-welcome">
        <p>我能读日志、查 Mod、在服务器上排查问题。读东西直接读，动手之前都会先问你。</p>
        <div class="ag-sugs">
            <button type="button" data-act="ag-sug" data-v="crash">「空岛生存」为什么崩了</button>
            <button type="button" data-act="ag-sug" data-v="pack">配一个科技向整合包</button>
            <button type="button" data-act="ag-sug" data-v="perf">让「生存一服」更流畅</button>
        </div>
    </div>`;
}

export function agFoot(c, A) {
    const ready = c.aiReady();
    return `<div class="ag-input">
            <textarea id="agInput" rows="2" placeholder="${ready ? '问点什么，比如：为什么进服务器卡' : '先填 Key'}"${ready ? '' : ' disabled'}></textarea>
            <button type="button" class="send" id="agSend" data-act="ag-send" aria-label="发送"${ready && !A.busy ? '' : ' disabled'}>${icon('send', 16)}</button>
        </div>
        <p class="ag-meta" id="agMeta">${agMeta(c, A)}</p>`;
}

export const agMeta = (c, A) => `${icon('shield', 12)}<span>发出前打码${c.S.aiPreview ? '，可预览' : ''} · 本次 ${(A.tokens / 1000).toFixed(1)}k token · 上限 ${CAP[c.S.aiCap] || '10 万'}</span>`;

const mark = (s) => esc(s).replace(/«([^»]*)»/g, '<mark>$1</mark>');

export function agItem(it) {
    switch (it.kind) {
        case 'user':
            return `<div class="ag-msg me" id="${it.id}">${esc(it.text)}</div>`;
        case 'say':
            return `<div class="ag-msg ai" id="${it.id}">${it.html}</div>`;
        case 'think':
            return `<div class="ag-think" id="${it.id}"><i></i><i></i><i></i><span>正在想</span></div>`;
        case 'note':
            return `<p class="ag-note" id="${it.id}">${esc(it.text)}</p>`;
        case 'tool':
            return `<div class="ag-tool ${it.state}" id="${it.id}">
                <span class="sic">${it.state === 'done' ? icon('check', 12) : it.state === 'fail' ? icon('x', 12) : ''}</span>
                <span class="tx"><span class="nm">${esc(it.title)}</span><span class="sb">${esc(it.detail)}</span></span>${lvChip(it.level)}
                ${it.preview && it.state === 'done' ? `<details class="ag-prev"><summary>${icon('eye', 12)}发给模型的内容（已打码）</summary><pre>${mark(it.preview)}</pre></details>` : ''}
            </div>`;
        case 'confirm': {
            const res = { yes: '已允许', all: '已允许（这次会话里同级的都允许）', no: '没允许' }[it.state];
            return `<div class="ag-confirm lv-${it.level}${it.state === 'wait' ? ' wait' : ''}" id="${it.id}">
                <div class="h">${lvChip(it.level)}<b>${esc(it.title)}</b></div>
                <p>${esc(it.detail)}</p>
                ${it.cmd ? `<pre class="cmdbox">${it.cmd.map((l) => `<span class="p">$</span> ${esc(l)}`).join('\n')}</pre>` : ''}
                <div class="actions">${it.state === 'wait'
                    ? `<button type="button" class="btn sm pri" data-act="ag-yes" data-id="${it.id}">${it.level === 'ext' ? '执行' : '允许'}</button>${it.level === 'rev' ? `<button type="button" class="btn sm" data-act="ag-all" data-id="${it.id}">这次会话都允许</button>` : ''}<button type="button" class="btn sm ghost" data-act="ag-no" data-id="${it.id}">不要</button>`
                    : `<span class="ag-res ${it.state}">${it.state === 'no' ? icon('x', 13) : icon('check', 13)}${res}</span>`}</div>
            </div>`;
        }
    }
    return '';
}
