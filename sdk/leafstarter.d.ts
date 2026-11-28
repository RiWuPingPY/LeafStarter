// LeafStarter 脚本插件的接口类型（MIT 许可）。在插件页面里通过全局的 `leafstarter` 使用。
export interface InstanceBrief { id: string; name: string; mc: string; loader: string | null }
export interface Instance extends InstanceBrief { loaderVersion: string | null; mods: { name: string; file: string; enabled: boolean }[] }

export interface Methods {
  /** 在启动器里弹一句提示（不要权限） */
  'ui.toast': [{ text: string }, void];
  /** 实例列表（instances:read） */
  'instances.list': [undefined, InstanceBrief[]];
  /** 当前选中的实例和它的 Mod（instances:read） */
  'instance.current': [undefined, Instance | null];
  /** 启动一个实例，每次都会问用户（launch） */
  'instance.launch': [{ id?: string }, true];
  /** GET 一个 https 网址，只能是 plugin.json 里 net:域名 写了的（net:<域名>） */
  'net.fetch': [{ url: string }, { status: number; text: string }];
  /** 插件自己的小存储，最多 256 KB（storage） */
  'storage.get': [{ key: string }, unknown];
  'storage.set': [{ key: string; value: unknown }, null];
  /** 调原生插件自带的程序（native，只有认证开发者的原生插件能要） */
  'native.call': [{ method: string; params?: unknown }, unknown];
  /** 交回 AI 工具的结果（ai:tools），call 来自 tool 事件 */
  'ai.result': [{ call: string; result?: unknown; error?: string }, boolean];
}

export interface Events {
  /** 游戏启动、运行、退出、崩溃（events） */
  launch: { instance: string; phase: 'preparing' | 'launching' | 'running' | 'exited' | 'crashed' | 'failed' };
  /** AI 助手要用这个插件的工具（ai:tools）；做完用 ai.result 交回 */
  tool: { call: string; tool: string; input: unknown };
}

export interface LeafStarter {
  call<K extends keyof Methods>(method: K, params?: Methods[K][0]): Promise<Methods[K][1]>;
  on<K extends keyof Events>(event: K, fn: (data: Events[K]) => void): void;
}

declare global {
  const leafstarter: LeafStarter;
}
