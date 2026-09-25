# API 参考

> 去 Wails 后服务端只剩两条面：**Agent WebSocket**（`/agent/ws`，唯一业务 API）与**3D 资产文件端点**（无状态本地文件）。日志解析、投影查询、配置全部在前端完成（wasm + localStorage），不经网络。响应字段名以代码为准，本表用于快速查阅。

## Agent WebSocket（`/agent/ws`）

实现：[app/transport/http/agentws.go](../app/transport/http/agentws.go)；前端客户端：[services/agent/ws/client.ts](../frontend/src/services/agent/ws/client.ts)。

**连接语义**：每条 WS 连接一个 agent 会话（服务端纯内存，断线即失，不落盘）；creds（LLM apiKey/地址/模型）由 init 帧推送，服务端不读任何持久化配置；**会话历史持久化在前端 localStorage**——每轮定稿后服务端以 `context_sync` 帧推回裁剪上下文快照，断线重连后前端以 `restore` 帧推回水合。断线由前端自动重连并重发 init；服务端周期 ping（25s）保活。帧为 JSON 文本帧，`type` 判别。

### C → S 帧

| type | 字段 | 说明 |
|------|------|------|
| `init` | `llm` | 建会话（先于其他帧）。`llm` = LlmConfig（`baseUrl/apiKey/model/temperature/maxSteps*`），回 `ready` |
| `chat` | `message, level, fileName, summary` | 一轮对话。`fileName` 会话隔离键；`summary` = wasm parseFinish 载荷的 summary 节（Go 据此建提示词/工具上下文），缺省报 `no log loaded` |
| `stop` | — | 取消进行中的一轮 |
| `restore` | `fileName, messages` | 断线重连后推回本地持久化的上下文快照（`context_sync` 帧产出，前端原样存取不解析）；每文件每连接至多一次，须先于该文件首个 `chat` |
| `clear` | `fileName` | 清空会话，回 `cleared` |
| `data_response` | `callId, payload \| error` | 工具数据回包（payload = wasm 查询结果 JSON；查询失败走帧级 `error` 原文，不占 payload） |

### S → C 帧

| type | 字段 | 说明 |
|------|------|------|
| `ready` | — | init 确认 |
| `agent_event` | `event` | 流式事件（delta/reasoning/tool_start/tool_end/final/error，载荷同原 `agent:event`） |
| `chat_result` | `ok, message \| error` | 一轮的回执（前端 chat promise 据此 resolve/reject） |
| `context_sync` | `fileName, messages` | 每轮历史定稿后的裁剪上下文快照（trimContext 语义，eino 消息 JSON 数组）；前端存 localStorage 作 restore 载荷 |
| `cleared` | — | clear 确认 |
| `data_request` | `callId, query` | 工具数据请求（见下） |
| `error` | `error` | 协议/服务错误 |

### data_request 的 query（`agentservice.Query` 镜像）

```jsonc
{ "kind": "signal", "payload": { /* wasm signalQuery 请求体，见 wasm-parser.md */ } }
{ "kind": "types" }                       // → messageTypes 投影
{ "kind": "fields", "type": "GPS" }       // → fields 投影
{ "kind": "parameters" | "errors" | "events" | "modes" | "commands" | "mavlink_commands" }
```

投影类 payload = parseFinish 载荷对应节原样（camelCase DTO，非有限浮点为 `"NaN"/"Inf"/"-Inf"` 哨兵）；`signal` = wasm 数值计算结果。超时 30s（`ErrDataTimeout`）。

## HTTP 文件端点

| 路由 | 说明 |
|------|------|
| `GET /model-file?path=` | 3D 模型文件（glb 等）本地直读；`Cache-Control: max-age=3600` |
| `GET /tiles/:id/*filepath` | 3D Tiles 目录树（tileset.json + b3dm/pnts 子资源，Range 支持）；`id` 经 configservice tileset 注册表解析为根目录，路径限制在根内（防穿越） |

> tileset/模型**导入 UI 已冻结**（功能保留不开放，方案后议）；端点与注册表后端保留。

## 静态 SPA

其余全部路径由嵌入的 `frontend/dist` 服务（`spaHandler`：文件命中直出，未命中回退 `index.html` 供前端 history 路由）。开发模式走 vite（5173），`/agent`（WS）、`/model-file`、`/tiles` 由 vite 代理到 Go 服务器（`vite.config.ts server.proxy`）。
