# 飞控日志分析 (Drone Log Analyzer)

本地运行的飞控日志分析应用：**Go web 服务器 + 浏览器前端**（启动后自动打开浏览器，关闭 Ctrl+C 即退出）。支持多格式飞控日志：

- **ArduPilot Dataflash**（`.bin` / `.log`）—— 二进制与文本日志
- **PX4 ULog**（`.ulg`）
- **MAVLink tlog**（`.tlog`）—— 遥测日志

架构要点：日志解析在前端 **Rust WASM**（[`wasm-parser/`](wasm-parser/)）完成；地图瓦片直连 provider https；配置（含 LLM creds）存浏览器 localStorage——**服务端无日志/配置状态**，只保留 Agent WebSocket（`/agent/ws`）与 3D 资产文件端点。

## 环境要求

- Go 1.25 或更高版本
- Node.js（首次/换机后在仓库根执行 `npm install`）
- Rust（仅改解析器时需要，wasm 产物已入库；工具链在 `~/.cargo/bin`）
- Windows 下可选安装 `make`

## 编译与运行

```bash
make dev        # Go 服务器(8642) + vite dev(5173, HMR)，浏览器访问 http://localhost:5173
make build      # 前端 + go build -> build/bin/DroneLogAnalyzer.exe
make run        # 构建后直接运行（ADDR=0.0.0.0:8642 可供局域网访问）
```

运行发行版：`build/bin/DroneLogAnalyzer.exe`（默认监听 `127.0.0.1:8642` 并自动打开浏览器；`-addr` 换地址，`-open=false` 关闭自动开窗）。

Docker 镜像与 k8s 部署（网关代理接入）见 [deploy/README.md](deploy/README.md)。

若本机没有 `make`，直接跑底层命令：

```bash
# dev
go run . -addr 127.0.0.1:8642 -open=false &
node node_modules/vite/bin/vite.js dev

# build
node node_modules/vite/bin/vite.js build
go build -o build/bin/DroneLogAnalyzer.exe .
```

提交或打包前建议检查（或直接 `make check`）：

```bash
go test ./...
go vet ./...
go build -buildvcs=false ./...
node node_modules/vue-tsc/bin/vue-tsc.js --noEmit
cd wasm-parser && cargo test   # 解析器金样对等（改 .rs 时必跑）
```

## 使用说明

1. 启动程序（浏览器自动打开，或手动访问打印的地址）。
2. 点击「打开日志文件」选择 `.bin` / `.log` / `.ulg` / `.tlog`——文件在浏览器内由 wasm 解析，不经服务端。
3. 选择消息类型和字段查看曲线。
4. 查看参数、飞行模式、文本消息和原始消息行。
5. AI 面板：在设置里填 LLM 接入（apiKey/地址/模型，存本地浏览器），即可对话式分析日志。

## 更多文档

- [docs/architecture.md](docs/architecture.md) — 总体架构、wasm 解析、无状态后端、数据流
- [docs/frontend.md](docs/frontend.md) — 前端目录、setup store、传输 client、3D/地图/chart、profile
- [docs/api.md](docs/api.md) — Agent WebSocket 协议、HTTP 端点
- [docs/wasm-parser.md](docs/wasm-parser.md) — wasm 解析器契约（零拷贝/查询层/金样）
- 开发约束（模块边界、禁止行为、不变量）见 [AGENTS.md](AGENTS.md)
