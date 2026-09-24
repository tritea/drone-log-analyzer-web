# wasm 解析器契约（wasm-parser/）

> Rust crate（仓库根 `wasm-parser/`）编译到 wasm32，在浏览器解析三格式飞控日志（dataflash `.bin/.log` / ulog `.ulg` / tlog `.tlog`）。**Go 解析已删除**——本 crate 与冻结金样是解析行为的唯一权威（用户指令：原 Go 写法即标准，逐条对齐）。

## 构建与产物

- `scripts/build-wasm.mjs`（wasm-pack，`--target web`）→ `frontend/src/wasm/parser/`；vite 插件在构建时自动触发，改 `.rs` 后跑 `make wasm` 或直接 `make web`。
- 前端入口 [services/log/wasm/loader.ts](../frontend/src/services/log/wasm/loader.ts)：**调用必须走 glue 模块导出**（`init()` 返回的是带 ABI 指针签名的原始 wasm exports，只从中捕获 `memory`）。

## 解析生命周期（增量 feed）

```ts
parseStart(filename)            // 丢弃上一代 LogFile，开新会话（≥8 字节嗅探格式）
parseFeed(bytes) → fedBytes     // 任意分块；rAF 节流驱动进度 UI
parseFinish() → jsonString      // 一次性序列化全部投影载荷（见下）
```

`parseFinish` 载荷（JSON.parse 后缓存于 [wasm/client.ts](../frontend/src/services/log/wasm/client.ts)）：`summary / messageTypes / fields / typeSchema / parameters / commands / mavlinkCommands / modeChanges / messages / errors / events / logDefs`——字段名与原 Go logservice DTO 的 JSON 标签逐一对齐（camelCase；非有限浮点为 `"NaN"/"Inf"/"-Inf"` 哨兵）。

## 零拷贝契约

- **finish 后 wasm 只读**：曲线体经 `typeBodyPtr(type)/typeBodyLen(type)` 以 `(ptr, len)` 视图直供渲染链（`new Uint8Array(memory.buffer, ptr, len)`），**不复制**。视图有效至下一次 `parseStart`。
- 构造保证：`parseFinish` 在发布状态前一次性完成全部序列化——首个指针交付后不再有解析期分配。
- **唯一例外 `signalQuery`**（agent 数值查询）：finish 后分配 KB 级响应。正常复用解析期释放的空闲内存不会增长线性内存；前端护栏检查 `memory.buffer` 前后同一性，换代时派发 `wasm-memory-grown` 事件清空 curve-manager 的 TypeBody 缓存（按需重建视图）。

## 查询层

- `query.rs`：logservice 投影（金样锁定值级对等）。
- `signal.rs` + `fieldstats/`：agent `query_data` 的数值核心——窗口切片、min/max/avg/minmax/p2p/变化率/趋势/峰值/越限段（含合并）、LTTB 抽稀 + RLE 压缩。请求/响应契约（camelCase JSON 字符串进出）：

```jsonc
// 请求（Go 工具组装；知识库阈值在 Go 侧解析成 conds 下发）
{ "type": "CTUN", "field": "Alt", "op": "raw|minmax|derivative|trend|peaks|abnormal",
  "startSec": 0, "endSec": 10, "maxPoints": 300,
  "conds": [{ "op": "gt", "value": 5 }], "mergeGapSecs": 1.0 }

// 响应：win/n/res；raw=pts(RLE'd)+res；stats 族全精度对象；abn=按级段行
{ "name": "CTUN.Alt", "op": "minmax", "win": [0, 123.4], "n": 4567,
  "stats": { "ok": true, "count": 4567, "min": 0.1, "minAt": 1.2, "...": "..." } }
```

语义细则：`type/field` 未注册报 `type not found: X[.Y]`；注册但不在 body（topic 时间列/字符串字段）回空序列；Go nil 切片语义 = JSON `null`。绝对时刻列（T 后缀）、RawBudget、50 段截断在 Go 工具侧组装。

## 金样对等（tests/fixtures/）

`cargo test`（native）跑四 fixture（apm_text/apm_bin/px4_ulog/apm_tlog，chunk=7 喂入）比对：
TypeBody/curve 二进制**逐字节**；全部投影 + signal 查询矩阵（fixturegen 镜像生成，`l2.json` 的 `signal` 节）**值级**。金样已冻结（fixturegen 随 Go 解析删除）；改解析行为 = 改金样 + 说明理由。数据表 `data/logdefs.json`、`data/mavlink-dialect.json` 为提交入库的生成物。

## tlog 布局要点（MAVLink wire 陷阱）

非扩展字段按字节宽降序稳定排序（8>4>2>1），扩展字段声明序殿后——gomavlib readwriter 的 sort 即规格；**列序仍按结构体声明序**。全 4 字节消息（ATTITUDE）重排恰好等于声明序所以侥幸对齐；混合宽度（BATTERY_STATUS）必错。时间戳 8 字节大端微秒；MAVLink2 尾零截断（payload 短于 wire 长度时读前零填充）。
