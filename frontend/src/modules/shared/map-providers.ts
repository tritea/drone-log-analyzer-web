/**
 * 地图瓦片 provider 常量：前端直连各 provider 的 https 端点（原 Go 代理
 * +MBTiles 缓存管线已随无状态化移除）。URL 公式逐条镜像自
 * app/modules/maptiles/providers/*（Go 源即规格：子域挑选、esri 的 z/y/x
 * 交换、bing quadkey、高德 style 码）。
 *
 * providerId 与旧版保持一致（settings 兼容）；高德系为 GCJ-02 坐标。
 */

export interface MapProviderDef {
  id: string;
  name: string;
  attribution: string;
  /** 模板 URL（{z}/{x}/{y} 占位）；计算型 provider 用 tileUrl。 */
  urlTemplate?: string;
  /** 计算型瓦片 URL（quadkey/子域挑选等无法用模板表达的场景）。 */
  tileUrl?: (x: number, y: number, z: number) => string;
  /** Cesium UrlTemplateImageryProvider 可用的模板 + customTags。 */
  cesiumTags?: Record<string, (x: number, y: number, level: number) => string>;
  /** 高德系 provider 坐标为 GCJ-02（轨迹/航点跨制度换算的判定源）。 */
  gcj02: boolean;
  /** 非影像 provider（如地形 DEM）：不出现在底图选择器。 */
  hidden?: boolean;
  maxNativeZoom: number;
}

/** 高德子域挑选：1..4，标准散列公式（与 Go hostIndex 一致）。 */
function amapHost(x: number, y: number): number {
  return (x * 2 + y) % 4 + 1;
}

/** Bing quadkey 编码（z/x/y → 四叉树键）。 */
export function quadKey(x: number, y: number, z: number): string {
  let key = '';
  for (let bit = z - 1; bit >= 0; bit--) {
    let digit = 0x00;
    const mask = 1 << bit;
    if ((x & mask) !== 0) digit += 1;
    if ((y & mask) !== 0) digit += 2;
    key += digit.toString();
  }
  return key;
}

const BING_PARAMS =
  'mkt=zh-CN&it=GB,LC&shading=hill&n=t&og=2505&cstl=s23&o=webp&ur=hk';

// OSM provider 已按需求移除（2026-09-20）；未知/已移除 id 统一回退高德矢量，
// 避免持久化旧配置（providerId='osm'）落成空白瓦片。
export const MAP_PROVIDERS: MapProviderDef[] = [
  {
    id: 'esri_satellite',
    name: 'Esri 卫星影像',
    attribution: 'Imagery © Esri, Maxar, Earthstar Geographics',
    // Esri 的 /tile/z/y/x 寻址交换了 y 与 x（行在前列在后）。
    urlTemplate: 'https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}',
    tileUrl: (x, y, z) =>
      `https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/${z}/${y}/${x}`,
    gcj02: false,
    maxNativeZoom: 18,
  },
  {
    id: 'amap_vector',
    name: '高德矢量地图',
    attribution: '© AutoNavi (高德)',
    urlTemplate: 'https://webrd0{n}.is.autonavi.com/appmaptile?lang=zh_cn&size=1&scale=1&style=7&x={x}&y={y}&z={z}',
    tileUrl: (x, y, z) =>
      `https://webrd0${amapHost(x, y)}.is.autonavi.com/appmaptile?lang=zh_cn&size=1&scale=1&style=7&x=${x}&y=${y}&z=${z}`,
    cesiumTags: { n: (x, y) => String(amapHost(x, y)) },
    gcj02: true,
    maxNativeZoom: 18,
  },
  {
    id: 'amap_road',
    name: '高德路标地图',
    attribution: '© AutoNavi (高德)',
    urlTemplate: 'https://webst0{n}.is.autonavi.com/appmaptile?style=8&x={x}&y={y}&z={z}',
    tileUrl: (x, y, z) =>
      `https://webst0${amapHost(x, y)}.is.autonavi.com/appmaptile?style=8&x=${x}&y=${y}&z=${z}`,
    cesiumTags: { n: (x, y) => String(amapHost(x, y)) },
    gcj02: true,
    maxNativeZoom: 18,
  },
  {
    id: 'amap_satellite',
    name: '高德卫星地图',
    attribution: '© AutoNavi (高德)',
    urlTemplate: 'https://webst0{n}.is.autonavi.com/appmaptile?style=6&x={x}&y={y}&z={z}',
    tileUrl: (x, y, z) =>
      `https://webst0${amapHost(x, y)}.is.autonavi.com/appmaptile?style=6&x=${x}&y=${y}&z=${z}`,
    cesiumTags: { n: (x, y) => String(amapHost(x, y)) },
    gcj02: true,
    maxNativeZoom: 18,
  },
  {
    id: 'bing_road',
    name: 'Bing街道地图',
    attribution: '© Microsoft Corporation',
    urlTemplate: `https://t.ssl.ak.dynamic.tiles.virtualearth.net/comp/ch/{q}?${BING_PARAMS}`,
    tileUrl: (x, y, z) =>
      `https://t.ssl.ak.dynamic.tiles.virtualearth.net/comp/ch/${quadKey(x, y, z)}?${BING_PARAMS}`,
    cesiumTags: { q: (x, y, level) => quadKey(x, y, level) },
    gcj02: false,
    maxNativeZoom: 19,
  },
  {
    id: 'dem_terrarium',
    name: '地形高程 (Terrarium)',
    attribution: '© Mapzen/AWS Terrain Tiles',
    urlTemplate: 'https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{z}/{x}/{y}.png',
    tileUrl: (x, y, z) => `https://s3.amazonaws.com/elevation-tiles-prod/terrarium/${z}/${x}/${y}.png`,
    gcj02: false,
    hidden: true,
    maxNativeZoom: 14,
  },
];

export function findProvider(id: string): MapProviderDef | undefined {
  return MAP_PROVIDERS.find((p) => p.id === id) ?? MAP_PROVIDERS.find((p) => p.id === 'amap_vector');
}

/** 是否 GCJ-02 provider（未知 id 视为非 GCJ，与旧 isGcj02Provider 对齐）。 */
export function isGcj02Provider(id: string): boolean {
  return findProvider(id)?.gcj02 ?? false;
}

/** 底图选择器可见 provider（hidden 过滤）。 */
export function visibleProviders(): MapProviderDef[] {
  return MAP_PROVIDERS.filter((p) => !p.hidden);
}
