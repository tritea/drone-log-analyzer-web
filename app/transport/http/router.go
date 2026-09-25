package http

import (
	"net/http"
	"os"
	"path/filepath"
	"strings"

	"drone-log-analyzer/app/services/configservice"

	"github.com/gin-gonic/gin"
)

// Router 装配 HTTP 面：Agent WebSocket、3D 资产文件端点（无状态本地
// 文件服务）与 SPA 静态回退。瓦片/日志数据/配置均在前端，不设路由。
func Router(cfgSvc configservice.Service, staticHandler http.Handler) *gin.Engine {
	gin.SetMode(gin.ReleaseMode)
	r := gin.New()
	r.Use(gin.Recovery())

	registerAgentWS(r)

	r.GET("/model-file", handleModelFile())

	r.GET("/tiles/:id/*filepath", handleTilesetFile(cfgSvc))

	if staticHandler != nil {
		r.NoRoute(gin.WrapH(staticHandler))
	}
	return r
}

func handleModelFile() gin.HandlerFunc {
	return func(c *gin.Context) {
		path := strings.TrimSpace(c.Query("path"))
		if path == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "path required"})
			return
		}
		data, err := os.ReadFile(path)
		if err != nil {
			c.JSON(http.StatusNotFound, gin.H{"error": "model not found"})
			return
		}
		contentType := http.DetectContentType(data)
		if i := strings.IndexByte(contentType, ';'); i >= 0 {
			contentType = strings.TrimSpace(contentType[:i])
		}
		c.Header("Cache-Control", "public, max-age=3600")
		c.Data(http.StatusOK, contentType, data)
	}
}

// handleTilesetFile serves a 3D Tiles directory tree. Cesium loads
// tileset.json then streams the referenced .b3dm/.pnts children via relative
// paths and byte-range requests, so files are served with c.File (http.ServeFile
// under the hood: Range support + extension-based mime). The on-disk root is
// resolved by tileset name from configservice and confined to that root to
// prevent path traversal.
func handleTilesetFile(cfgSvc configservice.Service) gin.HandlerFunc {
	return func(c *gin.Context) {
		id := c.Param("id")
		root, ok := cfgSvc.ResolveTilesetDir(c.Request.Context(), id)
		if !ok {
			c.JSON(http.StatusNotFound, gin.H{"error": "tileset not found"})
			return
		}
		// Catch-all param includes a leading slash; force-absolute before Clean
		// so ".." segments collapse and cannot escape the root.
		rel := filepath.Clean("/" + c.Param("filepath"))
		full := filepath.Join(root, rel)
		if r, err := filepath.Rel(root, full); err != nil || strings.HasPrefix(r, "..") {
			c.JSON(http.StatusForbidden, gin.H{"error": "outside tileset root"})
			return
		}
		c.Header("Cache-Control", "public, max-age=3600")
		c.File(full)
	}
}
