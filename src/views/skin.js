// 皮肤工作台页面。只出 HTML，画布和 3D 预览在 skin.js 里接
import { esc, head } from '../ui.js';
import { icon } from '../icons.js';
import { rigHTML, genHTML } from '../skin.js';

const seg = (act, list, cur) => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="${act}" data-v="${v}" class="${String(cur) === v ? 'on' : ''}" aria-pressed="${String(cur) === v}">${t}</button>`).join('')}</div>`;
const SW = ['#1F1D26', '#E6C3A5', '#A8754E', '#F4F4F4', '#C0392B', '#E67E22', '#E9C12E', '#2E8B57', '#2E6FD8', '#22305A', '#6D4ED6', '#F08DB6', '#7A4E2D', '#7A7A80'];
const EX = ['穿蓝色卫衣、戴黑框眼镜的程序员', '戴红色围巾和毛线帽的学生', '金色长发、穿白色裙子', '穿西装的上班族', '厨师', '忍者'];

export function skinView(c) {
    const K = c.S.skin;
    const a = c.ACCOUNTS.find((x) => x.id === c.S.account) || c.ACCOUNTS[0];
    const tools = [['brush', `${icon('wrench', 13)}画笔`], ['fill', `${icon('package', 13)}填充`], ['pick', `${icon('eye', 13)}取色`], ['erase', `${icon('x', 13)}橡皮`]];
    const routes = [['draw', '描述 → 程序画'], ['img', '生图模型画 → 转皮肤'], ['comfy', '本机 ComfyUI']];
    const routeNote = {
        draw: '默认，最稳：文字模型把描述拆成每个部位的颜色和配饰，再由程序按皮肤布局画出来。任何文字模型都能用，很便宜。',
        img: '生图模型画角色的正面和背面，再像素化、压缩调色板、贴到对应部位。效果看运气，适合当草稿。',
        comfy: '接本机的 ComfyUI 或 SD WebUI，用社区专门训练过的皮肤模型。高级选项。',
    }[K.route];
    return `<div class="page skinpage" id="page">
        <div class="page-h">
            <button type="button" class="btn ghost sm" data-act="skin-back">${icon('chevron', 14)}返回</button>
            <h1>皮肤工作台</h1>
            <span class="chip">${head(a.face, 16)}${esc(a.name)} · ${esc(a.label)}</span>
            <span class="sp"></span>
            <label class="btn" for="skinFile">${icon('upload')}导入 PNG</label><input type="file" id="skinFile" accept="image/png" hidden>
            ${a.id === 'msa' ? `<button type="button" class="btn" data-act="skin-import">${icon('download')}从正版档案导入</button>` : ''}
            <button type="button" class="btn pri" data-act="skin-up">${icon('check')}换上</button>
        </div>
        <div class="skin-grid">
            <section class="card sk-edit">
                <div class="toolbar">${seg('sk-tool', tools, K.tool)}${seg('sk-layer', [['0', '身体层'], ['1', '外套层']], String(K.layer))}
                    <label class="tick"><input type="checkbox" id="skMirror"${K.mirror ? ' checked' : ''}>左右镜像</label>
                    <span class="sp"></span><button type="button" class="iconbtn" data-act="sk-undo" aria-label="撤销一笔" title="撤销一笔（Ctrl Z）">${icon('undo', 16)}</button></div>
                <div class="toolbar sw">${SW.map((col) => `<button type="button" class="swatch${K.color.toLowerCase() === col.toLowerCase() ? ' on' : ''}" style="--c:${col}" data-act="sk-sw" data-v="${col}" aria-label="颜色 ${col}"></button>`).join('')}
                    <label class="swatch pick" style="--c:${K.color}" title="自选颜色"><input type="color" id="skColor" value="${K.color}" aria-label="自选颜色"></label>
                    <span class="sp"></span>${seg('sk-part', [['all', '全部'], ['head', '头'], ['body', '身体'], ['arm', '手臂'], ['leg', '腿']], K.part)}</div>
                <div class="uvbox"><canvas id="skinCv" width="64" height="64" aria-label="皮肤展开图，按住拖动来画"></canvas><canvas id="skinMask" width="64" height="64" aria-hidden="true"></canvas><i class="uvgrid" aria-hidden="true"></i></div>
                <p class="note" id="skinHover">指到格子上看是哪个部位</p>
            </section>
            <section class="card sk-3d">
                <div class="stage" id="skinStage"><div class="turn" id="skinTurn"><div class="${K.pose === 'walk' ? 'walk' : ''}" style="transform-style:preserve-3d">${rigHTML(K.slim)}</div></div><span class="floor" aria-hidden="true"></span></div>
                <div class="toolbar">${seg('sk-slim', [['0', 'Steve · 4 像素手臂'], ['1', 'Alex · 3 像素手臂']], K.slim ? '1' : '0')}
                    ${seg('sk-pose', [['stand', '站着'], ['walk', '走路']], K.pose)}
                    <label class="tick"><input type="checkbox" id="skOuter"${K.outer ? ' checked' : ''}>外套层</label>
                    <span class="sp"></span><button type="button" class="btn sm" data-act="sk-face" data-v="front">正面</button><button type="button" class="btn sm" data-act="sk-face" data-v="back">背面</button></div>
                <p class="note">拖动转着看。正式版的 3D 预览用 skinview3d（MIT 许可），能摆姿势。</p>
            </section>
            <section class="card sk-ai">
                <h3><span class="ico violet">${icon('sparkles', 16)}</span>AI 生成<span class="chip">结果落进编辑器，能接着改</span></h3>
                ${seg('sk-route', routes, K.route)}
                <p class="note">${routeNote}</p>
                <div class="ag-input"><textarea id="skinDesc" rows="2" placeholder="说你想要什么样的，比如：穿蓝色卫衣、戴黑框眼镜的程序员">${esc(K.gen?.text || '')}</textarea>
                    <button type="button" class="send" data-act="sk-gen" aria-label="生成"${K.busy ? ' disabled' : ''}>${icon('sparkles', 16)}</button></div>
                <div class="sk-ex">${EX.map((x) => `<button type="button" data-act="sk-ex" data-v="${esc(x)}">${esc(x)}</button>`).join('')}</div>
                <div id="skGen">${genHTML(K)}</div>
            </section>
            <section class="card sk-hist">
                <h3><span class="ico sky">${icon('history', 16)}</span>历史版本</h3>
                ${K.hist.length ? `<div class="thumbs">${K.hist.map((h, k) => `<button type="button" class="thumb" data-act="sk-hist" data-k="${k}" title="退回到这个版本"><img src="${h.url}" alt=""><span>${esc(h.label)}<small>${h.at}</small></span></button>`).join('')}</div>` : '<p>每次生成、导入、换上之前，都会把当时的样子存一份在这里。</p>'}
                <dl class="facts">
                    <dt>正版账户</dt><dd>调 Minecraft 官方接口上传，一键换上；披风只能选自己有的</dd>
                    <dt>离线账户</dt><dd>只有自己看得到；想让别人看到，用 LittleSkin 这类外置登录，或者都装 CustomSkinLoader</dd>
                    <dt>风景头图</dt><dd>换上以后，每个实例头图里站着的小人就是你的皮肤</dd>
                </dl>
            </section>
        </div>
    </div>`;
}
