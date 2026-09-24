import { MAP_PROVIDERS, findProvider } from '@/modules/shared/map-providers';
import type { MapClient } from '../client';

/** 本地常量实现：provider 清单与瓦片 URL 全在前端，无服务端依赖。 */
export const localMapClient: MapClient = {
  async available(): Promise<boolean> {
    return true;
  },

  async providers(): Promise<{ providers: typeof MAP_PROVIDERS }> {
    return { providers: MAP_PROVIDERS };
  },

  tileUrl(providerId: string, x: number, y: number, z: number): string {
    const p = findProvider(providerId);
    if (p?.tileUrl) return p.tileUrl(x, y, z);
    if (p?.urlTemplate) {
      return p.urlTemplate.replace('{z}', String(z)).replace('{x}', String(x)).replace('{y}', String(y));
    }
    return '';
  },
};
