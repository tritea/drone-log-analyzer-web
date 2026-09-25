# Drone Log Analyzer

English | [简体中文](README.md)

A locally-run flight controller log analysis app: **Go web server + browser frontend** (opens your browser automatically on startup). Supports multiple log formats:

- **ArduPilot Dataflash** (`.bin` / `.log`) — binary and text logs
- **PX4 ULog** (`.ulg`)
- **MAVLink tlog** (`.tlog`) — telemetry logs

All data is processed and stored in the browser: logs are parsed in the browser, and settings are kept in browser-local storage only — the server never stores any log or configuration data.

## Features

**Curve Analysis**
- Plot curves of any message fields, with multi-curve overlay and comparison
- Zoom, pan, and show/hide curves on demand
- Flight mode timeline annotations, with message events linked to curves

**3D Flight Playback**
- 3D attitude and trajectory playback, drone model driven live by log data
- Attitude indicator, spinning propellers and control-surface animations
- Scrub the timeline to control playback, with curve/3D/map kept in sync

**Map Visualization**
- 2D map with flight path display and local tile caching for faster loading
- 3D globe view with worldwide terrain and 3D trajectory

**Data Browsing**
- View and search flight controller parameters
- Browse flight events and text messages
- Inspect raw message lines

**AI Analysis**
- Built-in AI assistant that automatically generates flight analysis reports (LLM credentials stored in your browser only)

**Misc**
- Chinese / English UI switch
- All data stays in your browser — nothing leaves your machine

## Requirements

- Go 1.25 or later
- Node.js (run `npm install` at the repo root on first setup / new machine)
- Rust (only needed when modifying the parser; wasm artifacts are committed)

## Build & Run

```bash
make dev        # Go server (8642) + vite dev (5173, HMR), visit http://localhost:5173
make build      # frontend + go build -> build/bin/DroneLogAnalyzer.exe
make run        # run after build (ADDR=0.0.0.0:8642 to expose on LAN)
```

Run the release build: `build/bin/DroneLogAnalyzer.exe` (listens on `127.0.0.1:8642` by default and opens the browser automatically; use `-addr` to change the address, `-open=false` to disable auto-open). For Docker images and k8s deployment, see [deploy/README.md](deploy/README.md).

If `make` is unavailable, run the underlying commands directly:

```bash
# dev
go run . -addr 127.0.0.1:8642 -open=false &
node node_modules/vite/bin/vite.js dev

# build
node node_modules/vite/bin/vite.js build
go build -o build/bin/DroneLogAnalyzer.exe .
```

## Usage

1. Launch the app (the browser opens automatically, or visit the printed address).
2. Click "Open Log File" to load a `.bin` / `.log` / `.ulg` / `.tlog` file — it is parsed in the browser, never sent to the server.
3. Pick message types and fields to view curves.
4. Browse parameters, flight modes, text messages and raw message lines.
5. AI panel: fill in your LLM access (apiKey/endpoint/model, stored in your browser) in settings, then analyze the log conversationally.
