# syntax=docker/dockerfile:1

# 构建说明（详见 deploy/README.md）：
#   镜像内只做 Go 交叉编译。前端 frontend/dist（含 wasm parser 资产）
#   必须先在本机/CI 预构建好（make web），随构建上下文带入。
#
# 多架构：docker buildx build --platform linux/amd64,linux/arm64 ...
#   构建器始终跑在宿主架构（--platform=$BUILDPLATFORM），目标架构交给
#   Go 交叉编译（CGO_ENABLED=0 + GOARCH=$TARGETARCH）。

# ---- 构建阶段：Go 静态交叉编译 ----
FROM --platform=$BUILDPLATFORM golang:1.25-alpine AS build

# buildx 自动注入；普通 docker build 默认取宿主平台
ARG TARGETOS
ARG TARGETARCH
# 国内网络覆盖：--build-arg GOPROXY=https://goproxy.cn,direct
ARG GOPROXY=https://proxy.golang.org,direct
ENV GOPROXY=${GOPROXY}

WORKDIR /src

# 依赖层（go.mod/go.sum 不变则命中缓存）
COPY go.mod go.sum ./
RUN go mod download

# 源码：go build . 只需根包 + app/（cmd/ 是 fixturegen 工具，不进镜像）
COPY main.go ./
COPY app ./app

# go:embed all:frontend/dist 的输入（缺失时本步直接报错，先跑 make web）
COPY frontend/dist ./frontend/dist

RUN CGO_ENABLED=0 GOOS=$TARGETOS GOARCH=$TARGETARCH \
    go build -buildvcs=false -trimpath -ldflags="-s -w" -o /out/dla .

# ---- 运行阶段：静态二进制，无需 libc ----
# 国内拉取 gcr.io 受限时可换：
#   FROM alpine:3.20
#   USER nobody
FROM gcr.io/distroless/static-debian12:nonroot

COPY --from=build /out/dla /dla

EXPOSE 8642
USER nonroot:nonroot

# k8s 内必须监听 0.0.0.0 且不拉起浏览器
ENTRYPOINT ["/dla", "-addr", "0.0.0.0:8642", "-open=false"]
