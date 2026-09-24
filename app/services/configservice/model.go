package configservice

// 仅剩 tileset 注册表 DTO（其余配置域已前端 localStorage 化）。

type Tileset struct {
	Name         string  `json:"name"`
	Dir          string  `json:"dir"`           // local tileset root dir (in-place reference or copied)
	Url          string  `json:"url,omitempty"` // optional remote tileset.json URL (Cesium Ion / external)
	HeightOffset float64 `json:"heightOffset"`  // height offset in metres (terrain fit)
	Scale        float64 `json:"scale"`         // uniform scale (1 = unchanged; only applies under manual position)
	Lon          float64 `json:"lon"`           // manual geo longitude in degrees; Lon=0 & Lat=0 = use tileset's native georef
	Lat          float64 `json:"lat"`           // manual geo latitude in degrees (paired with Lon)
	Yaw          float64 `json:"yaw"`           // three-axis rotation: yaw in degrees (about local Up)
	Pitch        float64 `json:"pitch"`         // three-axis rotation: pitch in degrees
	Roll         float64 `json:"roll"`          // three-axis rotation: roll in degrees
	Hidden       bool    `json:"hidden"`
	CreatedAt    string  `json:"createdAt,omitempty"`
	UpdatedAt    string  `json:"updatedAt,omitempty"`
}

type TilesetsResponse struct {
	Tilesets     []Tileset `json:"tilesets"`
	Path         string    `json:"path"`
	SavedTileset string    `json:"savedTileset,omitempty"`
}

type DeleteTilesetRequest struct {
	Name string `json:"name"`
}
