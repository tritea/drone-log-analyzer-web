import type { ConfigClient } from '../client';

/**
 * localStorage 配置实现（后端无状态化）：一切配置存前端本地。
 * 数据形状就是前端自己的词汇（无 Go DTO 的 template* 键位转译、无
 * entries 响应重映射——那些是 wails 绑定层的运输关注点）。
 * 旧 configservice 的存量配置不做迁移（用户重选即可）。
 *
 * 格式作用域：字段组（fieldEntries）/已存曲线（curveState）/飞行指标（flightMetrics）
 * 引用的都是 TYPE.field 字段名，跨格式互不通用——按当前格式分键（`name@fmt`），
 * 切格式各自一套。渲染偏好/地图物件等与格式无关的键保持全局。
 * 日志加载流程保证 setFormat 先于 loadFields/loadFlightMetricsConfig/曲线恢复。
 */
const KEY_PREFIX = 'dla.config.';
const key = {
  settings: 'settings',
  flightMetrics: 'flightMetrics',
  curveState: 'curveState',
  fieldEntries: 'fieldEntries',
  models: 'models',
  modelGroups: 'modelGroups',
  tilesets: 'tilesets',
  format: 'format',
} as const;

/** 格式作用域的键清单：切格式时分键存取。 */
const FORMAT_SCOPED = [key.fieldEntries, key.curveState, key.flightMetrics] as const;

function read<T>(name: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(KEY_PREFIX + name);
    if (raw === null) return fallback;
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function write(name: string, value: unknown): void {
  try {
    localStorage.setItem(KEY_PREFIX + name, JSON.stringify(value));
  } catch (e) {
    console.error('config persist failed:', e);
  }
}

/** 当前格式（内存态；启动时取上次格式，日志加载时 setFormat 更新）。 */
let currentFormat = '';
try {
  currentFormat = typeof read<string>(key.format, '') === 'string' ? read<string>(key.format, '') : '';
} catch {
  currentFormat = '';
}

/** 格式作用域键名；无格式时退回裸键（启动早期未加载日志）。 */
function fmtKey(name: string): string {
  return currentFormat ? `${name}@${currentFormat}` : name;
}

// 一次性迁移：旧版全局键 → 当前（上次）格式的分键，老用户不丢已存配置。
// 其他格式首次使用时分键为空、从头配置（本就不通用）。
for (const name of FORMAT_SCOPED) {
  const scoped = fmtKey(name);
  if (scoped === name) break; // 无格式信息可迁移
  if (localStorage.getItem(KEY_PREFIX + scoped) === null && localStorage.getItem(KEY_PREFIX + name) !== null) {
    write(scoped, read(name, name === key.fieldEntries ? [] : {}));
  }
}

/** 字段模板清单响应（沿用 wails 客户端解包后的形状，消费方不感知）。 */
function entriesResponse(entries: unknown, saved?: unknown): Record<string, unknown> {
  return { entries: Array.isArray(entries) ? entries : [], path: '', savedEntry: saved };
}

export const localConfigClient: ConfigClient = {
  getSettings: () => Promise.resolve(read('settings', {})),
  saveSettings: (settings) => {
    write(key.settings, settings);
    return Promise.resolve();
  },
  setFormat: (format: string) => {
    currentFormat = format || '';
    write(key.format, currentFormat);
    return Promise.resolve();
  },
  getFlightMetrics: () => Promise.resolve(read(fmtKey(key.flightMetrics), {})),
  saveFlightMetrics: (metrics) => {
    write(fmtKey(key.flightMetrics), metrics);
    return Promise.resolve();
  },
  getCurveState: () => Promise.resolve(read(fmtKey(key.curveState), { activeCurves: [] })),
  saveCurveState: (activeCurves) => {
    write(fmtKey(key.curveState), { activeCurves });
    return Promise.resolve();
  },
  listFieldEntries: () => Promise.resolve(entriesResponse(read(fmtKey(key.fieldEntries), []))),
  saveFieldEntry: (tmpl) => {
    const list = read<Record<string, unknown>[]>(fmtKey(key.fieldEntries), []);
    const name = (tmpl as { name?: string } | null)?.name ?? '';
    const i = list.findIndex((t) => (t as { name?: string }).name === name);
    if (i >= 0) list[i] = tmpl as Record<string, unknown>;
    else list.push(tmpl as Record<string, unknown>);
    write(fmtKey(key.fieldEntries), list);
    return Promise.resolve(entriesResponse(list, tmpl));
  },
  deleteFieldEntry: (name) => {
    const list = read<Record<string, unknown>[]>(fmtKey(key.fieldEntries), []).filter(
      (t) => (t as { name?: string }).name !== name,
    );
    write(fmtKey(key.fieldEntries), list);
    return Promise.resolve(entriesResponse(list));
  },
  listCustomModels: () => Promise.resolve(read(key.models, { models: [] })),
  saveCustomModel: (model) => {
    const payload = read<{ models: Record<string, unknown>[] }>(key.models, { models: [] });
    const name = (model as { name?: string } | null)?.name ?? '';
    const i = payload.models.findIndex((m) => (m as { name?: string }).name === name);
    if (i >= 0) payload.models[i] = model as Record<string, unknown>;
    else payload.models.push(model as Record<string, unknown>);
    write(key.models, payload);
    return Promise.resolve(payload);
  },
  deleteCustomModel: (name) => {
    const payload = read<{ models: Record<string, unknown>[] }>(key.models, { models: [] });
    payload.models = payload.models.filter((m) => (m as { name?: string }).name !== name);
    write(key.models, payload);
    return Promise.resolve(payload);
  },
  listModelGroups: () => Promise.resolve(read(key.modelGroups, { groups: [] })),
  saveModelGroup: (group) => {
    const payload = read<{ groups: Record<string, unknown>[] }>(key.modelGroups, { groups: [] });
    const name = (group as { name?: string } | null)?.name ?? '';
    const i = payload.groups.findIndex((g) => (g as { name?: string }).name === name);
    if (i >= 0) payload.groups[i] = group as Record<string, unknown>;
    else payload.groups.push(group as Record<string, unknown>);
    write(key.modelGroups, payload);
    return Promise.resolve(payload);
  },
  deleteModelGroup: (name) => {
    const payload = read<{ groups: Record<string, unknown>[] }>(key.modelGroups, { groups: [] });
    payload.groups = payload.groups.filter((g) => (g as { name?: string }).name !== name);
    write(key.modelGroups, payload);
    return Promise.resolve(payload);
  },
  listTilesets: () => Promise.resolve(read(key.tilesets, { tilesets: [] })),
  saveTileset: (t) => {
    const payload = read<{ tilesets: Record<string, unknown>[] }>(key.tilesets, { tilesets: [] });
    const name = (t as { name?: string } | null)?.name ?? '';
    const i = payload.tilesets.findIndex((x) => (x as { name?: string }).name === name);
    if (i >= 0) payload.tilesets[i] = t as Record<string, unknown>;
    else payload.tilesets.push(t as Record<string, unknown>);
    write(key.tilesets, payload);
    return Promise.resolve(payload);
  },
  deleteTileset: (name) => {
    const payload = read<{ tilesets: Record<string, unknown>[] }>(key.tilesets, { tilesets: [] });
    payload.tilesets = payload.tilesets.filter((x) => (x as { name?: string }).name !== name);
    write(key.tilesets, payload);
    return Promise.resolve(payload);
  },
};
