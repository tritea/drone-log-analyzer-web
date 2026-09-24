/**
 * Vite plugin（构建参数 CESIUM_CDN 启用）：把 Cesium 全家桶从本服务器
 * 离载到官方 CDN。作为 vite-plugin-cesium 的替代出现在 plugins 列表：
 *   - `import 'cesium'` 编译为全局 window.Cesium（rollup external +
 *     external-globals，与 vite-plugin-cesium 的 rebuildCesium=false 同构）
 *   - index.html 注入 CDN 的 Cesium.js 与 widgets.css；Workers/Assets/
 *     ThirdParty 由 Cesium 从 script src 自推基址，同样落在 CDN
 *   - 不再把 node_modules/cesium/Build/Cesium（~14MB）拷进 dist
 *
 * CESIUM_CDN 取值：`1` 用官方 jsdelivr（版本锁定本机安装的 cesium）；
 * 任意 URL 则作为自定义基址（如 npmmirror、自建镜像）。见 Makefile。
 */
import { readFileSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import externalGlobals from 'rollup-plugin-external-globals'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')

/** 解析 CESIUM_CDN 环境变量 → CDN 基址（以 / 结尾）；空串表示未启用。 */
export function resolveCesiumCdnBase(value) {
  if (!value || value === '0' || value === 'false') return ''
  if (value === '1' || value === 'true') {
    const { version } = JSON.parse(
      readFileSync(resolve(root, 'node_modules/cesium/package.json'), 'utf8')
    )
    return `https://cdn.jsdelivr.net/npm/cesium@${version}/Build/Cesium/`
  }
  return value.endsWith('/') ? value : `${value}/`
}

export function cesiumCdn(base) {
  return {
    name: 'dla-cesium-cdn',
    config() {
      return {
        build: {
          rollupOptions: {
            external: ['cesium'],
            plugins: [externalGlobals({ cesium: 'Cesium' })],
          },
        },
      }
    },
    transformIndexHtml() {
      return [
        { tag: 'link', attrs: { rel: 'stylesheet', href: `${base}Widgets/widgets.css` } },
        { tag: 'script', attrs: { src: `${base}Cesium.js` } },
      ]
    },
  }
}
