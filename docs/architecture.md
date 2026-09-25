# 总体架构

> 本文讲清无状态 web 架构的整体设计与数据流。前端架构见 [frontend.md](frontend.md)；接口协议见 [api.md](api.md)；wasm 解析器契约见 [wasm-parser.md](wasm-parser.md)。

## 架构总览（2026-09 去 Wails 重构后）

```
┌─ 浏览器（全部业务逻辑） ────────────────────────────────────────┐
│ Vue 3 SPA + Rust WASM 解析器（wasm-parser/）                     │
│   日志文件 → File 直喂 wasm（parseStart/parseFeed/parseFinish）  │
│   曲线渲染：TypeBody 零拷贝视图（wasm 线性内存直供）              │
│   地图瓦片：直连 provider https（map-providers.ts）              │
│   配置（含 LLM creds）：localStorage                             │
└────────────┬─────────────────────────────────────────────────────┘
             │ WS /agent/ws（唯一 API 通道） + 文件端点
┌────────────▼─────────────────────────────────────────────────────┐
│ Go 服务器（main.go → gin）                                       │
│   /agent/ws   Agent 会话（每连接一个；creds 来自 init 帧）        │
│               工具数据经 data_request⇄data_response 回前端 wasm  │
│   /model-file /tiles/:id   3D 资产本地文件（无状态，冻结功能）    │
│   静态 SPA（//go:embed frontend/dist，history 回退）             │
│   agentservice：eino ReAct 编排 + 知识库融合 + 编码/截断/预算     │
└──────────────────────────────────────────────────────────────────┘
```

**服务端无状态**：不保存日志、不保存用户配置（LLM creds 由前端每连接经 init 帧推送）。重启服务器不丢任何东西（会话历史除外——按日志文件名落盘在配置目录）。

## 后端 `app/` 三层

```
modules/    纯工具：knowledge（字段/参数知识库，formats/*.json 内嵌）
services/   领域服务：agentservice（LLM 编排+工具）/ configservice（仅 tileset 注册表）
transport/  传输适配：http（gin 路由 + Agent WebSocket）
main.go     组装：embed frontend/dist → Router → net.Listen（-addr/-open 参数）
```

依赖方向严格自上而下：`transport → services → modules`。

## 组装（main.go）

1. `appcfg.MigrateConfigDir()` —— 旧配置目录一次性迁移（须在任何 service 读配置前）。
2. `cfgSvc := jsonstore.New()` —— 仅 tileset 注册表（`/tiles/:id` 目录解析；导入 UI 已冻结）。
3. `router := httptransport.Router(cfgSvc, spaHandler(dist))` —— Agent WS + 文件端点 + SPA。
4. `net.Listen` + 可选自动开浏览器（`-open`，默认开）。

## Agent 数据流（一轮对话）

```
浏览器 chat 帧 {message, level, fileName, summary}   ←─ 日志上下文取自 wasm 载荷缓存
   └→ agentservice.Chat（eino ReAct 循环）
        ├→ 工具调用 deps.Data.Query({kind, payload})
        │     └→ WS data_request 帧 ──→ 前端分发（wasm 查询层）
        │          9 类投影：直读 parseFinish 载荷缓存
        │          signal：wasm signalQuery（fieldstats 全套运算）
        │     ←─ WS data_response 帧 {callId, payload}
        ├→ 知识库融合 / cols-rows 编码 / 截断 / AbsTime 列（Go 侧）
        └→ 流式 agent_event 帧（delta/tool_start/...）→ chat_result 帧
```

数值计算在 wasm（min/max/derivative/trend/peaks/越限段/LTTB 抽稀），Go 只做编排、知识库阈值、绝对时刻列与 token 预算——上下文体积控制点不变。

## 日志解析（wasm-parser/，Rust）

三格式（dataflash/ulog/tlog）在浏览器解析，与原 Go 解析**金样对等**（`wasm-parser/tests/fixtures/`：TypeBody 逐字节、全部 logservice 投影值级、signal 查询矩阵）。增量 feed 状态机 + rAF 分块驱动，主线程不阻塞。契约详见 [wasm-parser.md](wasm-parser.md)。

## 格式标识透传

`Summary.Format`（`apm`/`ulog`/`tlog`）随 parseFinish 载荷到前端；前端 3D/地图/飞行数据按 format 取字段源映射（见 [frontend.md §profile](frontend.md#格式无关的字段源-profile)）。曲线/字段模板状态随格式分键存 localStorage。

## 关键不变量

| 不变量 | 含义 |
|--------|------|
| 服务端无日志/配置状态 | 解析在前端 wasm；配置在 localStorage；creds 走 WS init 帧 |
| services 不 import transport | 业务与传输解耦；传输层实现 service 接缝（Data/EventSink） |
| 不引入 Wails/桌面壳 | 纯 web 服务器架构（用户决策，2026-09-19 起） |
| wasm 零拷贝契约 | parseFinish 后只读；signalQuery 例外带护栏（见 wasm-parser.md） |
| 日志格式只增不改 | 新格式 = wasm-parser 新后端 + 新 profile 文件 |
| tileset/模型导入冻结 | 端点保留、UI 不开放，注册方案后议 |
