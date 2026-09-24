# 前端架构

> 本文讲清 `frontend/` 的目录布局、状态管理、传输 client、3D/地图/chart 模块。后端契约见 [architecture.md](architecture.md)；接口字段见 [api.md](api.md)；3D/地图渲染的硬核规则见 [skills/frontend-3d-map](../skills/frontend-3d-map)。

技术栈：Vue 3（`<script setup>` + Composition API）+ Pinia 3（**setup store**）+ Three.js r160 + Cesium + Leaflet 1.9，TypeScript，Vite 5 编译。**主图是自绘 `CurveChart`（[modules/analysis/renderer/curve-chart.ts](../frontend/src/modules/analysis/renderer/curve-chart.ts)），不用 ECharts。**

## 工程结构（注意：package.json 在仓库根）

`package.json` / `tsconfig.json` / `vite.config.ts` / `node_modules/` 都在**仓库根**，不在 `frontend/` 下。`frontend/` 只放 `src/`、`dist/`（gitignored，Go `//go:embed`）、`index.html`。所以跑 node 命令在**根目录**，不要 `cd frontend`。Vite 编译 `frontend/src` → `frontend/dist`，再由 Go 服务器同源服务（开发模式 vite 5173 + 代理，见 [api.md](api.md)）。

```
frontend/src/
├── main.ts / App.vue / lifecycle.ts   启动：createApp + createPinia + 挂载 + window 监听
├── stores/          Pinia setup store，一域一文件（见下节）
├── services/{log,map,config,agent}/  前端实现域：wasm/local/WS 客户端（见下节）
├── profiles/        格式无关的字段源映射（apm/ulog），见 §profile
├── modules/analysis/   分析域：renderer/ 自绘折线 CurveChart + 刻度/配色；store/ 按 data/view/curves 三域拆工厂；utils/ 曲线二进制解析/格式化/飞行模式
├── utils/           runtime.ts（非响应式重对象容器）/ dom / colors / drone-model / geo / maplibre-drone-layer / flight-fields
├── types/           所有状态/数据类型（store state 用 as XxxState 挂类型）
├── constants/       阈值/颜色常量
├── composables/     组合式逻辑（useToolbarOverflow）
├── components/      charts / common / curves / dialogs / fields / map / three + views/Home
└── wasm/parser/     Rust wasm 解析器构建产物（scripts/build-wasm.mjs 生成）
```

> 本项目**没有** `core/` 目录——共享逻辑在 `utils/`、`stores/` 的 action、`constants/`、`types/`。新代码按此归位。

## 服务 client：一域一文件夹，实现可替换

各域 client 是前端自有能力（wasm 解析 / localStorage / WebSocket），接口与实现分离，`index.ts` 导出当前实例：

```
services/log/
├── client.ts      接口 LogClient（load(File)/summary/messageTypes/typeBody/...）
├── types.ts       DTO 的 TS 镜像（namespace logservice）
├── pick-file.ts   浏览器文件选择（替代原生对话框）
├── wasm/          解析实现：loader.ts（wasm 初始化+memory 访问）+ client.ts（缓存 parseFinish 载荷）
└── index.ts       export const logClient = wasmLogClient

services/config/   localStorage 实现（local/client.ts）：设置/曲线/模板/模型/tilesets
services/map/      provider 常量直连（local/client.ts，URL 公式在 modules/shared/map-providers.ts）
services/agent/    WS 实现（ws/client.ts：/agent/ws 帧协议）+ storage.ts（LLM creds localStorage）
```

wasm 二进制契约：`typeBody` 返回 wasm 线性内存的零拷贝 `Uint8Array` 视图（finish 后 wasm 只读，视图到下次 parseStart 前有效），`parseTypeBody` 接受视图构造定位 `DataView`。store 只 import `{ logClient }` 等，不直接碰 wasm/WS 细节。

## 状态管理：Pinia setup store

所有跨组件共享状态进 Pinia store，组件不自己持有。统一 **setup store** 写法（`defineStore(name, () => {...})`），不用 Options Store：

```ts
// stores/ui.ts —— 纯状态域
export const useUiStore = defineStore('ui', () => {
  const ui = reactive({ mainView: 'chart', dragOver: false, /* ... */ }) as UiState;
  return { ui };
});
```

带逻辑的域（`log`/`chart`/`three` 等）额外有 `computed` 和普通函数（不是 Options 的 getters/actions）：

```ts
export const useLogStore = defineStore('log', () => {
  const log = reactive({ loading: false, summary: null, /* ... */ }) as LogState;

  const currentLogFileName = computed<string>(() => { /* ... */ });
  const filteredMessages = computed<LogMessage[]>(() => {
    // 跨域派生：在 computed 内调用其它 store（惰性，避开循环依赖初始化）
    return log.messages.filter((m) => useChartStore().formatMessageTime(m) /* ... */);
  });

  async function loadMessages(): Promise<void> {
    try { log.messages = await logClient.messages() ?? []; }
    catch { log.messages = []; }
  }
  return { log, currentLogFileName, filteredMessages, loadMessages };
});
```

要点：

- **state 是 `reactive()` 对象**，用 `as XxxState` 挂 `@/types` 的类型；store id = 文件名 = `useXxxStore`。
- **跨 store 调用写在 setup 函数/computed/函数体内**（不在模块顶层），否则 store 未注册时触发 "no active pinia"。
- setup store 的函数里**直接读写 `log.xxx`**（Pinia 允许），`this` 不参与——所以也不需要旧式的 `const self = this`。

### 异步：async/await，不要 .then 链

```ts
async function loadMessages() {
  try {
    const items = await logClient.messages();
    log.messages = Array.isArray(items) ? items : [];
  } catch {
    log.messages = [];          // 不要静默吞错；UI 兜底
  }
}
```

- 多个独立请求用 `Promise.all([...])` 并行。
- 用户可见错误用 `showToast(msg, 'error')`（来自 [`@/utils`](../frontend/src/utils/index.ts)，不是 `@/core/util`）。
- `loading` / `restoring` 这类标志要能在异常路径复位——放 `finally`，或像上例那样在 `catch` 里兜底赋值。

## 组件：`<script setup>` + storeToRefs

组件只做「展示 + 转发用户操作到 store」。共享状态从 store 拿，不在组件里 `ref` 一份：

```vue
<script setup lang="ts">
import { storeToRefs } from 'pinia';
import { useLogStore } from '@/stores/log';

const logStore = useLogStore();
const { log } = storeToRefs(logStore);          // state 必须经 storeToRefs 才保响应式
const { loadMessages } = logStore;              // 函数直接解构没问题
</script>
```

模板 ref 通过 store 注册（如 `:ref="registerThreeMain"`），让 store 能拿到 DOM——不在组件里持有 chart/three 实例。单个 `.vue` 超过 ~200 行或出现多个无关功能块时，拆成同目录小组件。

## 非响应式重对象：`runtime`

ECharts 早已移除；主图是自绘 [`CurveChart`](../frontend/src/modules/analysis/renderer/curve-chart.ts)。`CurveChart` 实例、Three.js 视图、Leaflet/Cesium 地图、RAF/timer 句柄这些**不该进响应式系统**的重对象，统一放在 [`modules/shared/runtime.ts`](../frontend/src/modules/shared/runtime.ts) 的模块级容器：

```ts
export const runtime: {
  mainChart: CurveChart | null;       // 自绘主图（绘图区铺满宿主元素）
  view3dMain: View3dMainRuntime | null;
  view3dDial: View3dDialRuntime | null;
  view3dCurve: CurveChart | null;     // 3D 底部曲线轴
  mapView: MapRuntime | null;         // Leaflet 地图
  earthView: EarthRuntime | null;     // Cesium 地球（3D 地图，含 lock 相机/地形/3D Tiles 状态）
  chartInteractionsBound: boolean;
} = { /* ...null... */ };
```

store 的函数 `import { runtime } from '@/utils/runtime'` 读写它。用对象容器（而非导出变量）是因为 ES 模块导入绑定只读、不能重新赋值，而 `runtime.mainChart` 需要被替换。

## 三个渲染模块

| 模块 | 入口 | 说明 |
|------|------|------|
| 自绘折线 | [`modules/analysis/renderer/`](../frontend/src/modules/analysis/renderer/) | `CurveChart` 直接画 GPU 折线，独立缩放/平移/可见性。曲线数据走后端二进制 `CurveData`/`TypeBody`/`TypeSchema`，`curve-binary.ts` 解析 |
| 3D 姿态/航线 | [`stores/three.ts`](../frontend/src/stores/three.ts) + `components/three/` | Three.js 场景：无人机模型、轨迹、姿态仪、桨叶、天空/地面/水面。姿态/位置/RC/电机源全部 profile 驱动 |
| 地图 | `modules/map-2d`（Leaflet 2D）+ `modules/earth`（Cesium 3D 地图） | 2D 用 Leaflet 加载瓦片；3D 地图用 Cesium 全球地形渲染无人机/3D 轨迹/3D Tiles/自定义模型（MapLibre 3D 渲染器已移除） |

3D 与 Cesium 的硬核规则（altitude 基准、GLSL ES 1.00、GCJ-02 约束、锁定相机）沉淀在 [skills/frontend-3d-map](../skills/frontend-3d-map)，**动 3D/地图前先读**。

## 配置存储与格式作用域

一切配置存 localStorage（`services/config/local`，键前缀 `dla.config.`），无服务端。**格式相关配置按日志格式分键**（`fieldEntries@apm` / `curveState@ulog`…）：字段组、已存曲线、飞行指标引用的字段名跨格式互不通用，切格式各自一套。**切换前必须冲洗三个防抖保存**（`applyLoadedLog` 在 `setFormat` 前调用 `flushCurveSave`/`flushFieldSettingsSave`/`flushSave`）——曲线/字段组/指标保存均有 ~350ms 防抖，不冲洗会让旧格式待写数据落进新格式键（跨格式串配置的根因）。渲染偏好（settings）与地图物件（models/tilesets）与格式无关，保持全局键。老版本的全局键会在启动时一次性迁移到上次使用的格式下。

## 移动端适配（小屏）

断点策略：`(max-width: 768px), (pointer: coarse)`（逗号=或），JS 侦测与 CSS 媒体查询用**同一条 query**，永不分叉：

| 件 | 位置 | 说明 |
|----|------|------|
| 状态与编排 | [`modules/shared/utils/viewport.ts`](../frontend/src/modules/shared/utils/viewport.ts) | `bindViewportWatchers()`（lifecycle 挂载）维护 `ui.mobile/portrait/fullscreen`；drawer 开合/展开、专注模式、全屏 API、`screen.orientation.lock('landscape')`（非标准 API，iOS 不支持时提示手动旋转） |
| 样式 | [`styles/_mobile.scss`](../frontend/src/styles/_mobile.scss) | **必须最后引入**（覆盖窄屏列宽规则）：主布局单列、弹窗全屏化（`.modal`/`.native-dialog`/`.settings-modal`）、可视化 HUD 边条、工具栏换行、时间轴压缩、2D 地图 attribution 黑底药丸化 |
| 曲线列表 drawer | `views/Home/components/DataSidebar.vue` | 小屏侧栏变 fixed drawer（滑入 + 遮罩），顶栏可展开铺满并触发全屏 + 横屏提示（`LandscapeHint.vue`） |
| 可视化 HUD 条 | `Stage.vue` + `styles/_mobile.scss` | 马鞍形：两端 72px 圆形姿态球（磨砂玻璃）+ 中段 36px 下沉数据药丸（图标化小块左右分布、横向滚动），容器透明遮挡最小；无标题无开关；电机输出小屏隐藏；小屏**不允许添加指标**（点数据块进编辑器替换字段）；RC 摇杆/地图 attribution 抬到球上方；姿态仪 WebGL 画布透明底（底色交给 CSS） |
| 工具侧栏 | `ChartToolbar`（`mobileSheetOpen`） | 小屏工具栏**左侧原位**（logo/打开/视图 tabs/曲线列表入口），右侧只加 tune 按钮收纳渲染器/相机/开关进 `tool-sheet`；渲染器/相机为**整行列表单选**（名称左、选中圆点右），开关**整行**（名称左、mini 开关右）；专注/AI/记录/设置照旧 |
| 专注模式 | `ChartToolbar` 专注按钮 + `viewport.ts toggleFocusMode()` | 纯 UI 切换（不动全屏/横屏锁——退出专注不转回竖屏）；退出入排不浮动——曲线图在图例行右端（无曲线时角落兜底小图标）、可视化在播放时间轴/曲线轴面板行内；系统返回退出全屏时自动退出专注 |
| 强制横屏门 | `viewport.ts syncLandscapeHint` + `LandscapeHint.vue` | 小屏竖屏即弹中央对话框（进入页面即检测、旋转即检测），仅「进入横屏」按钮（全屏 + `screen.orientation.lock`，不支持时提示手动旋转）；转横屏自动收回 |
| 曲线操作按钮 | `ChartLegend` 右端 sticky 组 | 小屏框选/重置钉在图例行右端（图例不换行、横向滚动），专注模式下也可达；弹窗 z 提到 120 压过曲线 drawer；SimplePicker 搜索框小屏并入标题行省竖向空间 |

触屏手势：`CurveChart` 与 3D 视口（`camera/rig.ts`）都走 Pointer Events——触屏单指=平移/轨道、双指=捏合缩放；鼠标拖拽默认=框选（`shiftZoomActive` 显式开关对两类指针统一生效），右键=重置视图；画布必须 `touch-action: none`（否则浏览器截断 pointer 流），拖拽手柄（播放头/高度条）同样已 pointer 化。X 轴带大小刻度尺（大刻度对齐网格线，小刻度 4/5 细分、过密自动隐藏，`minorTimeTicks`）。

## 格式无关的字段源：profile

3D/地图/飞行数据按日志格式取字段（APM `ATT.Roll` vs PX4 `vehicle_attitude.q[0..3]` 等），通过 [`profiles/`](../frontend/src/profiles/) 抽象，**不写死格式名**：

```ts
// profiles/profile.ts
export interface FormatProfile {
  format: string; label: string;
  attitudeSources: AttitudeSource[];   // euler 或 quat
  positionSources: PositionSource[];   // global（alt=绝对海拔 MSL + relAlt=相对 home）或 local NED
  defaultAttitude: string; defaultPosition: string;
  rc?: RcSource; motor?: MotorSource; volt?: FieldSource[];
  voltCells?: {...}; velocity?: {...}; armedDetection?: ArmedDetection;
  homePosition?: {...};
}
export function registerProfile(p: FormatProfile): void;
export function getProfile(format: string): FormatProfile | undefined;
```

各格式一个文件（`apm.ts`/`ulog.ts`），顶层 `registerProfile(...)`，`profiles/index.ts` 汇总，`main.ts` `import '@/profiles'` 触发注册。`three.ts` 用 `getProfile(summary.format)` 取当前 profile；**新格式只加 profile 文件，不动 three/地图/面板**。

## 文件加载

「打开文件」按钮 → `pickLogFile()`（浏览器文件选择，`services/log/pick-file.ts`）取 `File` → `logClient.load(file)`（wasm `parseStart/parseFeed/parseFinish`，rAF 分块驱动进度 UI）→ 返回摘要，前端 `applyLoadedLog` 复位派生 store + 调 `configClient.setFormat`。解析全程在浏览器内，不经服务端。拖放仍禁用（`lifecycle.ts` 捕获 drop，防浏览器默认打开文件）。

## 类型与依赖

- 业务模型类型集中在 [`types/index.ts`](../frontend/src/types/index.ts)，store state 用 `as XxxState` 挂类型；联合类型优于枚举。
- `api()` 返回的对接后端的局部 `any` 是当前刻意的取舍（后端 JSON 未强类型化），新代码尽量定义类型收窄，不必为存量 `any` 强改。
- 依赖走 npm（根 `package.json`），代码里 ES import；禁止 CDN `<script>` 与 `window.XXX`。
