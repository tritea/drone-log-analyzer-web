import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import cesium from 'vite-plugin-cesium'
import { resolve } from 'path'
import { wasmParser } from './scripts/vite-plugin-wasm.mjs'
import { cesiumCdn, resolveCesiumCdnBase } from './scripts/vite-plugin-cesium-cdn.mjs'

export default defineConfig(({ command, mode }) => {
  // Cesium CDN 离载（仅构建生效，dev 恒走本地）：
  // CESIUM_CDN=1 官方 jsdelivr（版本锁定 node_modules）；CESIUM_CDN=<url> 自定义源。
  // 开启后 dist 不含 ~14MB 的 cesium/ 目录，Cesium 流量不走本服务器。
  const cdnBase = command === 'build' ? resolveCesiumCdnBase(process.env.CESIUM_CDN) : ''
  return {
    plugins: [wasmParser(), vue(), ...(cdnBase ? [cesiumCdn(cdnBase)] : [cesium()])],

    root: 'frontend',
  publicDir: 'public',

  build: {
    outDir: 'dist',
    emptyOutDir: true,
    rollupOptions: {
      output: {
        entryFileNames: 'assets/[name].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name].[ext]',
        manualChunks(id) {
          if (id.includes('node_modules')) {
            // @intlify 是 vue-i18n 的运行时依赖，须与 vue 同 chunk，避免 vendor/vue-vendor 循环依赖
            if (id.includes('vue') || id.includes('@intlify')) return 'vue-vendor'
            if (id.includes('three')) return 'three-vendor'
            if (id.includes('leaflet')) return 'leaflet-vendor'
            return 'vendor'
          }
        }
      }
    },
    minify: 'terser',
    terserOptions: {
      compress: {
        drop_console: true,
        drop_debugger: true
      }
    },
    // sourcemap 仅开发构建保留（vite build --mode development）；
    // 生产 dist 不生成 .map——累计 ~5.8MB 会随 go:embed 进 exe/镜像。
    // vite dev 的 sourcemap 由 dev server 提供，与此无关。
    sourcemap: mode === 'development',
    target: 'es2020',
    chunkSizeWarningLimit: 1000
  },

  server: {
    port: 5173,
    // dev：后端 API（agent WS / 3D 资产文件端点）代理到 Go 服务器；
    // 生产同源直连（go:embed SPA 与 API 同一 gin 路由服务）。
    proxy: {
      '/agent': {
        target: 'http://127.0.0.1:8642',
        ws: true,
        changeOrigin: true
      },
      '/model-file': 'http://127.0.0.1:8642',
      '/tiles': 'http://127.0.0.1:8642'
    }
  },

  resolve: {
    alias: {
      '@': resolve(__dirname, './frontend/src')
    }
  }
  }
})
