// DroneLogAnalyzer 后端：纯 web 服务器。
//
// 解析在前端 wasm、配置在前端 localStorage、地图瓦片直连 provider——
// 服务端只保留：
//   - /agent/ws   Agent 会话（creds 由 init 帧推送；工具数据经
//     data_request/data_response 与前端 wasm 查询层往返）
//   - /model-file /tiles/:id   3D 资产本地文件端点（无状态）
//   - 嵌入的前端 SPA 静态服务（含 history 路由回退）
//
// 运行：直接执行（默认 127.0.0.1:8642 并打开浏览器）；-addr 换监听
// 地址（0.0.0.0:8642 可供局域网访问），-open=false 关闭自动开窗。
package main

import (
	"embed"
	"flag"
	"fmt"
	"io/fs"
	"log"
	"net"
	"net/http"
	"os/exec"
	"runtime"
	"strings"

	appcfg "drone-log-analyzer/app/config"
	"drone-log-analyzer/app/services/configservice/jsonstore"
	httptransport "drone-log-analyzer/app/transport/http"

	"github.com/klauspost/compress/gzhttp"
)

//go:embed all:frontend/dist
var staticFiles embed.FS

func main() {
	addr := flag.String("addr", "127.0.0.1:8642", "HTTP 监听地址")
	open := flag.Bool("open", true, "启动后打开浏览器")
	flag.Parse()

	appcfg.MigrateConfigDir()
	// 会话历史已前端化（localStorage），顺手清掉旧版本服务端落的
	// agent_session_*.json。
	appcfg.RemoveLegacyAgentSessions()

	dist, err := fs.Sub(staticFiles, "frontend/dist")
	if err != nil {
		log.Fatalf("load embedded frontend files failed: %v", err)
	}

	// configservice 仅剩 tileset/模型注册表（/tiles/:id 的目录解析）；
	// LLM/界面/曲线等配置已前端化。
	cfgSvc := jsonstore.New()

	// 静态资产 gzip/zstd 压缩：SPA 面裸传首访 ~8.6MB → ~2.5MB（按 Accept-Encoding
	// 协商，≥1KB 才压，Range/视频/音频自动跳过）。只包静态 handler——/agent/ws
	// 的连接升级与 /tiles、/model-file 的字节区间语义不受影响。
	router := httptransport.Router(cfgSvc, gzhttp.GzipHandler(spaHandler(dist)))

	ln, err := net.Listen("tcp", *addr)
	if err != nil {
		log.Fatalf("listen %s: %v", *addr, err)
	}
	url := fmt.Sprintf("http://%s", ln.Addr().String())
	log.Printf("DroneLogAnalyzer serving on %s", url)
	if *open {
		go openBrowser(url)
	}
	if err := http.Serve(ln, router); err != nil {
		log.Fatalf("serve: %v", err)
	}
}

// spaHandler 服务嵌入的前端 dist：文件命中直接回；未命中回退 index.html
// （前端 history 路由刷新兜底），静态资产命中不受影响。
func spaHandler(dist fs.FS) http.Handler {
	fileServer := http.FileServer(http.FS(dist))
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		path := strings.TrimPrefix(r.URL.Path, "/")
		if path == "" {
			path = "index.html"
		}
		if _, err := fs.Stat(dist, path); err != nil {
			r.URL.Path = "/"
		}
		fileServer.ServeHTTP(w, r)
	})
}

// openBrowser 尽力而为地拉起系统浏览器（失败静默，地址已打印）。
func openBrowser(url string) {
	var cmd *exec.Cmd
	switch runtime.GOOS {
	case "windows":
		cmd = exec.Command("cmd", "/c", "start", "", url)
	case "darwin":
		cmd = exec.Command("open", url)
	default:
		cmd = exec.Command("xdg-open", url)
	}
	if err := cmd.Start(); err != nil {
		log.Printf("open browser: %v（手动访问 %s）", err, url)
	}
}
