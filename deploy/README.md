# Docker 镜像与 k8s 部署

镜像内**只做 Go 交叉编译**：前端（Vue + wasm parser）预先构建为 `frontend/dist`，
随构建上下文带入，由 `//go:embed` 嵌入二进制。运行时是单个静态文件、无状态
（解析在前端 wasm、配置在 localStorage、LLM creds 由 WS init 帧推送），可任意多副本。

## 1. 预构建前端（必须先做）

```sh
make web        # vite build -> frontend/dist（含 wasm 资产；首次先 npm install）
```

`frontend/dist` 缺失时 `docker build` 会在 `COPY frontend/dist` 一步直接报错。

## 2. 构建镜像

一键（= `make web` + `docker build`，`IMAGE=`/`GOPROXY=` 可覆盖；
shell 里已 export 的 GOPROXY 会被自动吸收透传）：

```sh
make docker
make docker IMAGE=reg.example.com/drone-log-analyzer:v1   # 自定义 tag
```

手动等价：

```sh
# 单架构
docker build -t drone-log-analyzer:latest .

# 多架构（amd64 + arm64，Go 交叉编译，构建器不模拟目标架构）
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t <你的registry>/drone-log-analyzer:latest --push .
```

国内网络覆盖 Go 模块代理：

```sh
make docker GOPROXY=https://goproxy.cn,direct
```

### 打包参数：Cesium CDN 离载（CESIUM_CDN）

默认构建把 Cesium 全家桶（~14MB：Cesium.js + Workers/Assets/ThirdParty/Widgets）
拷进 `frontend/dist`、由 `//go:embed` 嵌入二进制，从本服务器分发。服务器带宽
压力大时，可让 Cesium 改由官方 CDN 提供，dist 与镜像同步变小：

```sh
make docker CESIUM_CDN=1            # 官方 jsdelivr（版本锁定 node_modules 的 cesium）
make docker CESIUM_CDN=<base-url>   # 自定义源，如国内可用的
                                    # https://registry.npmmirror.com/cesium/<版本>/files/Build/Cesium/
```

生效方式：`scripts/vite-plugin-cesium-cdn.mjs` 替代 `vite-plugin-cesium`，
index.html 注入 CDN 的 Cesium.js/widgets.css，Workers/Assets 等由 Cesium 从
script src 自推基址、同样落在 CDN；dist 不再包含 `cesium/` 目录。前端代码零改动
（`import 'cesium'` 编译为全局 `window.Cesium`）。代价：依赖 CDN 可达性——
内网无外联的环境请保持默认自托管。

> 服务端 SPA 静态资产恒开 gzip/zstd 压缩（按 `Accept-Encoding` 协商），
> 与本参数无关：剩余 js/css/wasm 传输体积约为裸传的 1/3。

> 运行层用 `gcr.io/distroless/static-debian12:nonroot`（无 shell、非 root）。
> 拉取受限时换 Dockerfile 里注释的 `alpine:3.20` 备选即可。

## 3. 部署到 k8s

```sh
# 先把 deployment.yaml 里的 image 换成你的仓库地址
kubectl apply -f deploy/k8s/
kubectl -n <namespace> rollout status deploy/drone-log-analyzer
```

要点：

- 监听 `0.0.0.0:8642`、`-open=false` 已固化在镜像 ENTRYPOINT。
- 探针走 `GET /`（命中嵌入 SPA 的 index.html）。
- `readOnlyRootFilesystem: true`：服务端无落盘需求（tileset 注册表功能冻结；
  解冻时再给配置目录挂 emptyDir）。
- 无 Secret/ConfigMap：服务端不读任何密钥。

## 4. 网关代理

upstream 指向 Service：`drone-log-analyzer.<namespace>.svc:8642`。

`/agent/ws` 是 WebSocket，nginx 需带 Upgrade 头并调大超时：

```nginx
location /agent/ws {
    proxy_pass http://drone_log_analyzer;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_read_timeout 3600s;   # Agent 会话长连接，按需调大
    proxy_send_timeout 3600s;
}
```

其余路径（`/`、`/model-file`、`/tiles/...`）普通 HTTP 反代即可。
