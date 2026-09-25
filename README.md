# 飞控日志分析 (Drone Log Analyzer)

[English](README.en.md) | 简体中文

本地运行的飞控日志分析应用：**Go web 服务器 + 浏览器前端**（启动后自动打开浏览器）。支持多格式飞控日志：

- **ArduPilot Dataflash**（`.bin` / `.log`）—— 二进制与文本日志
- **PX4 ULog**（`.ulg`）
- **MAVLink tlog**（`.tlog`）—— 遥测日志

所有数据均在浏览器端处理与储存：日志解析在浏览器内完成，配置也只保存在浏览器本地，服务端不保存任何日志或配置数据。

## 功能

**曲线分析**
- 任意消息字段的曲线绘制，多曲线叠加对比
- 曲线缩放、平移、按需显示/隐藏
- 飞行模式时间线标注，消息事件与曲线联动

**3D 飞行回放**
- 三维姿态与航线回放，无人机模型实时跟随日志数据
- 姿态仪表、桨叶与舵面动态效果
- 时间轴拖动控制回放进度，曲线/3D/地图同步联动

**地图可视化**
- 2D 地图航线显示，瓦片本地缓存加速
- 3D 地球视图，支持全球地形与三维轨迹

**数据浏览**
- 飞控参数查看与搜索
- 飞行事件与文本消息浏览
- 原始消息行查看

**AI 分析**
- 内置 AI 助手，自动生成飞行分析报告（LLM 接入信息存本地浏览器）

**其他**
- 中文 / 英文界面切换
- 所有数据保存在浏览器端，不出本机

## 环境要求

- Go 1.25 或更高版本
- Node.js（首次/换机后在仓库根执行 `npm install`）
- Rust（仅修改解析器时需要，wasm 产物已入库）

## 编译与运行

```bash
make dev        # Go 服务器(8642) + vite dev(5173, HMR)，浏览器访问 http://localhost:5173
make build      # 前端 + go build -> build/bin/DroneLogAnalyzer.exe
make run        # 构建后直接运行（ADDR=0.0.0.0:8642 可供局域网访问）
```

运行发行版：`build/bin/DroneLogAnalyzer.exe`（默认监听 `127.0.0.1:8642` 并自动打开浏览器；`-addr` 换地址，`-open=false` 关闭自动开窗）。Docker 镜像与 k8s 部署见 [deploy/README.md](deploy/README.md)。

若本机没有 `make`，直接跑底层命令：

```bash
# dev
go run . -addr 127.0.0.1:8642 -open=false &
node node_modules/vite/bin/vite.js dev

# build
node node_modules/vite/bin/vite.js build
go build -o build/bin/DroneLogAnalyzer.exe .
```

## 使用说明

1. 启动程序（浏览器自动打开，或手动访问打印的地址）。
2. 点击「打开日志文件」选择 `.bin` / `.log` / `.ulg` / `.tlog`——文件在浏览器内解析，不经服务端。
3. 选择消息类型和字段查看曲线。
4. 查看参数、飞行模式、文本消息和原始消息行。
5. AI 面板：在设置里填 LLM 接入（apiKey/地址/模型，存本地浏览器），即可对话式分析日志。
