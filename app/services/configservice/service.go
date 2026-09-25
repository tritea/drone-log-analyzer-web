package configservice

import (
	"context"
	"errors"
)

// 配置服务仅剩 tileset 注册表：/tiles/:id 的目录解析与（冻结的）注册面
// 的后端存储。其余配置（设置/曲线/模板/模型/LLM creds）已全部前端
// localStorage 化，服务端无状态。
var (
	ErrTilesetNameInvalid = errors.New("tileset name invalid")
	ErrTilesetNotFound    = errors.New("tileset not found")
)

type Service interface {
	ListTilesets(ctx context.Context) (*TilesetsResponse, error)
	SaveTileset(ctx context.Context, req Tileset) (*TilesetsResponse, error)
	DeleteTileset(ctx context.Context, req DeleteTilesetRequest) (*TilesetsResponse, error)

	// ResolveTilesetDir maps a tileset name to its on-disk root directory for
	// serving over HTTP. Returns ok=false when the name is unknown or the
	// tileset is remote-only (no Dir).
	ResolveTilesetDir(ctx context.Context, name string) (string, bool)
}
