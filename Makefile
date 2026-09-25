APP := Drone-Log-Analyzer
EXE := build/bin/DroneLogAnalyzer.exe
ADDR ?= 127.0.0.1:8642

# 前端构建在本机需用 node 直调 vite/vue-tsc（见 memory npm-spawn-workaround）。
VITE := node node_modules/vite/bin/vite.js
TSC  := node node_modules/vue-tsc/bin/vue-tsc.js

# Rust 工具链在 ~/.cargo/bin（不在 Git Bash PATH，脚本内部兜底查找）。
WASM := node scripts/build-wasm.mjs

# Docker 镜像 tag / Go 模块代理（make docker IMAGE=... GOPROXY=... 覆盖；
# GOPROXY 默认吸收 shell 环境变量，国内网络无需额外传参）。
IMAGE   ?= drone-log-analyzer:latest
GOPROXY ?= https://proxy.golang.org,direct

# Cesium CDN 离载（打包参数）：CESIUM_CDN=1 用官方 jsdelivr（版本锁定
# node_modules 的 cesium）；CESIUM_CDN=<base-url> 自定义源（如 npmmirror、
# 自建镜像）。开启后 dist 不含 ~14MB 的 cesium/ 目录（exe/镜像更小，
# Cesium 流量不走本服务器），默认关闭 = 自托管。
CESIUM_CDN ?=

.PHONY: help check web build dev run docker wasm wasm-test fixture-gen clean

help:
	@echo 飞控日志分析工具（本地 web 服务架构：Go 服务器 + 浏览器前端）- Available targets:
	@echo   make check      - go vet + go build + 前端类型检查 (vue-tsc)
	@echo   make web        - 构建前端 TS -> frontend/dist（node 直调 vite；自动带 wasm）
	@echo   make build      - 前端 + go build 产出服务器 exe -> $(EXE)
	@echo   make run        - 构建后直接运行服务器（ADDR=... 覆盖监听地址）
	@echo   make docker     - 一键打包 Docker 镜像（自动先 make web；IMAGE=/GOPROXY=/CESIUM_CDN= 可覆盖）
	@echo   make dev        - 开发模式：Go 服务器(8642) + vite dev(5173, HMR, 代理 API)
	@echo   make wasm       - 重建 wasm 解析器 -> frontend/src/wasm/parser（增量跳过）
	@echo   make wasm-test  - cargo test（wasm-parser 金样 + 单测）
	@echo   make fixture-gen- 重新生成金样 fixtures + 方言/logdefs 表（go run ./cmd/fixturegen）
	@echo   make clean      - 清理构建产物

check:
	go vet ./...
	go build -buildvcs=false ./...
	$(TSC) --noEmit

# Rust wasm 解析器：wasm-pack 构建到 frontend/src/wasm/parser（vite 插件也会自动触发）。
wasm:
	$(WASM)

# Rust 测试：三格式金样对等 + 单元测试（native 跑，无需 wasm 环境）。
wasm-test:
	cd wasm-parser && PATH="$$PATH:$(HOME)/.cargo/bin" cargo test

# 重新生成金样与 Rust 侧数据表（fixtures/*.bin+json、logdefs.json、mavlink-dialect.json）。
fixture-gen:
	go run ./cmd/fixturegen

# 前端构建：frontend/src -> frontend/dist（Go //go:embed 嵌入）。首次/换机后先 npm install。
# CESIUM_CDN=1/=<url> 时 Cesium 全家桶改由 CDN 提供，dist 不含 cesium/。
web:
	CESIUM_CDN=$(CESIUM_CDN) $(VITE) build

# 打包构建：前端产物嵌入 + 服务器 exe（纯 go build，无 wails/build tag）。
build:
	CESIUM_CDN=$(CESIUM_CDN) $(VITE) build
	go build -buildvcs=false -o $(EXE) .

# 运行发行版（-open 自动拉起浏览器；-open=false 关闭）。
run: build
	$(EXE) -addr $(ADDR)

# 一键 Docker 打包：先构建前端（镜像内只做 Go 交叉编译，frontend/dist
# 由 //go:embed 嵌入，缺失则 COPY 步直接报错），再出镜像。多架构推仓库
# 见 deploy/README.md（buildx --push）。
docker: web
	docker build --build-arg GOPROXY=$(GOPROXY) -t $(IMAGE) .

# 开发模式：Go 服务器（API+嵌入 SPA）与 vite dev server 并行；vite 把
# /agent、/model-file、/tiles 代理到 Go（vite.config.ts server.proxy）。
# 浏览器访问 http://localhost:5173（HMR 生效）。
dev:
	go run . -addr $(ADDR) -open=false & \
	$(VITE) dev

clean:
	@if exist build rmdir /s /q build
	@if exist frontend\dist rmdir /s /q frontend\dist
	go clean
