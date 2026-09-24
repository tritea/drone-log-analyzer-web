package jsonstore

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"

	appcfg "drone-log-analyzer/app/config"
	"drone-log-analyzer/app/services/configservice"
)

// jsonstore：configservice 的 JSON 文件实现（配置目录见 app/config）。
// 仅剩 tileset 注册表的持久化——其余配置域已前端 localStorage 化。

const tilesetsFileName = "tilesets_v1.json"

type service struct {
	mu          sync.Mutex
	tilesetDirs map[string]string
}

func New() configservice.Service {
	return &service{}
}

func loadConfig(filename string, v any) error {
	path := appcfg.ConfigPath(filename)
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return fmt.Errorf("read %s: %w", filename, err)
	}
	if len(data) == 0 {
		return nil
	}
	if err := json.Unmarshal(data, v); err != nil {
		return fmt.Errorf("parse %s: %w", filename, err)
	}
	return nil
}

func saveConfig(filename string, v any) error {
	path := appcfg.ConfigPath(filename)
	data, err := json.MarshalIndent(v, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal %s: %w", filename, err)
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return fmt.Errorf("mkdir %s: %w", filename, err)
	}
	return os.WriteFile(path, data, 0o644)
}

func configPath(filename string) string {
	return appcfg.ConfigPath(filename)
}
