import type { MapProviderDef } from '@/modules/shared/map-providers';

/**
 * 地图服务：瓦片已前端直连 provider https（map-providers.ts 常量），
 * 无服务端代理与缓存。available 恒真（不再有后端依赖）。
 */
export interface MapClient {
  available(): Promise<boolean>;
  providers(): Promise<{ providers: MapProviderDef[] }>;
  /** 计算型瓦片 URL（子域散列/quadkey/esri 交换等无法用模板表达）。 */
  tileUrl(providerId: string, x: number, y: number, z: number): string;
}
