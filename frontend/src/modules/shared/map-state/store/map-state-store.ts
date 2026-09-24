import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { MapProvider, MapState } from '@/types';
import { MAP_PROVIDERS, isGcj02Provider } from '@/modules/shared/map-providers';
import { useUiStore } from '@/modules/shared/ui-store';
import { useView3dStore } from '@/modules/view3d'
import { usePlaybackStore } from '@/modules/playback';

/** 地图全局状态：激活/渲染器/provider 选择。瓦片直连 provider https
 * （map-providers.ts 常量），无服务端代理与缓存域。 */
export const useMapStateStore = defineStore('map-state', () => {
  const map = ref<MapState>({
    active: false,
    renderer: '2d',
    terrainOn: true,
    followDrone: false,
    lockView: false,
    droneModel: 'lowpoly',
    droneScale: 3,
    droneShaded: true,
    mapFps: 30,
    providerId: '',
    providers: [] as MapProvider[],
    showPath: true,
    showWaypoints: true,
    showRoute: true,
    loaded: false,
    loading: false,
    error: '',
    tileError: false,
    controlBarCollapsed: false,
  });

  function loadProviders(): Promise<void> {
    // 常量清单同步可取；保留 async 签号与 loading 语义（调用方不感知）。
    map.value.loading = true;
    try {
      // 过滤隐藏 provider（地形 DEM 等）：底图选择器不出现，仅地形渲染按 id 拉取。
      const providers = MAP_PROVIDERS.filter((p) => !p.hidden) as MapProvider[];
      map.value.providers = providers;
      if (!map.value.providerId && providers.length) map.value.providerId = providers[0].id;
      if (map.value.providerId && !providers.some((p) => p.id === map.value.providerId)) {
        map.value.providerId = providers.length ? providers[0].id : '';
      }
      map.value.loaded = true;
    } finally {
      map.value.loading = false;
    }
    return Promise.resolve();
  }

  async function setMapActive(on: boolean): Promise<void> {
    map.value.active = on;
    if (on) {
      usePlaybackStore().ensureThreeTelemetry();
      if (!map.value.loaded) await loadProviders();
    } else if (useUiStore().ui.mainView === 'three') {
      useView3dStore().ensureView3d();
    }
  }

  const setProvider = (id: string): void => { map.value.providerId = id; map.value.tileError = false; };

  async function setMapRenderer(r: MapState['renderer']): Promise<void> {
    if (map.value.renderer === r) return;
    map.value.renderer = r;
    if (map.value.active) {
      usePlaybackStore().ensureThreeTelemetry();
      if (!map.value.loaded) await loadProviders();
    }
  }
  const setMapTerrain = (on: boolean): void => { map.value.terrainOn = on; };
  const setMapFollow = (on: boolean): void => { map.value.followDrone = on; };
  const setMapLockView = (on: boolean): void => { map.value.lockView = on; };
  const setControlBarCollapsed = (on: boolean): void => { map.value.controlBarCollapsed = !!on; };
  const toggleControlBar = (): void => { map.value.controlBarCollapsed = !map.value.controlBarCollapsed; };
  const setMapDroneModel = (m: string): void => { map.value.droneModel = m === 'glb' || m === 'lowpoly' ? m : 'lowpoly'; };
  const setMapDroneScale = (v: number): void => { if (typeof v === 'number' && isFinite(v)) map.value.droneScale = Math.max(0.1, Math.min(20, v)); };
  const setMapDroneShaded = (on: boolean): void => { map.value.droneShaded = !!on; };
  const setMapFps = (v: number): void => { map.value.mapFps = !v || v <= 0 ? 0 : Math.max(1, Math.round(v)); };
  const terrainAllowed = (): boolean => !mapCoordNeedsGcj02();

  const currentAttribution = (): string => {
    const p = map.value.providers.find((x) => x.id === map.value.providerId);
    return (p && (p.attribution || p.name)) || '';
  };

  /** 高德系 provider 坐标为 GCJ-02（换源跨制度时，轨迹/航点须按新制度重算）。 */
  function mapCoordNeedsGcj02(): boolean {
    return isGcj02Provider(map.value.providerId);
  }

  return {
    map,
    loadProviders,
    setMapActive,
    setProvider,
    setMapRenderer,
    setMapTerrain,
    setMapFollow,
    setMapLockView,
    setMapDroneModel,
    setMapDroneScale,
    setMapDroneShaded,
    setMapFps,
    terrainAllowed,
    setControlBarCollapsed,
    toggleControlBar,
    currentAttribution,
    mapCoordNeedsGcj02,
  };
});
